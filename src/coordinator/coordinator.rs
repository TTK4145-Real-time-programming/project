use driver_rust::elevio::elev::{CAB, HALL_DOWN, HALL_UP};
use crossbeam_channel as cbc;
use std::{collections::HashMap, process::Command};
use network_rust::udpnet::peers::PeerUpdate;
use crate::shared_structs::{ElevatorData, ElevatorState, Behaviour, Direction};


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
pub struct Coordinator{
    elevator_data: ElevatorData, 
    local_id: String,
    n_floors: u8,
    
    // Hardware channels
    hw_button_light_tx: cbc::Sender<(u8,u8,bool)>,
    hw_request_rx: cbc::Receiver<(u8,u8)>,

    //Local elevaotr com channels
    request_tx: cbc::Sender<(u8, u8)>,
    state_rx: cbc::Receiver<ElevatorState>,
    complete_order_rx: cbc::Receiver<(u8, u8)>,

    //Network thread channels
    data_send_tx: cbc::Sender<ElevatorData>,
    data_recv_rx: cbc::Receiver<ElevatorData>, 
    peer_update_rx: cbc::Receiver<PeerUpdate>,
}


impl Coordinator{
    //Initilizing Request assigner struct and puts it in a thread (?)
    pub fn new(
        elevator_data: ElevatorData,
        local_id: String,
        n_floors: u8,

        hw_button_light_tx: cbc::Sender<(u8,u8,bool)>,
        hw_request_rx: cbc::Receiver<(u8,u8)>,

        request_tx: cbc::Sender<(u8, u8)>,
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
            state_rx,
            complete_order_rx,
            request_tx,

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
                let merge_type = self.check_version(elevator_data.version);
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
            },

            GlobalEvent::NewButtonRequest(new_button_request) => {
                //Checking if button already has been handled
                if new_button_request.1 == CAB && !self.elevator_data.states.get_mut(&self.local_id).unwrap().cab_requests[new_button_request.0 as usize] {
                    //Updating local elevator data
                    self.elevator_data.states.get_mut(&self.local_id).unwrap().cab_requests[new_button_request.0 as usize] = true;
                    //Sending the new request to local elevator
                    if let Err(e) = self.request_tx.send(new_button_request) {
                        eprintln!("Failed to send new button request to local elevator from coordinator: {:?}", e);
                        std::process::exit(1);
                    }
                    //Updating lights
                    self.update_lights((new_button_request.0, new_button_request.1, true));
                }
                else if new_button_request.1 == HALL_DOWN && !self.check_hall_button(new_button_request.0, HALL_DOWN) {
                    //Updating local elevator data
                    self.elevator_data.hall_requests[new_button_request.0 as usize][HALL_DOWN as usize] = true;
                    //Sending the new request to local elevator
                    if let Err(e) = self.request_tx.send(new_button_request) {
                        eprintln!("Failed to send new button request to local elevator from coordinator: {:?}", e);
                        std::process::exit(1);
                    }
                    //Updating lights
                    self.update_lights((new_button_request.0, new_button_request.1, true));
                }
                else if new_button_request.1 == HALL_UP && !self.check_hall_button(new_button_request.0, HALL_UP) {
                    //Updating local elevator data
                    self.elevator_data.hall_requests[new_button_request.0 as usize][HALL_UP as usize] = true;
                    //Sending the new request to local elevator
                    if let Err(e) = self.request_tx.send(new_button_request) {
                        eprintln!("Failed to send new button request to local elevator from coordinator: {:?}", e);
                        std::process::exit(1);
                    }
                    //Updating lights
                    self.update_lights((new_button_request.0, new_button_request.1, true));
                }

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
                    return GlobalEvent::NewButtonRequest(request);
                 },
                 Err(e) => {
                     eprintln!("Error extracting button package in coordinator: {:?}\r\n", e);
                     std::process::exit(1);
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
                     eprintln!("Error extracting network package in coordinator: {:?}\r\n", e);
                     std::process::exit(1);
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
                     eprintln!("Error extracting completed order from local elevator in coordinator: {:?}\r\n", e);
                     std::process::exit(1);
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
            eprintln!("Failed to send light command to light thread from coordinator: {:?}", e);
            std::process::exit(1);
        }
    }

    //Calcualting hall requests
    fn hall_request_assigner(&self, transmit: bool){
        // let hra_input = serde_json::to_string(&self.elevator_data).expect("Failed to serialize data");
        let hra_input = ElevatorData {
            version: 0,
            hall_requests: vec![vec![true, false], vec![false, true], vec![false, false], vec![false, false]],
            states: HashMap::from([
                ("id_1".to_string(), ElevatorState {
                    behaviour: Behaviour::Idle,
                    floor: 0,
                    direction: Direction::Stop,
                    cab_requests: vec![false, false, true, false],
                }),
            ]),
        };
        let hra_input_serialized = serde_json::to_string(&hra_input).expect("Failed to serialize data");
        // println!("Serialized data: {:?}", hra_input_serialized);
        
        // Run the Linux executable with serialized_data as input
        let hra_output = Command::new("./src/coordinator/hall_request_assigner")
        .arg("--input")
        .arg(&hra_input_serialized)
        .output()
        .expect("Failed to execute hall_request_assigner");

        // Check if the command was executed successfully
        if hra_output.status.success() {
            // The output of the executable is in the `stdout` field of the `hra_output` variable
            let hra_output_str = String::from_utf8(hra_output.stdout).expect("Invalid UTF-8 hra_output");
            
            // Use hra_output_str as needed
            // println!("hra_output from hall_request_assigner: {:?}", hra_output_str);

            // Convert back to ElevatorData or send through a channel, etc.
            // let new_hall_requests: ElevatorData = serde_json::from_str(&hra_output_str).expect("Failed to deserialize hra_output");
            // if transmit {
            //     if let Err(e) = self  
            //     .hall_requests_tx
            //     .send(new_hall_requests.hall_requests) {
            //         eprintln!("Failed to send hall requests from coordinator: {:?}", e);
            //         std::process::exit(1);
            //     }
            // }

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
    
    //Checks if hall button is already been handled (return false if not pressed)
    fn check_hall_button(&self, floor: u8, call: u8) -> bool{
        if call == HALL_DOWN && !self.elevator_data.hall_requests[floor as usize][0] {
                return false;
        }
        else if call == HALL_UP && !self.elevator_data.hall_requests[floor as usize][1] {
                return false;
        }

        //Hall request has already been handled
        else{
            return true;
        }
    }
}



