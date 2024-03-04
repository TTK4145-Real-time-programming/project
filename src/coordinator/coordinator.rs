use driver_rust::elevio::elev::{CAB, HALL_DOWN, HALL_UP};
use crossbeam_channel as cbc;
use std::{collections::HashMap, process::Command};
use network_rust::udpnet::peers::PeerUpdate;
use crate::shared_structs::{ElevatorData, ElevatorState, Behaviour, Direction};


// Defining events the thread will trigger on
pub enum GlobalEvent {
    NewPackage(ElevatorData),
    RequestReceived((u8, u8)),
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
pub struct Coordinator{
    elevator_data: ElevatorData, 
    local_id: String,
    n_floors: u8,
    
    // Hardware channels
    hw_button_light_tx: cbc::Sender<(u8,u8,bool)>,
    hw_request_rx: cbc::Receiver<(u8,u8)>,

    //Local elevator com channels
    hall_requests_tx: cbc::Sender<Vec<Vec<bool>>>,
    cab_request_tx: cbc::Sender<u8>,
    state_rx: cbc::Receiver<ElevatorState>,
    complete_order_rx: cbc::Receiver<(u8, u8)>,

    //Network thread channels
    data_send_tx: cbc::Sender<ElevatorData>,
    data_recv_rx: cbc::Receiver<ElevatorData>, 
    peer_update_rx: cbc::Receiver<PeerUpdate>,
}


impl Coordinator{
    pub fn new(
        elevator_data: ElevatorData,
        local_id: String,
        n_floors: u8,

        hw_button_light_tx: cbc::Sender<(u8,u8,bool)>,
        hw_request_rx: cbc::Receiver<(u8,u8)>,

        hall_requests_tx: cbc::Sender<Vec<Vec<bool>>>,
        cab_request_tx: cbc::Sender<u8>,
        state_rx: cbc::Receiver<ElevatorState>,
        complete_order_rx: cbc::Receiver<(u8, u8)>,

        data_send_tx: cbc::Sender<ElevatorData>,
        data_recv_rx: cbc::Receiver<ElevatorData>,
        peer_update_rx: cbc::Receiver<PeerUpdate>,
    ) -> Result<Self, std::io::Error>{
        
        Ok(Coordinator{
            //Local elevator
            elevator_data,
            local_id,
            n_floors,

            //Hardware channels
            hw_button_light_tx,
            hw_request_rx,

            //Local elevator thread channels
            hall_requests_tx,
            cab_request_tx,
            state_rx,
            complete_order_rx,

            // Netowrk thread channels
            data_recv_rx,
            peer_update_rx,
            data_send_tx,
        })
    }
    
    // ---- main functions -----

    //Main run function
    pub fn run(&mut self) { 
        // Main Coordinator loop
        loop {
            let event: GlobalEvent = self.wait_for_event();
            self.handle_event(event);
        }
    }



    // ---- Extra functions -----

