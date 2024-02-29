use driver_rust::elevio::elev::{CAB, HALL_DOWN, HALL_UP};
use std::collections::HashMap;
use driver_rust::elevio::elev::Elevator;
use serde::{Deserialize, Serialize};
use std::sync::mpsc;
use std::thread;
use std::sync::{Arc, Mutex};
use crate::elevator::fsm::Behaviour;
use crate::elevator::fsm::ElevatorFSM;

use crate::config;

/*  TODO:
    Alle data structure (structs) should be in own file?
    This data structure WILL be used by this module AND Netowrk
    This file should only contain Updatdata related implementation
*/

/*
    to run the rquest assigner. Call:
    let assigner = RequestAssigner::init(....)
    let assigner = Arc::new(Mutex::new(assigner))
    RequestAssigner::run(assigner.clone())

    This will ensure thread safty when having button and "main"-Request_Assigner thread running
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
pub struct RequestAssigner{
    elevator_data: ElevatorData, 
    local_elevator: Elevator,
    local_id: String,
    
    // Button thread variables
    button_tx: mpsc::Sender<GlobalEvent>,
    button_rx: Arc<Mutex<mpsc::Receiver<GlobalEvent>>>,
}


impl RequestAssigner{
    //Initilizing Request assigner strcuct and puts it in a thread (?)
    pub fn init(elevator_data: ElevatorData, local_id: String) -> Result<Self, std::io::Error>{
        
        //Making instance of elevator to read buttons
        let config = config::load_config();
        let elevator = Elevator::init(&config.elevator.driver_address, config.elevator.n_floors)?;

        //Making channel for button thread
        let (button_tx, button_rx) = mpsc::channel::<GlobalEvent>();
        let button_rx_shared = Arc::new(Mutex::new(button_rx));
        
        Ok(RequestAssigner{
            //num_floors: num_floors,
            elevator_data: elevator_data,
            local_elevator: elevator,
            local_id: local_id,

            //Button thread related atributes
            button_tx: button_tx,
            button_rx: button_rx_shared,
        })
    }
    
    // ---- main functions -----

    //Main run function
    pub fn run(assigner: Arc<Mutex<Self>>) { 
        //Spawning the button-thread to listen for button calls
        let tx_clone = {
            let locked_assigner = assigner.lock().unwrap();
            locked_assigner.button_tx.clone()
        };

        thread::spawn(move || {
            loop{
                let event = {
                    let locked_assigner = assigner.lock().unwrap();
                    locked_assigner.wait_for_button()

                    // TODO: Slow down this loop perhaps??
                };

                match event {
                    GlobalEvent::NoEvent => {
                        //Do nothing
                    },
                    //If other event transmit it
                    _ => {
                        tx_clone.send(event).expect("Failed to send event to Rewuest assigner thread")
                    }
                }
            }
        });

        // Add the wait_for_event here:
            // listen to three channels
            // Handle this based on the events
    }



    // ---- Extra functions -----

    pub fn send_to_fsm(&self, order_list: Vec<Vec<bool>>){

    }

    pub fn wait_for_event(&self){
        // Listen to:
            //network
                //peer
                //data
            
            //Fsm

            //buttons

        //based on this
          //merge/mergeconflict -> JSON -> HRA -> Network
          //Set lights
          //send to FSM
    }
    
    pub fn wait_for_button(&self) -> GlobalEvent{
        // Checking for all button presses and if they are already handled
        for floor in 0..self.local_elevator.num_floors {
            //Checking cab buttons 
            if !self.check_cab_button(floor) 
            && self.local_elevator.call_button(floor, CAB)
            {
                return GlobalEvent::newButtonRequest(floor, CAB);
            }

            //Checking hall buttons
            if !self.check_hall_button(floor, HALL_UP) 
            && self.local_elevator.call_button(floor, HALL_UP)
            {
                return GlobalEvent::newButtonRequest(floor, HALL_UP);
            }
            if !self.check_hall_button(floor, HALL_DOWN) 
            && self.local_elevator.call_button(floor, HALL_DOWN)
            {
                return GlobalEvent::newButtonRequest(floor, HALL_DOWN);
            }
        }

        return GlobalEvent::NoEvent;
    }

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
            // This should NEVER happen, implmented for cosmic bit-flip
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



