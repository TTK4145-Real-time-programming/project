/***************************************/
/*        3rd party libraries          */
/***************************************/
use driver_rust::elevio::elev::{CAB, HALL_DOWN, HALL_UP};
use log::info;
use network_rust::udpnet::peers::PeerUpdate;
use std::{collections::HashMap, process::Command};
use crossbeam_channel as cbc;
use std::time::Duration;

/***************************************/
/*           Local modules             */
/***************************************/
use crate::shared::{Behaviour, Direction, ElevatorData, ElevatorState};

/***************************************/
/*               Enums                 */
/***************************************/
pub enum Event {
    NewPackage(ElevatorData),
    RequestReceived((u8, u8)),
    NewPeerUpdate(PeerUpdate),
    NewElevatorState(ElevatorState),
    OrderComplete((u8, u8)),
}

#[derive(PartialEq, Debug)]
pub enum MergeType {
    Conflict,
    Accept,
    Reject,
}

/***************************************/
/*             Public API              */
/***************************************/
pub struct Coordinator {
    // Private fields
    coordinator_terminate_rx: cbc::Receiver<()>,
    elevator_data: ElevatorData,
    local_id: String,
    n_floors: u8,

    // Hardware channels
    hw_button_light_tx: cbc::Sender<(u8, u8, bool)>,
    hw_request_rx: cbc::Receiver<(u8, u8)>,

    // FSM channels
    fsm_hall_requests_tx: cbc::Sender<Vec<Vec<bool>>>,
    fsm_cab_request_tx: cbc::Sender<u8>,
    fsm_state_rx: cbc::Receiver<ElevatorState>,
    fsm_order_complete_rx: cbc::Receiver<(u8, u8)>,

    // Network channels
    net_data_send_tx: cbc::Sender<ElevatorData>,
    net_data_recv_rx: cbc::Receiver<ElevatorData>,
    net_peer_update_rx: cbc::Receiver<PeerUpdate>,
}

impl Coordinator {
    pub fn new(
        elevator_data: ElevatorData,
        local_id: String,
        n_floors: u8,

        hw_button_light_tx: cbc::Sender<(u8, u8, bool)>,
        hw_request_rx: cbc::Receiver<(u8, u8)>,

        fsm_hall_requests_tx: cbc::Sender<Vec<Vec<bool>>>,
        fsm_cab_request_tx: cbc::Sender<u8>,
        fsm_state_rx: cbc::Receiver<ElevatorState>,
        fsm_order_complete_rx: cbc::Receiver<(u8, u8)>,

        net_data_send_tx: cbc::Sender<ElevatorData>,
        net_data_recv_rx: cbc::Receiver<ElevatorData>,
        net_peer_update_rx: cbc::Receiver<PeerUpdate>,

        coordinator_terminate_rx: cbc::Receiver<()>,
    ) -> Coordinator {
        Coordinator {
            // Private fields
            coordinator_terminate_rx,
            elevator_data,
            local_id,
            n_floors,

            //Hardware channels
            hw_button_light_tx,
            hw_request_rx,

            // FSM channels
            fsm_hall_requests_tx,
            fsm_cab_request_tx,
            fsm_state_rx,
            fsm_order_complete_rx,

            // Netowrk channels
            net_data_recv_rx,
            net_peer_update_rx,
            net_data_send_tx,
        }
    }

    pub fn run(&mut self) {
        // Main loop
        loop {
            cbc::select! {
                //Handling new package
                recv(self.net_data_recv_rx) -> package => {
                   match package {
                        Ok(elevator_data) => self.handle_event(Event::NewPackage(elevator_data)),
                        Err(e) => {
                            eprintln!("Error extracting network package in coordinator: {:?}\r\n", e);
                            std::process::exit(1);
                        }
                    }
                },
    
                //Hanlding peer update
                recv(self.net_peer_update_rx) -> peer => {
                    match peer {
                        Ok(peer_update) => self.handle_event(Event::NewPeerUpdate(peer_update)),
                        Err(e) => {
                            eprintln!("Error extracting peer update package in coordinator: {:?}\r\n", e);
                            std::process::exit(1);
                        }
                    }
                },
    
                //Handling new button press
                recv(self.hw_request_rx) -> request => {
                    match request {
                        Ok(request) => self.handle_event(Event::RequestReceived(request)),
                        Err(e) => {
                            eprintln!("Error extracting button package in coordinator: {:?}\r\n", e);
                            std::process::exit(1);
                        }
                    }
                },
    
                // Handling new fsm state
                recv(self.fsm_state_rx) -> state => {
                    match state {
                        Ok(state) => self.handle_event(Event::NewElevatorState(state)),
                        Err(e) => {
                            eprintln!("Error extracting network package in coordinator: {:?}\r\n", e);
                            std::process::exit(1);
                        }
                    }
                },
    
                // Handling completed order from fsm
                recv(self.fsm_order_complete_rx) -> completed_order => {
                    match completed_order {
                        Ok(finish_order) => self.handle_event(Event::OrderComplete(finish_order)),
                        Err(e) => {
                            eprintln!("Error extracting completed order from fsm in coordinator: {:?}\r\n", e);
                            std::process::exit(1);
                        }
                    }
                }
    
                recv(self.coordinator_terminate_rx) -> _ => {
                    break;
                }
    
                default(Duration::from_millis(50)) => {
                    // TODO: Maybe do something idunno
                }
            }
        }
    }

