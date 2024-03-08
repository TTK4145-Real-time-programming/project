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

/**
 * Known bugs:
 *
 * - When obstruction is activated and deactivated, it stops the system.
 * - Doesn't stop when there is no orders at all and it's moving.
 *
 * Things that must be fixed:
 *
 */

/***************************************/
/*        3rd party libraries          */
/***************************************/
use driver_rust::elevio::elev::{CAB, DIRN_DOWN, DIRN_STOP, DIRN_UP, HALL_DOWN, HALL_UP};
use std::time::{Duration, Instant};
use crossbeam_channel as cbc;

/***************************************/
/*           Local modules             */
/***************************************/
use crate::config::ElevatorConfig;
use crate::shared::Behaviour::{DoorOpen, Idle, Moving};
use crate::shared::Direction::{Down, Stop, Up};
use crate::shared::{Direction, ElevatorState};

/***************************************/
/*               Enums                 */
/***************************************/
enum Event {
    FloorReached(u8),
    StopPressed,
}

/***************************************/
/*             Public API              */
/***************************************/
pub struct ElevatorFSM {
    // Hardware channels
    hw_motor_direction_tx: cbc::Sender<u8>,
    hw_floor_sensor_rx: cbc::Receiver<u8>,
    hw_door_light_tx: cbc::Sender<bool>,
    hw_obstruction_rx: cbc::Receiver<bool>,
    hw_stop_button_rx: cbc::Receiver<bool>,

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
    door_timer: Instant,
}

impl ElevatorFSM {
    pub fn new(
        config: &ElevatorConfig,

        hw_motor_direction_tx: cbc::Sender<u8>,
        hw_floor_sensor_rx: cbc::Receiver<u8>,
        hw_door_light_tx: cbc::Sender<bool>,
        hw_obstruction_rx: cbc::Receiver<bool>,
        hw_stop_button_rx: cbc::Receiver<bool>,

        fsm_hall_requests_rx: cbc::Receiver<Vec<Vec<bool>>>,
        fsm_cab_request_rx: cbc::Receiver<u8>,
        fsm_order_complete_tx: cbc::Sender<(u8, u8)>,
        fsm_state_tx: cbc::Sender<ElevatorState>,
        fsm_terminate_rx: cbc::Receiver<()>,
    ) -> ElevatorFSM {
        ElevatorFSM {
            hw_motor_direction_tx,
            hw_floor_sensor_rx,
            hw_door_light_tx,
            hw_obstruction_rx,
            hw_stop_button_rx,

            fsm_hall_requests_rx,
            fsm_cab_request_rx,
            fsm_order_complete_tx,
            fsm_state_tx,
            fsm_terminate_rx,
            
            hall_requests: vec![vec![false; 2]; config.n_floors as usize],
            state: ElevatorState::new(config.n_floors),
            n_floors: config.n_floors,
            obstruction: false,
            door_open_time: config.door_open_time,
            door_timer: Instant::now(),
        }
    }