    fn handle_event(&mut self, event: GlobalEvent){
        match event {
            GlobalEvent::NewPackage(elevator_data) => {
                println!("New package: {:?}", elevator_data);
                // let merge_type = self.check_version(elevator_data.version);
                let merge_type = MergeEvent::MergeNew;
                if merge_type != MergeEvent::NoMerge {
                    //Incomming version newer than local
                    if merge_type == MergeEvent::MergeNew {
                        //Updating lights
                        let new_hall_request = elevator_data.hall_requests.clone();
                        for floor in 0..self.n_floors {
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

                // Add new elevators
                for id in peer_update.new.iter() {
                    println!("New elevator: {:?}", id);
                    self.elevator_data.states.insert(id.clone(), ElevatorState {
                        behaviour: Behaviour::Idle,
                        floor: 0,
                        direction: Direction::Stop,
                        cab_requests: vec![false; self.n_floors as usize],
                    });
                }
            },

            GlobalEvent::RequestReceived(request) => {
                
                if request.1 == CAB {
                    //Updating local elevator
                    self.elevator_data.states.get_mut(&self.local_id).unwrap().cab_requests[request.0 as usize] = true;
                    //Sending the change to the local elevator
                    self.cab_request_tx.send(request.0).expect("Failed to send cab request to local elevator");
                    //Updating lights
                    self.update_lights((request.0, CAB, true));
                }
                else if request.1 == HALL_DOWN || request.1 == HALL_UP {
                    //Checking if hall button has already been handled
                    if !self.check_hall_button(request.0, request.1){
                        //Updating hall requests
                        self.elevator_data.hall_requests[request.0 as usize][request.1 as usize] = true;
                        // Calculating and sending to local elevator
                        self.hall_request_assigner(true);
                        //Updating lights
                        self.update_lights((request.0, request.1, true));
                    }
                }

                // Send the updated elevator data
                self.data_send_tx.send(self.elevator_data.clone()).expect("Failed to send elevator data to network thread");

            },

            GlobalEvent::NewElevatorState(elevator_state) => {
                // Checking for new cab requests
                let current_cab_requests = &self.elevator_data.states[&self.local_id].cab_requests;

                for floor in 0..self.n_floors {
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

                // Send the updated elevator data
                self.data_send_tx.send(self.elevator_data.clone()).expect("Failed to send elevator data to network thread");
            },

            GlobalEvent::CompletedOrder(finish_order) => {
                //Updating elevatorData, lights and sending the change 
                println!("Order completed: {:?}", finish_order);
                if finish_order.1 == CAB {
                    self.elevator_data.states.get_mut(&self.local_id).unwrap().cab_requests[finish_order.0 as usize] = false;
                }
                else if finish_order.1 == HALL_DOWN {
                    self.elevator_data.hall_requests[finish_order.0 as usize][HALL_DOWN as usize] = false;
                }
                else if finish_order.1 == HALL_UP {
                    self.elevator_data.hall_requests[finish_order.0 as usize][HALL_UP as usize] = false;
                }
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
                    eprintln!("Error extracting network package in coordinator: {:?}\r\n", e);
                    std::process::exit(1);
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
                     eprintln!("Error extracting peer update package in coordinator: {:?}\r\n", e);
                     std::process::exit(1);
                 },
                }
             },
 
            //Handling new button press
            recv(self.hw_request_rx) -> request => {
                match request {
                 Ok(request) => {
                    return GlobalEvent::RequestReceived(request);
                 },
                 Err(e) => {
                     eprintln!("Error extracting button package in coordinator: {:?}\r\n", e);
                     std::process::exit(1);
                 },
                }
             },

            // Handling new local elevator state
            recv(self.state_rx) -> state => {
                match state {
                 Ok(state) => {
                    return GlobalEvent::NewElevatorState(state);
                 },
                 Err(e) => {
                     eprintln!("Error extracting network package in coordinator: {:?}\r\n", e);
                     std::process::exit(1);
                 },
                }
             },
             
            // Handling completed order from local elevator
            recv(self.complete_order_rx) -> completed_order => {
                match completed_order {
                 Ok(finish_order) => {
                    return GlobalEvent::CompletedOrder(finish_order);
                 },
                 Err(e) => {
                     eprintln!("Error extracting completed order from local elevator in coordinator: {:?}\r\n", e);
                     std::process::exit(1);
                 },
                }
             }
        }
        return GlobalEvent::NoEvent;
    }

    // Update lights
    fn update_lights(&self, light: (u8,u8,bool)){
        //Sending change in lights
        if let Err(e) = self.hw_button_light_tx.send(light) {
            eprintln!("Failed to send light command to light thread from coordinator: {:?}", e);
            std::process::exit(1);
        }
    }

    // Calcualting hall requests
    fn hall_request_assigner(&mut self, transmit: bool){
        let hra_input = serde_json::to_string(&self.elevator_data).expect("Failed to serialize data");
        
        // Run the Linux executable with serialized_data as input
        let hra_output = Command::new("./src/coordinator/hall_request_assigner")
        .arg("--input")
        .arg(&hra_input)
        .output()
        .expect("Failed to execute hall_request_assigner");

        // Check if the command was executed successfully
        if hra_output.status.success() {
            // The output of the executable is in the `stdout` field of the `hra_output` variable
            let hra_output_str = String::from_utf8(hra_output.stdout).expect("Invalid UTF-8 hra_output");
            let hra_output = serde_json::from_str::<HashMap<String, Vec<Vec<bool>>>>(&hra_output_str).expect("Failed to deserialize hra_output");
            
            // Update hall requests assigned to local elevator (HRA has three inner dimentions lol)
            let mut local_hall_requests = vec![vec![false; 2]; self.n_floors as usize];
            for (id, hall_requests) in hra_output.iter() {
                if id == &self.local_id {
                    for floor in 0..self.n_floors {
                        local_hall_requests[floor as usize][HALL_UP as usize] = hall_requests[floor as usize][HALL_UP as usize];
                        local_hall_requests[floor as usize][HALL_DOWN as usize] = hall_requests[floor as usize][HALL_DOWN as usize];
                    }
                }
            }

            self.hall_requests_tx.send(local_hall_requests).expect("Failed to send hall requests to local elevator");
            

        } else {
            // If the executable did not run successfully, you can handle the error
            let error_message = String::from_utf8(hra_output.stderr).expect("Invalid UTF-8 error hra_output");
            eprintln!("Error executing hall_request_assigner: {:?}", error_message);
            std::process::exit(1);
        }

        // Send orders that belongs to local elevator
    }
    
    // Checks if incomming version is newer than local version
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
    
    // Checks if hall button is already been handled (return false if not pressed)
    fn check_hall_button(&self, floor: u8, call: u8) -> bool{
        if call == HALL_DOWN && !self.elevator_data.hall_requests[floor as usize][0] {
                return false;
        }
        else if call == HALL_UP && !self.elevator_data.hall_requests[floor as usize][1] {
                return false;
        }

        // Hall request has already been handled
        else{
            return true;
        }
    }
}



