use driver_rust::elevio::elev::DIRN_STOP;
use std::collections::HashMap;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct ElevatorState {
    pub behaviour: String,
    pub floor: u8,
    pub direction: u8,
    pub cab_requests: Vec<bool>,
}

impl ElevatorState {
    pub fn new(n_floors: u8) -> ElevatorState {
        ElevatorState {
            behaviour: "idle".to_string(),
            floor: 0,
            direction: DIRN_STOP,
            cab_requests: vec![false; n_floors as usize],
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct ElevatorData {
    pub version: u64,
    pub hall_requests: Vec<Vec<bool>>,
    pub states: HashMap<String, ElevatorState>,
}

impl ElevatorData {
    pub fn new(n_floors: u8) -> ElevatorData {
        let hall_requests = (0..n_floors)
            .map(|_| vec![false, false])
            .collect::<Vec<Vec<bool>>>();

        ElevatorData {
            version: 0,
            hall_requests,
            states: HashMap::new(),
        }
    }

    pub fn update_state(&mut self, id: String, state: ElevatorState) {
        self.states.insert(id, state);
    }

    pub fn update_data(&mut self, data: ElevatorData) {
        self.version = data.version;
        self.hall_requests = data.hall_requests;
        self.states = data.states;
    }
}