    pub fn run(mut self) {
        // Find the initial floor
        let _ = self.hw_motor_direction_tx.send(DIRN_DOWN);

        // Main loop
        loop {
            cbc::select! {
                recv(self.hw_floor_sensor_rx) -> floor => {
                    match floor {
                        Ok(f) => self.handle_event(Event::FloorReached(f)),
                        Err(e) => {
                            eprintln!("ERROR - hw_floor_sensor_rx: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
                recv(self.fsm_hall_requests_rx) -> hall_requests => {
                    match hall_requests {
                        Ok(hall_requests) => {
                            self.hall_requests = hall_requests;
                        }
                        Err(e) => {
                            eprintln!("ERROR - fsm_hall_requests_rx: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
                recv(self.fsm_cab_request_rx) -> request => {
                    match request {
                        Ok(request) => {
                            self.state.cab_requests[request as usize] = true;
                        }
                        Err(e) => {
                            eprintln!("ERROR - fsm_cab_request_rx: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
                recv(self.hw_stop_button_rx) -> stop_button => {
                    match stop_button {
                        Ok(true) => self.handle_event(Event::StopPressed),
                        Ok(false) => (),
                        Err(e) => {
                            eprintln!("ERROR - hw_stop_button_rx: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
                recv(self.hw_obstruction_rx) -> obstruction => {
                    match obstruction {
                        Ok(value) => self.obstruction = value,
                        Err(e) => {
                            eprintln!("ERROR - hw_obstruction_rx: {}", e);
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
                            self.state.direction = self.choose_direction();
                            if self.state.direction != Stop {
                                self.state.behaviour = Moving;
                                let _ = self.hw_motor_direction_tx.send(self.state.direction.to_u8());
                            }
                        }
                        DoorOpen => {
                            if self.obstruction {
                                self.door_timer = Instant::now() + Duration::from_secs(self.door_open_time);
                            } else if self.door_timer <= Instant::now() {
                                let _ = self.hw_door_light_tx.send(false);
                                self.close_door();
                            }
                        }
                        Moving => (), // Should implement stop button logic here
                    }
                }
            }
        }
    }

    fn handle_event(&mut self, event: Event) {
        match event {
            Event::FloorReached(floor) => {
                self.state.floor = floor;

                // If orders at this floor, complete them and open the door
                if self.complete_orders() {
                    self.open_door();
                }
                // No orders at this floor, find next direction
                else {
                    self.state.direction = self.choose_direction();

                    if self.state.direction == Stop {
                        self.state.behaviour = Idle;
                        let _ = self
                            .hw_motor_direction_tx
                            .send(self.state.direction.to_u8());
                    } else {
                        self.state.behaviour = Moving;
                        let _ = self
                            .hw_motor_direction_tx
                            .send(self.state.direction.to_u8());
                    }
                }

                // Send new state to coordinator
                let _ = self.fsm_state_tx.send(self.state.clone());
            }
            Event::StopPressed => {
                // TBA ;)
            }
        }
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

    // Returns true if order has been completed
    fn complete_orders(&mut self) -> bool {
        let current_floor = self.state.floor;
        let is_top_floor = current_floor == self.n_floors - 1;
        let is_bottom_floor = current_floor == 0;
        let mut orders_completed = false;

        // Remove cab orders at current floor.
        if self.state.cab_requests[current_floor as usize] {
            // Open the door
            orders_completed = true;

            // Update the state and send it to the coordinator
            self.state.cab_requests[current_floor as usize] = false;
            self.fsm_order_complete_tx
                .send((current_floor, CAB))
                .unwrap();
        }
        // Remove hall up orders.
        if self.hall_requests[current_floor as usize][HALL_UP as usize]
        {
            // Open the door
            orders_completed = true;

            // Update the state and send it to the coordinator
            self.hall_requests[current_floor as usize][HALL_UP as usize] = false;
            self.fsm_order_complete_tx
                .send((current_floor, HALL_UP))
                .unwrap();
        }

        // Remove hall down orders.
        if self.hall_requests[current_floor as usize][HALL_DOWN as usize]
        {
            // Open the door
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
        let _ = self.hw_motor_direction_tx.send(DIRN_STOP); // Don't like having this here
        self.door_timer = Instant::now() + Duration::from_millis(self.door_open_time);
        self.state.behaviour = DoorOpen;
    }

    fn close_door(&mut self) {
        self.complete_orders();
        let _ = self.hw_door_light_tx.send(false);
        self.state.direction = self.choose_direction();
        let _ = self
            .hw_motor_direction_tx
            .send(self.state.direction.to_u8());
        self.state.behaviour = if self.state.direction == Stop {
            Idle
        } else {
            Moving
        };
        let _ = self.fsm_state_tx.send(self.state.clone());
    }

    // --------- Unused methods --------- //

    fn _should_stop(&self) -> bool {
        match self.state.direction {
            Up => {
                // Check for order at current floor
                if self.state.cab_requests[self.state.floor as usize]
                    || self.hall_requests[self.state.floor as usize][HALL_UP as usize]
                    || self.hall_requests[self.state.floor as usize][HALL_DOWN as usize]
                {
                    return true;
                }

                // Check if top floor is reached
                if self.state.floor == self.n_floors - 1 {
                    return false;
                }

                // Check for orders above current floor
                self.has_orders_in_direction(Up)
            }
            Down => {
                // Check for order at current floor
                if self.state.cab_requests[self.state.floor as usize]
                    || self.hall_requests[self.state.floor as usize][HALL_UP as usize]
                    || self.hall_requests[self.state.floor as usize][HALL_DOWN as usize]
                {
                    return true;
                }

                // Check if bottom floor is reached
                if self.state.floor == 0 {
                    return false;
                }

                // Check for orders below current floor
                self.has_orders_in_direction(Down)
            }
            Stop => true,
        }
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
        // Publicly expose the private fields for testing
        pub fn test_get_state(&self) -> &ElevatorState {
            &self.state
        }

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

        pub fn test_open_door(&mut self) {
            self.open_door()
        }
        
        pub fn test_close_door(&mut self) {
            self.close_door()
        }
        
        pub fn test_handle_event(&mut self, event: super::Event) {
            self.handle_event(event)
        }
    }
}