    fn handle_event(&mut self, event: Event) {
        match event {
            Event::NewPackage(elevator_data) => {
                let merge_type = self.check_version(elevator_data.version);

                match merge_type {
                    MergeType::Accept => {
                        //Updating lights
                        let new_hall_request = elevator_data.hall_requests.clone();
                        for floor in 0..self.n_floors {
                            if new_hall_request[floor as usize][HALL_DOWN as usize]
                                != self.elevator_data.hall_requests[floor as usize]
                                    [HALL_DOWN as usize]
                            {
                                self.update_lights((
                                    floor,
                                    HALL_DOWN,
                                    new_hall_request[floor as usize][HALL_DOWN as usize],
                                ));
                            }
                            if new_hall_request[floor as usize][HALL_UP as usize]
                                != self.elevator_data.hall_requests[floor as usize]
                                    [HALL_UP as usize]
                            {
                                self.update_lights((
                                    floor,
                                    HALL_UP,
                                    new_hall_request[floor as usize][HALL_UP as usize],
                                ));
                            }
                        }
                        //Writing the new changes to elevatorData
                        self.elevator_data.version = elevator_data.version;
                        self.elevator_data.hall_requests = new_hall_request;
                        self.elevator_data.states = elevator_data.states;

                        self.hall_request_assigner(false);
                    }
                    MergeType::Conflict => {
                        // TODO: merge conflict
                    }
                    MergeType::Reject => {
                        // TODO: reject merge
                    }
                }
            }

            Event::NewPeerUpdate(peer_update) => {
                let mut lost_elevators = peer_update.lost;
                info!("Peers: {:?}", peer_update.peers);

                //Removing dead elevators
                for elevator in lost_elevators.iter_mut() {
                    self.elevator_data.states.remove(elevator);
                }

                // Add new elevators
                for id in peer_update.new.iter() {
                    self.elevator_data.states.insert(
                        id.clone(),
                        ElevatorState {
                            behaviour: Behaviour::Idle,
                            floor: 0,
                            direction: Direction::Stop,
                            cab_requests: vec![false; self.n_floors as usize],
                        },
                    );
                }
            }

            Event::RequestReceived(request) => {
                if request.1 == CAB {
                    // Updating elevator data
                    self.elevator_data
                        .states
                        .get_mut(&self.local_id)
                        .unwrap()
                        .cab_requests[request.0 as usize] = true;

                    //Sending the change to the fsm
                    self.fsm_cab_request_tx
                        .send(request.0)
                        .expect("Failed to send cab request to fsm");

                    //Updating lights
                    self.update_lights((request.0, CAB, true));
                } 
                
                else if request.1 == HALL_DOWN || request.1 == HALL_UP {
                    //Updating hall requests
                    self.elevator_data.hall_requests[request.0 as usize][request.1 as usize] = true;

                    // Calculating and sending to fsm
                    self.hall_request_assigner(true);

                    // Updating lights
                    self.update_lights((request.0, request.1, true));
                }

            }

            Event::NewElevatorState(elevator_state) => {
                // Checking for new cab requests
                let current_cab_requests = &self.elevator_data.states[&self.local_id].cab_requests;

                for floor in 0..self.n_floors {
                    if !current_cab_requests[floor as usize] && elevator_state.cab_requests[floor as usize] {
                        //Updating cab button lights with new changes from FSM
                        self.update_lights((floor, CAB, true));
                    }
                }

                // Updating state elevator data
                if let Some(state) = self.elevator_data.states.get_mut(&self.local_id) {
                    *state = elevator_state;
                }

                self.hall_request_assigner(true);

            }

            Event::OrderComplete(completed_order) => {
                info!("Order completed: {:?}", completed_order);
                // Updating elevator data
                if completed_order.1 == CAB {
                    self.elevator_data
                        .states
                        .get_mut(&self.local_id)
                        .unwrap()
                        .cab_requests[completed_order.0 as usize] = false;
                }

                if completed_order.1 == HALL_DOWN || completed_order.1 == HALL_UP {
                    self.elevator_data.hall_requests[completed_order.0 as usize][completed_order.1 as usize] = false;
                }
                
                // Update lights and hall requests
                self.update_lights((completed_order.0, completed_order.1, false));
                self.hall_request_assigner(true);
            }
        }
    }

