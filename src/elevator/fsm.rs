/**
 * Manages elevator operation logic.
 *
 * The `ElevatorFSM` (Finite State Machine) controls the elevator's behavior by processing events such as floor requests,
 * door operations, and sensor inputs. It communicates with elevator hardware and coordinator thread.
 *
 * # Fields
 * - `hw_motor_direction_tx`:   Sends motor direction commands (up, down, stop).
 * - `hw_floor_sensor_rx`:      Receives current floor updates from the elevator sensor.
 * - `hw_door_light_tx`:        Controls the door's open/close light indicator.
 * - `hw_obstruction_rx`:       Receives obstruction detection signals (e.g., if something blocks the door).
 * - `hw_stop_button_rx`:       Receives stop button press signals.
 * - `fsm_cab_request_rx`:      Receives cabin request inputs (e.g., buttons pressed inside the elevator).
 * - `fsm_hall_requests_rx`:    Receives hall request inputs (e.g., buttons pressed on each floor).
 * - `fsm_order_complete_tx`:   Sends notifications when a request is completed.
 * - `fsm_state_tx`:            Broadcasts the current state of the elevator (e.g., current floor, direction).
 * - `hall_requests`:           Stores the state of hall requests (up/down) for each floor.
 * - `state`:                   Maintains the current state of the elevator (e.g., floor, direction).
 * - `n_floors`:                The total number of floors serviced by the elevator.
 * - `obstruction`:             Indicates if there is an obstruction detected by the elevator.
 * - `door_open_time`:          Configurable time for how long the door remains open.
 * - `door_timer`:              Timer used to track door open duration.
 *
 */

/***************************************/
/*              libraries              */
/***************************************/
use driver_rust::elevio::elev::{HALL_UP, HALL_DOWN, CAB};
use std::time::{Duration, Instant};
use crossbeam_channel as cbc;
use log::{info, error};


/***************************************/
/*           Local modules             */
/***************************************/
use crate::config::ElevatorConfig;
use crate::shared::Behaviour::{DoorOpen, Idle, Moving, Error};
use crate::shared::Direction::{Down, Stop, Up};
use crate::shared::{Direction, ElevatorState};
use crate::elevator::cab_orders::{load_cab_orders, save_cab_orders};


/***************************************/
/*             Public API              */
/***************************************/
pub struct ElevatorFSM {
    // Hardware channels
    hw_motor_direction_tx: cbc::Sender<u8>,
    hw_floor_sensor_rx: cbc::Receiver<u8>,
    hw_floor_indicator_tx: cbc::Sender<u8>,
    hw_door_light_tx: cbc::Sender<bool>,
    hw_obstruction_rx: cbc::Receiver<bool>,

    // Coordinator channels
    fsm_hall_requests_rx: cbc::Receiver<Vec<Vec<bool>>>,
    fsm_cab_request_rx: cbc::Receiver<u8>,
    fsm_order_complete_tx: cbc::Sender<(u8, u8)>,
    fsm_state_tx: cbc::Sender<ElevatorState>,

    // Private fields
    fsm_terminate_rx: cbc::Receiver<()>,
    hall_requests: Vec<Vec<bool>>,
    state: ElevatorState,
    n_floors: u8,
    obstruction: bool,
    door_open_time: u64,
    motor_timeout: u64,
    door_timeout: u64,
    door_timer: Instant,
    obstruction_timer: Instant,
    motor_timer: Instant,
}

