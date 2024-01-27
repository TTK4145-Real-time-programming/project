/*
* This file contains the scheduler for the elevator
*/
use driver_rust::elevio::{elev, poll};

//Find orders above the elevator
fn request_Above(e: Elevator) -> bool{
    let floor = e.current_floor + 1;

    // Finding if there are orders obove
    for i in  floor..e.requests.len() {
        for j in 0..e.requests.len() {
            if(e.requests[i][j] == true){
                return true;
            }
        }
    }
    return false;
}

//Find orders below the elevator   
fn request_Below(e: Elevator) -> bool{
    let floor = e.current_floor - 1;

    // Finding if there are orders below
    for i in floor..0 {
        for j in 0..e.requests.len() {
            if(e.requests[i][j] == true){
                return true;
            }
        }
    }
    return false;
}

//Find if requests here
fn request_Here(e: Elevator) -> bool{
    let floor = e.current_floor;

    // Finding if there are orders here
    for i in 0..e.requests.len() {
        if(e.requests[floor][i] == true){
            return true;
        }
    }
    return false;
}

//Check if elevator should stop
fn should_stop(e: Elevator)-> bool{
    match e.direction {
        Direction::Up => {
            if(e.requests[e.current_floor][elev::HALL_UP] || e.requests[e.current_floor][elev::CAB] || !request_Above(e)){
                return true;
            }
        }
        Direction::Down => {
            if(e.requests[e.current_floor][elev::HALL_DOWN] || e.requests[e.current_floor][elev::CAB] || !request_Below(e)){
                return true;
            }
        }
        Direvtion::Stop => {
            return true;
        }
        _ => return true,
    }
}


//TODO: This function may be uselsss...
fn clear_request(e: Elevator, button_floor: i32) -> bool{
    return e.current_floor == butting_floor;
}


fn request_clear_current_floor(e: Elevator) ->Elevator{
    //Clear all order from current floor
    for i in 0..e.requests.len() {
        e.requests[e.current_floor][i] = false;
    }

    return e;
}

//Find the desired next direction
fn requests_chooseDirection(e: Elevator) -> DesiredDirection{
    match e.dirn {
        Direction::Up => {
            if requests_above(e) {
                DirnBehaviourPair { dirn: Direction::Up, behaviour: ElevatorBehaviour::Moving }
            } else if requests_here(e) {
                DirnBehaviourPair { dirn: Direction::Down, behaviour: ElevatorBehaviour::DoorOpen }
            } else if requests_below(e) {
                DirnBehaviourPair { dirn: Direction::Down, behaviour: ElevatorBehaviour::Moving }
            } else {
                DirnBehaviourPair { dirn: Direction::Stop, behaviour: ElevatorBehaviour::Idle }
            }
        },
        Direction::Down => {
            if requests_below(e) {
                DirnBehaviourPair { dirn: Direction::Down, behaviour: ElevatorBehaviour::Moving }
            } else if requests_here(e) {
                DirnBehaviourPair { dirn: Direction::Up, behaviour: ElevatorBehaviour::DoorOpen }
            } else if requests_above(e) {
                DirnBehaviourPair { dirn: Direction::Up, behaviour: ElevatorBehaviour::Moving }
            } else {
                DirnBehaviourPair { dirn: Direction::Stop, behaviour: ElevatorBehaviour::Idle }
            }
        },
        Direction::Stop => {
            if requests_here(e) {
                DirnBehaviourPair { dirn: Direction::Stop, behaviour: ElevatorBehaviour::DoorOpen }
            } else if requests_above(e) {
                DirnBehaviourPair { dirn: Direction::Up, behaviour: ElevatorBehaviour::Moving }
            } else if requests_below(e) {
                DirnBehaviourPair { dirn: Direction::Down, behaviour: ElevatorBehaviour::Moving }
            } else {
                DirnBehaviourPair { dirn: Direction::Stop, behaviour: ElevatorBehaviour::Idle }
            }
        },
        _ => DirnBehaviourPair { dirn: Direction::Stop, behaviour: ElevatorBehaviour::Idle },
    }
}



