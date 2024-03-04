use driver_rust::elevio::elev::{DIRN_UP, DIRN_DOWN, DIRN_STOP};
use std::collections::HashMap;
use serde::Serialize;
use serde::Deserialize;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Behaviour {
    Idle,
    Moving,
    DoorOpen,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Up,
    Down,
    Stop,
}

impl Direction {
    pub fn to_u8(&self) -> u8 {
        match *self {
            Direction::Up => DIRN_UP,
            Direction::Down => DIRN_DOWN,
            Direction::Stop => DIRN_STOP,
        }
    }
}

impl From<u8> for Direction {
    fn from(item: u8) -> Self {
        match item {
            DIRN_UP => Direction::Up,
            DIRN_DOWN => Direction::Down,
            DIRN_STOP => Direction::Stop,
            _ => panic!("Invalid direction value"),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ElevatorState {
    pub behaviour: Behaviour,
    pub floor: u8,
    pub direction: Direction,
    #[serde(rename = "cabRequests")]
    pub cab_requests: Vec<bool>,
}

impl ElevatorState {
    pub fn new(n_floors: u8) -> ElevatorState {
        ElevatorState {
            behaviour: Behaviour::Idle,
            floor: 0,
            direction: Direction::Stop,
            cab_requests: vec![false; n_floors as usize],
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ElevatorData {
    #[serde(skip_serializing, skip_deserializing)]
    pub version: u64,
    #[serde(rename = "hallRequests")]
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
}