impl ElevatorFSM {
    pub fn new(
        fsm_config: &ElevatorConfig,

        hw_motor_direction_tx: cbc::Sender<u8>,
        hw_floor_sensor_rx: cbc::Receiver<u8>,
        hw_floor_indicator_tx: cbc::Sender<u8>,
        hw_door_light_tx: cbc::Sender<bool>,
        hw_obstruction_rx: cbc::Receiver<bool>,

        fsm_hall_requests_rx: cbc::Receiver<Vec<Vec<bool>>>,
        fsm_cab_request_rx: cbc::Receiver<u8>,
        fsm_order_complete_tx: cbc::Sender<(u8, u8)>,
        fsm_state_tx: cbc::Sender<ElevatorState>,
        fsm_terminate_rx: cbc::Receiver<()>,
    ) -> ElevatorFSM {
        ElevatorFSM {
            hw_motor_direction_tx,
            hw_floor_sensor_rx,
            hw_floor_indicator_tx,
            hw_door_light_tx,
            hw_obstruction_rx,

            fsm_hall_requests_rx,
            fsm_cab_request_rx,
            fsm_order_complete_tx,
            fsm_state_tx,
            fsm_terminate_rx,
            
            hall_requests: vec![vec![false; 2]; fsm_config.n_floors as usize],
            state: ElevatorState::new(fsm_config.n_floors),
            n_floors: fsm_config.n_floors,
            obstruction: false,
            door_open_time: fsm_config.door_open_time,
            door_timeout: fsm_config.door_timeout,
            motor_timeout: fsm_config.motor_timeout,
            obstruction_timer: Instant::now(),
            door_timer: Instant::now(),
            motor_timer: Instant::now(),
        }
    }

    pub fn run(mut self) {
        // Find the initial floor
        let _ = self.hw_motor_direction_tx.send(Direction::Down.to_u8());
        self.load_saved_cab_calls();

        // Main loop
        loop {
            cbc::select! {
                recv(self.hw_floor_sensor_rx) -> new_floor => {
                    match new_floor {
                        Ok(floor) => self.handle_floor_hit(floor),
                        Err(error) => {
                            error!("ERROR - hw_floor_sensor_rx: {}", error);
                            std::process::exit(1);
                        }
                    }
                }
                recv(self.fsm_hall_requests_rx) -> hall_requests => {
                    match hall_requests {
                        Ok(hall_requests) => {
                            self.hall_requests = hall_requests;
                        }
                        Err(error) => {
                            error!("ERROR - fsm_hall_requests_rx: {}", error);
                            std::process::exit(1);
                        }
                    }
                }
                recv(self.fsm_cab_request_rx) -> new_cab_request => {
                    match new_cab_request {
                        Ok(new_cab_request) => {
                            self.state.cab_requests[new_cab_request as usize] = true;
                            save_cab_orders(self.state.cab_requests.clone());
                            let _ = self.fsm_state_tx.send(self.state.clone());
                        }
                        Err(error) => {
                            error!("ERROR - fsm_cab_request_rx: {}", error);
                            std::process::exit(1);
                        }
                    }
                }
                recv(self.hw_obstruction_rx) -> obstruction => {
                    match obstruction {
                        Ok(value) => {
                            self.obstruction = value;
                            if !value {
                                self.reset_obstruction_timer();
                            }
                        }
                        Err(error) => {
                            error!("ERROR - hw_obstruction_rx: {}", error);
                            std::process::exit(1);
                        }
                    }
                }
                recv(self.fsm_terminate_rx) -> _ => {
                    break;
                }
                default(Duration::from_millis(100)) => {
                    match self.state.behaviour {
                        Idle => {
                            if self.complete_orders() {
                                self.open_door();
                            }

                            self.state.direction = self.choose_direction();
                            if self.state.direction != Stop && self.state.behaviour != DoorOpen {
                                self.state.behaviour = Moving;
                                let _ = self.hw_motor_direction_tx.send(self.state.direction.to_u8());
                                self.reset_motor_timer();
                            }
                        }
                        DoorOpen => {
                            if self.obstruction {
                                self.reset_door_timer();

                                if self.obstruction_timer <= Instant::now() {
                                    info!("Elevator Error: Door timeout. Re-assigning hall requests.");
                                    self.state.behaviour = Error;
                                    let _ = self.fsm_state_tx.send(self.state.clone());
                                }

                            } else if self.door_timer <= Instant::now() {
                                self.close_door();
                                
                                self.state.direction = self.choose_direction();
                                if self.complete_orders() {
                                    self.open_door();
                                }

                                else {
                                    let _ = self.hw_motor_direction_tx.send(self.state.direction.to_u8());
    
                                    if self.state.direction == Stop {
                                        self.state.behaviour = Idle;
                                    }
                                    
                                    else {
                                        self.state.behaviour = Moving;
                                        self.reset_motor_timer();
                                    }
                                }
                                
                                let _ = self.fsm_state_tx.send(self.state.clone());
                            } 
                        }
                        Moving => {
                            if self.motor_timer <= Instant::now() && self.state.behaviour != Error {
                                
                                // Disconnecting elevator from network
                                info!("Motor Loss elevator!");
                                self.state.behaviour = Error;
                                let _ = self.fsm_state_tx.send(self.state.clone());

                                //Trying to start up motor
                                let _ = self.hw_motor_direction_tx.send(self.state.direction.to_u8());
                            }
                        }
                        Error => {
                            if self.obstruction_timer > Instant::now() {
                                self.open_door();
                                info!("Door closing!");
                            } 
                        }
                    }
                }
            }
        }
    }

