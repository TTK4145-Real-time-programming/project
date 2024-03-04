use driver_rust::elevio::elev::{CAB, HALL_DOWN, HALL_UP};
use driver_rust::elevio::elev::Elevator;
use crossbeam_channel as cbc;
use std::sync::{Arc, Mutex};
use network_rust::udpnet::peers::PeerUpdate;
use crate::shared_structs::{ElevatorData, ElevatorState};


// Defining events the thread will trigger on
pub enum GlobalEvent {
    NewPackage(ElevatorData),
    NewButtonRequest((u8, u8)),
    NewPeerUpdate(PeerUpdate),
    NewElevatorState(ElevatorState),
    CompletedOrder((u8, u8)),
    NoEvent,
}

// Enum for mergiing of ElevetorData
#[derive(PartialEq)]
pub enum MergeEvent{
    MergeConflict,
    MergeNew,
    NoMerge,
}


// Request assigner 
pub struct Cordinator{
    elevator_data: ElevatorData, 
    local_elevator: Elevator,
    local_id: String,
    num_floors: u8,
    
    // Hardware channels
    hw_button_light_tx: cbc::Sender<(u8,u8,bool)>,
    hw_hall_request_rx: cbc::Receiver<(u8,u8)>,

    //Local elevaotr com channels
    hall_requests_tx: cbc::Sender<Vec<Vec<bool>>>,
    state_rx: cbc::Receiver<ElevatorState>,
    complete_order_rx: cbc::Receiver<(u8, u8)>,

    //Network thread channels
    data_send_tx: cbc::Sender<ElevatorData>,
    peer_update_rx: cbc::Receiver<PeerUpdate>,
    data_recv_rx: cbc::Receiver<ElevatorData>, 
}


impl Cordinator{
    //Initilizing Request assigner strcuct and puts it in a thread (?)
    pub fn init(
        elevator_data: ElevatorData,
        elevator_driver: Elevator, 
        local_id: String,
        num_floors: u8,

        hw_button_light_tx: cbc::Sender<(u8,u8,bool)>,
        hw_hall_request_rx: cbc::Receiver<(u8,u8)>,

        hall_requests_tx: cbc::Sender<Vec<Vec<bool>>>,
        state_rx: cbc::Receiver<ElevatorState>,
        complete_order_rx: cbc::Receiver<(u8, u8)>,

        data_send_tx: cbc::Sender<ElevatorData>,
        peer_update_rx: cbc::Receiver<PeerUpdate>,
        data_recv_rx: cbc::Receiver<ElevatorData>,
    ) -> Result<Self, std::io::Error>{
        
        Ok(Cordinator{
            //Local elevator
            elevator_data: elevator_data,
            local_elevator: elevator_driver,
            local_id: local_id,
            num_floors: num_floors,

            //Hardware channels
            hw_button_light_tx: hw_button_light_tx,
            hw_hall_request_rx: hw_hall_request_rx,

            //Local elevator thread channels
            state_rx: state_rx,
            complete_order_rx: complete_order_rx,
            hall_requests_tx: hall_requests_tx,

            // Netowrk thread channels
            data_recv_rx: data_recv_rx,
            peer_update_rx: peer_update_rx,
            data_send_tx: data_send_tx,
        })
    }
    
    // ---- main functions -----

    //Main run function
    pub fn run(& mut self, assigner: Arc<Mutex<Self>>) { 
        // Main cordinator loop
        loop {
            let event: GlobalEvent = self.wait_for_event();
            self.handle_event(event);
        }
    }



    // ---- Extra functions -----

