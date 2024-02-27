use driver_rust::elevio::elev::Elevator;
use driver_rust::elevio::elev::{CAB, DIRN_DOWN, DIRN_STOP, DIRN_UP, HALL_DOWN, HALL_UP};
use std::time::{Duration, Instant};
use elevator::fsm::Event;
use elevator::fsm::ElevatorFSM;
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;

type BooleanPair = [bool; 2];
type IdData = Vec<BooleanPair>;

#[derive(Deserialize, Debug)]
struct hallRequests(HashMap<String, IdData>);

fn deserialize(json_data: &str) -> Result<hallRequests, serde_json::Error> {
    serde_json::from_str(json_data)
}

fn getRequestType(pair: &BooleanPair) -> (u8, bool) {
    request_type = None;
    request = false;
    if pair[0] == 1 {
        request_type = HALL_UP;
        request = true;
    } else if pair[1] == 1 {
        request_type = HALL_DOWN;
        request = true;
    }
    return (request_type, request);
}

fn new_hall_requests(data: &hallRequests) {
    for (id, pairs) in data.O {
        if id = PREASSIGNED_ID { //trenger en måte å håndtere id-en til heisen på
            for pair in pairs {
                (request_type, request) = getRequestType(pair);
                floor = pairs.index(pair);
                if request {
                    Event::RequestReceived(floor, request_type);
                }
            }
        }
    }
}

//struktur for hvordan informasjonen inn kan se ut. Da spesifikt for en heis.
#[derive(Serialize, Deserialize, Debug)]
pub struct ElevatorState{
    behaviour: Behaviour,
    floor: Option<u8>,
    direction: Direction,
    cabRequests: [bool; 4],
}

#[derive(Serialize, Deserialize, Debug)]
type States = HashMap<String, ElevatorState>;

//Dette er det som blir sendt over network.
#[derive(Serialize, Deserialize, Debug)]
struct elevator_data{
    version: u64,
    hallRequests: Vec<BooleanPair>,
    states: States,
}

//Tar det fra network og gjør det til en struktur vi kan jobbe med.
fn deserialise_global_orderList(json_data: &str) -> Result<elevator_data, serde_json::Error> {
    serde_json::from_str(json_data)
}


fn update_global_orderlist(elevatorFSM: &ElevatorFSM, elevator_data: &elevator_data) -> Result<String, serde_json::Error> {
    let mut states = elevator_data.states;
    let mut version = elevator_data.version;
    let mut changed = false;
    for (id, state) in states {
        if id == PREASSIGNED_ID { //trenger en måte å håndtere id-en til heisen på
            let mut cabRequests = elevatorFSM.get_cab_requests(); 
            let mut floor = elevatorFSM.get_floor(); 
            let mut direction = elevatorFSM.get_direction(); 
            let mut behaviour = elevatorFSM.get_behaviour(); 
            let mut new_state = ElevatorState{behaviour, floor, direction, cabRequests};
            states[id] = new_state; 
            changed = true;
        }
    }
    if changed {
        version += 1;
    }
    let mut new_orderList = elevator_data{version: version, hallRequests: elevator_data.hallRequests, states};
    let mut new_elevator_data_json = serde_json::to_string(&new_orderList)?;
    return new_elevator_data_json;
}