    fn handle_floor_hit(&mut self, floor: u8) {
        if self.state.behaviour == Error{
            info!("Motor power restored. Elevator back in normal state.");
        }

        self.state.floor = floor;
        self.hw_floor_indicator_tx.send(floor).unwrap();

        // If orders at this floor, complete them, stop and open the door
        if self.complete_orders() {
            let _ = self.hw_motor_direction_tx.send(Direction::Stop.to_u8());
            self.open_door();
        }

        // Find next direction, and check if there are any orders
        else {
            self.state.direction = self.choose_direction();

            if self.complete_orders() {
                let _ = self.hw_motor_direction_tx.send(Direction::Stop.to_u8());
                self.open_door();
            }

            else if self.state.direction == Stop {
                self.state.behaviour = Idle;
                let _ = self.hw_motor_direction_tx.send(self.state.direction.to_u8());
            } 
            
            else {
                self.state.behaviour = Moving;
                let _ = self.hw_motor_direction_tx.send(self.state.direction.to_u8());
                self.reset_motor_timer();
            }
        }

        // Send new state to coordinator
        let _ = self.fsm_state_tx.send(self.state.clone());
    }

    fn choose_direction(&self) -> Direction {
        let current_direction = self.state.direction.clone();
        // Continue in current direction of travel if there are any further orders in that direction
        if self.has_orders_in_direction(current_direction.clone()) {
            return current_direction;
        }

        // Otherwise change direction if there are orders in the opposite direction
        if current_direction == Up && self.has_orders_in_direction(Down) {
            return Down;
        }
        if current_direction == Down && self.has_orders_in_direction(Up) {
            return Up;
        }

        // Start moving if necessary
        if current_direction == Stop {
            if self.has_orders_in_direction(Up) {
                return Up;
            }
            if self.has_orders_in_direction(Down) {
                return Down;
            }
        }

        // If there are no orders, stop.
        Stop
    }

    fn has_orders_in_direction(&self, direction: Direction) -> bool {
        match direction {
            // Check all orders above the current floor
            Up => {
                for f in (self.state.floor + 1)..self.n_floors {
                    if self.state.cab_requests[f as usize]
                        || self.hall_requests[f as usize][HALL_UP as usize]
                        || self.hall_requests[f as usize][HALL_DOWN as usize]
                    {
                        return true;
                    }
                }
            }

            // Check all orders below the current floor
            Down => {
                for f in (0..self.state.floor).rev() {
                    if self.state.cab_requests[f as usize]
                        || self.hall_requests[f as usize][HALL_UP as usize]
                        || self.hall_requests[f as usize][HALL_DOWN as usize]
                    {
                        return true;
                    }
                }
            }

            // No direction specified
            _ => {
                return false;
            }
        }

        false
    }

