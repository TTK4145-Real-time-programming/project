use driver_rust::elevio::elev::{CAB, HALL_DOWN, HALL_UP};
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::elevator::fsm::Behaviour;
use crate::elevator::fsm::ElevatorFSM;

/*  TODO:
    Alle data structure (structs) should be in own file?
    This data structure WILL be used by this module AND Netowrk
    This file should only contain Updatdata related implementation
*/


// Defining events the thread will trigger on
pub enum GlobalEvent {
    newButtonRequest(u8, u8),
    Network,
    MergeNew,
    MergeConflict,
    NoEvent,
    NewElevator,
    DeadElavtor,
}

//Defning merge events
pub enum MergeEvent{
    Merge,
    MergeConflict,
    NoMerging,
}

//defining datatypes for the structs
type BooleanPair = [bool; 2];
enum Direction{
    Up,
    Down,
    Stop,
}

// Defining struct for Elevator state
pub struct ElevatorState{
    behaviour: Behaviour,
    floor: Option<u8>,
    direction: u8, 
    cab_requests: [bool; 4], //TODO: This need to defined from factory
}

// Combining Elevator state with ID
type States = HashMap<String, ElevatorState>;


// Adding each elevators to one struct 
pub struct ElevatorData{
    version: u64,
    hall_requests: Vec<BooleanPair>,
    states: States,
}


impl ElevatorData{
    //Initlizing status based local elevator
    pub fn init(floors: u8, id: String, elevator: &ElevatorFSM)-> Result<Self, std::io::Error>{
        //Adding local elevator to list
        let mut states = States::new();
        let elevator_state = ElevatorState{
            behaviour: elevator.get_behaviour(),
            floor: elevator.get_floor(),
            direction: elevator.get_direction(),
            cab_requests: [false; 4],
        };
        
        states.insert(id, elevator_state);
        
        // Constructing elevatorData
        Ok(ElevatorData{
            version: 0,
            hall_requests: vec![[false; 2]; floors.into()],
            states: states,
        })
    }
    
    //Adds new elevator when new elevator has appeard on network
    pub fn add_new_elevator(id: String){
        //Adds new elevator to States vector
    }

    // Removes elevator when elevator has dissapeared (gone SOLO)
    pub fn remove_elevator(id: String){
        //Removes a elevator from vec based on id
    }
}



// Request assigner 
//TODO: update_data creates ElevatorFSM and puts it in thread(?) -> Then it will have full ownership <-- More clean approach(?)
pub struct RequestAssigner<'a>{
    num_floors: u8,
    //event: GlobalEvent,
    //merge_conflict: MergeEvent,
    elevator_data: ElevatorData,
    local_elevator: &'a ElevatorFSM,
    local_id: String,
    
}


impl <'a>RequestAssigner<'a>{
    //Initilizing Request assigner strcuct and puts it in a thread (?)
    pub fn init(floors: u8, id: String, local_elevator: & 'a ElevatorFSM) -> Result<Self, std::io::Error>{
        // Initilizing the order book with local elevator
        let elevator_data = ElevatorData::init(floors, id.clone(), local_elevator)?;
        
        // Contructing Request assigner
        Ok(RequestAssigner{
            //event: GlobalEvent::NoEvent,
            //merge_conflict: MergeEvent::NoEvent,
            elevator_data: elevator_data,
            local_elevator: local_elevator,
            num_floors: floors,
            local_id: id,
        })
    }
    
    // ---- main functions -----
    
    pub fn wait_for_event(&self) -> GlobalEvent{

        // Checking for all button presses and if they are already handled
        for floor in 0..self.num_floors {
            //Checking cab buttons 
            if !self.check_cab_button(floor) 
            && self.local_elevator.get_elevator().call_button(floor, CAB)
            {
                return GlobalEvent::newButtonRequest(floor, CAB);
            }

            //Checking hall buttons
            if !self.check_hall_button(floor, HALL_UP) 
            && self.local_elevator.get_elevator().call_button(floor, HALL_UP)
            {
                return GlobalEvent::newButtonRequest(floor, HALL_UP);
            }
            if !self.check_hall_button(floor, HALL_DOWN) 
            && self.local_elevator.get_elevator().call_button(floor, HALL_DOWN)
            {
                return GlobalEvent::newButtonRequest(floor, HALL_DOWN);
            }
        }
        
        //Check if package from netowrk is newer than local ElevatorData
        

        return GlobalEvent::NoEvent;
    }



    // ---- Extra functions -----

    //Checks if cab button is already pressed (returns false if not pressed)
    fn check_cab_button(&self, floor: u8) -> bool{
        match self.elevator_data.states.get(&self.local_id) {
            Some(elevator_state) => {
                if !elevator_state.cab_requests[floor as usize] {
                    //Button has not been handled
                    return false;
                }
                else{
                    //Button has already been handled
                    return true;
                }
            },
            // This should NEVER happen
            None => {
                print!("Elevator with id: {} not found", self.local_id);
                return false;
            }
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

    // Checks if incommning version is newer than local version
    fn check_version(&self, version: u64) -> GlobalEvent{
        if version > self.elevator_data.version {
            return GlobalEvent::MergeNew;
        }
        else if version == self.elevator_data.version{
            return GlobalEvent::MergeConflict;
        }
        else{
            //TODO: If versions is older it will completly ignore it. Is this safe?
            return GlobalEvent::NoEvent;
        } 
    }
}