    // Update lights
    fn update_lights(&self, light: (u8, u8, bool)) {
        //Sending change in lights
        if let Err(e) = self.hw_button_light_tx.send(light) {
            eprintln!(
                "Failed to send light command to light thread from coordinator: {:?}",
                e
            );
            std::process::exit(1);
        }
    }

    // Calcualting hall requests
    fn hall_request_assigner(&mut self, transmit: bool) {

        // Serialize data
        let mut json_value: serde_json::Value = serde_json::to_value(&self.elevator_data)
            .expect("Failed to serialize data");

        // Remove the `version` field from the serialized data
        json_value.as_object_mut().unwrap().remove("version");

        let hra_input = serde_json::to_string(&json_value).expect("Failed to serialize data");

        // Run the executable with serialized_data as input
        let hra_output = Command::new("./src/coordinator/hall_request_assigner")
            .arg("--input")
            .arg(&hra_input)
            .output()
            .expect("Failed to execute hall_request_assigner");

        if hra_output.status.success() {
            // Fetch and deserialize output
            let hra_output_str = String::from_utf8(hra_output.stdout).expect("Invalid UTF-8 hra_output");
            let hra_output = serde_json::from_str::<HashMap<String, Vec<Vec<bool>>>>(&hra_output_str)
                    .expect("Failed to deserialize hra_output");

            // Update hall requests assigned to local elevator
            let mut local_hall_requests = vec![vec![false; 2]; self.n_floors as usize];
            for (id, hall_requests) in hra_output.iter() {
                if id == &self.local_id {
                    for floor in 0..self.n_floors {
                        local_hall_requests[floor as usize][HALL_UP as usize] = hall_requests[floor as usize][HALL_UP as usize];
                        local_hall_requests[floor as usize][HALL_DOWN as usize] = hall_requests[floor as usize][HALL_DOWN as usize];
                    }
                }
            }

            // Transmit the updated hall requests to the FSM
            self.fsm_hall_requests_tx.send(local_hall_requests).expect("Failed to send hall requests to fsm");
        } 
        
        else {
            // If the executable did not run successfully, you can handle the error
            let error_message = String::from_utf8(hra_output.stderr).expect("Invalid UTF-8 error hra_output");
            eprintln!("Error executing hall_request_assigner: {:?}", error_message);
            std::process::exit(1);
        }

        // Transmit the updated elevator on the network
        if transmit {
            self.elevator_data.version += 1;
            self.net_data_send_tx
                .send(self.elevator_data.clone())
                .expect("Failed to send elevator data to network thread");
        }
    }

    // Checks if incomming version is newer than local version
    fn check_version(&self, version: u64) -> MergeType {
        if version > self.elevator_data.version {
            MergeType::Accept
        } 
        
        else if version == self.elevator_data.version {
            MergeType::Conflict
        }

        else {
            MergeType::Reject
        }
    }

}

/***************************************/
/*              Test API               */
/***************************************/
#[cfg(test)]
pub mod testing {
    use super::Coordinator;
    use crate::shared::ElevatorData;
    use crate::shared::ElevatorState;
    use network_rust::udpnet::peers::PeerUpdate;

    impl Coordinator {
        // Publicly expose the private fields for testing
        pub fn test_get_data(&self) -> &ElevatorData {
            &self.elevator_data
        }

        pub fn test_get_local_id(&self) -> &String {
            &self.local_id
        }
        
        pub fn test_get_n_floors(&self) -> &u8 {
            &self.n_floors
        }

        pub fn test_update_lights(&self, light: (u8, u8, bool)) {
            self.update_lights(light);
        }

        pub fn test_check_version(&self, version: u64) -> super::MergeType {
            self.check_version(version)
        }

        pub fn test_set_version(&mut self, version: u64) {
            self.elevator_data.version = version;
        }

        pub fn test_hall_request_assigner(&mut self, transmit: bool) {
            self.hall_request_assigner(transmit);
        }

        pub fn test_set_hall_requests(&mut self, hall_requests: Vec<Vec<bool>>) {
            self.elevator_data.hall_requests = hall_requests;
        }

        pub fn test_set_state(&mut self, elevator: String, state: ElevatorState) {
            self.elevator_data.states.insert(elevator, state);
        }

        pub fn test_handle_event(&mut self, event: super::Event) {
            self.handle_event(event);
        }

        pub fn test_set_peer_list(&mut self, peer_list: PeerUpdate) {
            for id in peer_list.peers.iter() {
                self.elevator_data.states.insert(id.clone(), ElevatorState::new(self.n_floors));
            }
        }

        pub fn test_get_peer_list(&self) -> Vec<String> {
            let mut peer_list = vec![];
            for id in self.elevator_data.states.keys() {
                peer_list.push(id.clone());
            }
            peer_list.reverse();
            peer_list
        }
    }
}