    fn reset_motor_timer(&mut self) {
        self.motor_timer = Instant::now() + Duration::from_millis(self.motor_timeout);
    }

    fn reset_door_timer(&mut self) {
        self.door_timer = Instant::now() + Duration::from_millis(self.door_open_time);
    }

    fn reset_obstruction_timer(&mut self) {
        self.obstruction_timer = Instant::now() + Duration::from_millis(self.door_timeout);
    }

    // Returns true if order has been completed
    fn complete_orders(&mut self) -> bool {

        // Floor specific variables
        let current_floor = self.state.floor;
        let is_top_floor = current_floor == self.n_floors - 1;
        let is_bottom_floor = current_floor == 0;

        // Order specific variables
        let cab_at_current_floor = self.state.cab_requests[current_floor as usize];
        let hall_up_at_current_floor = self.hall_requests[current_floor as usize][HALL_UP as usize];
        let hall_down_at_current_floor = self.hall_requests[current_floor as usize][HALL_DOWN as usize];

        // State specific variables
        let current_direction = self.state.direction.clone();
        let current_behaviour = self.state.behaviour.clone();
        let mut orders_completed = false;

        // Remove cab orders at current floor.
        if cab_at_current_floor {
            orders_completed = true;
            
            // Update the state and send it to the coordinator
            self.state.cab_requests[current_floor as usize] = false;
            self.fsm_order_complete_tx
            .send((current_floor, CAB))
            .unwrap();

            //Saving to cab order change to file
            save_cab_orders(self.state.cab_requests.clone());
        }

        // Remove hall up orders if moving up, stopped or at bottom floor
        if hall_up_at_current_floor && (current_direction == Up || is_bottom_floor || current_behaviour == Idle) {
            orders_completed = true;

            // Update the state and send it to the coordinator
            self.hall_requests[current_floor as usize][HALL_UP as usize] = false;
            self.fsm_order_complete_tx
                .send((current_floor, HALL_UP))
                .unwrap();
        }

        // Remove hall down orders if moving down, stopped or at top floor
        if hall_down_at_current_floor && (current_direction == Down || is_top_floor || current_behaviour == Idle) {
            orders_completed = true;

            // Update the state and send it to the coordinator
            self.hall_requests[current_floor as usize][HALL_DOWN as usize] = false;
            self.fsm_order_complete_tx
                .send((current_floor, HALL_DOWN))
                .unwrap();
        }

        orders_completed
    }

    fn open_door(&mut self) {
        let _ = self.hw_door_light_tx.send(true);
        self.reset_door_timer();
        self.reset_obstruction_timer();
        self.state.behaviour = DoorOpen;
        let _ = self.fsm_state_tx.send(self.state.clone());
    }

    fn close_door(&mut self) {
        let _ = self.hw_door_light_tx.send(false);
    }

    // Handles saved cab calls 
    fn load_saved_cab_calls(&mut self) {
        //Setting cab orders from file to elevatorData
        self.state.cab_requests = load_cab_orders().cab_calls;
        
        // Updating coordinator with the init state
        let _ = self.fsm_state_tx.send(self.state.clone());
    }
}

/***************************************/
/*              Test API               */
/***************************************/
#[cfg(test)]
pub mod testing {
    use crate::ElevatorState;
    use super::ElevatorFSM;

    impl ElevatorFSM {
        pub fn test_set_hall_requests(&mut self, hall_requests: Vec<Vec<bool>>) {
            self.hall_requests = hall_requests;
        }

        pub fn test_set_state(&mut self, state: ElevatorState) {
            self.state = state;
        }

        pub fn test_choose_direction(&self) -> super::Direction {
            self.choose_direction()
        }

        pub fn test_has_orders_in_direction(&self, direction: super::Direction) -> bool {
            self.has_orders_in_direction(direction)
        }

        pub fn test_complete_orders(&mut self) -> bool {
            self.complete_orders()
        }
        
    }
}