    fn handle_event(&mut self, event: GlobalEvent){
        match event {
            GlobalEvent::NewPackage(elevator_data) => {
                let merge_type = self.check_version(elevator_data.version);
                if merge_type != MergeEvent::NoMerge {
                    //Incomming version newer than local
                    if merge_type == MergeEvent::MergeNew {
                        //Updating lights
                        let new_hall_request = elevator_data.hall_requests.clone();
                        for floor in 0..self.num_floors {
                            if new_hall_request[floor as usize][HALL_DOWN as usize] != self.elevator_data.hall_requests[floor as usize][HALL_DOWN as usize] {
                                self.update_lights((floor, HALL_DOWN, new_hall_request[floor as usize][HALL_DOWN as usize]));
                                }
                            if new_hall_request[floor as usize][HALL_UP as usize] != self.elevator_data.hall_requests[floor as usize][HALL_UP as usize] {
                                self.update_lights((floor, HALL_UP, new_hall_request[floor as usize][HALL_UP as usize]));
                                } 
                        }
                        //Writing the new changes to elevatorData
                        self.elevator_data.version = elevator_data.version;
                        self.elevator_data.hall_requests = new_hall_request;
                        self.elevator_data.states = elevator_data.states;

                        self.hall_request_assigner(false);
                    }

                    //Inncommning data has merge conflict
                    if merge_type == MergeEvent::MergeConflict {
                        // TODO: merge conflict
                        
                        //self.update_lights();
                        //self.hall_request_assigner(false);
                    }
                }
            },

            GlobalEvent::NewPeerUpdate(peer_update) => {
                let mut lost_elevators = peer_update.lost;

                //Removing dead elevators
                for elevator in lost_elevators.iter_mut() {
                    self.elevator_data.states.remove(elevator);
                }
            },

            GlobalEvent::NewButtonRequest(new_button_request) => {
                //Checking if button already has been handled
                if !self.check_hall_button(new_button_request.0, new_button_request.1) {
                    // Writing change to elvatorData
                    self.elevator_data.hall_requests[new_button_request.0 as usize][new_button_request.1 as usize] = true;
                    self.update_lights((new_button_request.0,new_button_request.1,true));
                    self.hall_request_assigner(true);
                }

            },

            GlobalEvent::NewElevatorState(elevator_state) => {
                // Checking for new cab requests
                let current_cab_requests = &self.elevator_data.states[&self.local_id].cab_requests;

                for floor in 0..self.num_floors {
                    if current_cab_requests[floor as usize] != elevator_state.cab_requests[floor as usize] {
                        //Updating cab button lights with new changes from FSM
                        self.update_lights((floor, CAB, current_cab_requests[floor as usize]));
                    }
                }

                // Changing state of local elevator
                if let Some(state) = self.elevator_data.states.get_mut(&self.local_id) {
                    *state = elevator_state;
                }

                self.hall_request_assigner(true);
            },

            GlobalEvent::CompletedOrder(finish_order) => {
                //Updating elevatorData, lights and sending the change 
                self.elevator_data.hall_requests[finish_order.0 as usize][finish_order.1 as usize] = false;
                self.update_lights((finish_order.0, finish_order.1, false));
                self.hall_request_assigner(true);
            },

            GlobalEvent::NoEvent => {
                // Do some data cleanup? 
            }
        }
    }

    
    fn wait_for_event(&self) -> GlobalEvent{
        cbc::select! {
            //Handling new package
            recv(self.data_recv_rx) -> package => {
               match package {
                Ok(elevator_data) => {
                return GlobalEvent::NewPackage(elevator_data);
                },
                Err(e) => {
                    println!("Error extracting network package in cordinator\n");
                },
               }
            },

            //Hanlding peer update
            recv(self.peer_update_rx) -> peer => {
                match peer {
                 Ok(peer_update) => {
                    return GlobalEvent::NewPeerUpdate(peer_update);
                 },
                 Err(e) => {
                     println!("Error extracting peer update package in cordinator\n");
                 },
                }
             },
 
            //Handling new button press
            recv(self.hw_hall_request_rx) -> new_button => {
                match new_button {
                 Ok(new_button_request) => {
                    return GlobalEvent::NewButtonRequest(new_button_request);
                 },
                 Err(e) => {
                     println!("Error extracting button package in cordinator\n");
                 },
                }
             },

            //Handling new local elevator state
            recv(self.state_rx) -> new_state => {
                match new_state {
                 Ok(elevator_state) => {
                    return GlobalEvent::NewElevatorState(elevator_state);
                 },
                 Err(e) => {
                     println!("Error extracting network package in cordinator\n");
                 },
                }
             },
             
            //Handling completed order from local elevator
            recv(self.complete_order_rx) -> completed_order => {
                match completed_order {
                 Ok(finish_order) => {
                    return GlobalEvent::CompletedOrder(finish_order);
                 },
                 Err(e) => {
                     println!("Error extracting completed order from local elevator in cordinator\n");
                 },
                }
             }
        }
        return GlobalEvent::NoEvent;
    }

    //Update lights
    fn update_lights(&self, light: (u8,u8,bool)){
        //Sending change in lights
        if let Err(e) = self.hw_button_light_tx.send(light) {
            eprintln!("Failed to send light command to light thread from cordinator: {:?}", e);
        }
    }

    //Calcualting hall requests
    fn hall_request_assigner(&self, transmit: bool){
        // TODO:
        // To JSON
        // run exe
        // back to ElevatorData -> data_send_tx
        // Send orders that belongs to local elevator
    }
    
    // Checks if incommning version is newer than local version
    fn check_version(&self, version: u64) -> MergeEvent{
        if version > self.elevator_data.version {
            return MergeEvent::MergeNew;
        }
        else if version == self.elevator_data.version{
            return MergeEvent::MergeConflict;
        }
        else{
            return MergeEvent::NoMerge;
        } 
    }
    
    //Checks if hall button is already been handled (return false if not pressed)
    fn check_hall_button(&self, floor: u8, call: u8) -> bool{
        if call == HALL_DOWN && !self.elevator_data.hall_requests[floor as usize][0] {
                return false;
        }
        else if call == HALL_UP && !self.elevator_data.hall_requests[floor as usize][1] {
                return false;
        }

        //Hall requst has already been handeled
        else{
            return true;
        }
    }
}



