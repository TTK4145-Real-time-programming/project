use crate::config::ElevatorConfig;
use crate::shared_structs::{ElevatorState, Direction, Behaviour};
use crate::shared_structs::Direction::{Down, Stop, Up};
use crate::shared_structs::Behaviour::{Idle, Moving, DoorOpen};
use crossbeam_channel as cbc;
use driver_rust::elevio::elev::{CAB, DIRN_DOWN, DIRN_STOP, DIRN_UP, HALL_DOWN, HALL_UP};
use std::time::{Duration, Instant};

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
 * - `request_rx`:       Receives cabin request inputs (e.g., buttons pressed inside the elevator).
 * - `hall_request_rx`:         Receives hall request inputs (e.g., buttons pressed on each floor).
 * - `complete_order_tx`:       Sends notifications when a request is completed.
 * - `state_tx`:                Broadcasts the current state of the elevator (e.g., current floor, direction).
 * - `hall_requests`:           Stores the state of hall requests (up/down) for each floor.
 * - `state`:                   Maintains the current state of the elevator (e.g., floor, direction).
 * - `n_floors`:                The total number of floors serviced by the elevator.
 * - `door_open`:               Indicates whether the elevator door is currently open.
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
  */

enum Event {
    FloorReached(u8),
    RequestReceived(u8, u8),
    StopPressed,
    OpenDoor,
    DoorClosed,
}

pub struct ElevatorFSM {
    // Hardware channels
    hw_motor_direction_tx: cbc::Sender<u8>,
    hw_floor_sensor_rx: cbc::Receiver<u8>,
    hw_door_light_tx: cbc::Sender<bool>,
    hw_obstruction_rx: cbc::Receiver<bool>,
    hw_stop_button_rx: cbc::Receiver<bool>,
    
    // Coordinator channels
    request_rx: cbc::Receiver<(u8, u8)>,
    complete_order_tx: cbc::Sender<(u8, u8)>,
    state_tx: cbc::Sender<ElevatorState>,

    // Private fields
    hall_requests: Vec<Vec<bool>>,
    state: ElevatorState,
    n_floors: u8,
    door_open: bool,
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
        request_rx: cbc::Receiver<(u8, u8)>,
        complete_order_tx: cbc::Sender<(u8, u8)>,
        state_tx: cbc::Sender<ElevatorState>,
    ) -> ElevatorFSM {
        ElevatorFSM {
            hw_motor_direction_tx,
            hw_floor_sensor_rx,
            hw_door_light_tx,
            hw_obstruction_rx,
            hw_stop_button_rx,
            request_rx,
            complete_order_tx,
            state_tx,
            hall_requests: vec![vec![false; 2]; config.n_floors as usize],
            state: ElevatorState::new(config.n_floors),
            n_floors: config.n_floors,
            door_open: false,
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
                            eprintln!("Error receiving from hw_floor_sensor_rx: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
                recv(self.request_rx) -> request => {
                    match request {
                        Ok(request) => {
                            self.handle_event(Event::RequestReceived(request.0, request.1));
                        }
                        Err(e) => {
                            eprintln!("Error receiving from request_rx: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
                recv(self.hw_stop_button_rx) -> stop_button => {
                    match stop_button {
                        Ok(true) => self.handle_event(Event::StopPressed),
                        Ok(false) => (),
                        Err(e) => {
                            eprintln!("Error receiving from hw_stop_button_rx: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
                recv(self.hw_obstruction_rx) -> obstruction => {
                    match obstruction {
                        Ok(value) => self.obstruction = value,
                        Err(e) => {
                            eprintln!("Error receiving from hw_obstruction_rx: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
                default(Duration::from_millis(100)) => {
                    if self.door_open {
                        if self.obstruction {
                            self.door_timer = Instant::now() + Duration::from_secs(self.door_open_time);
                        } else if self.door_timer <= Instant::now() {
                            let _ = self.hw_door_light_tx.send(false);
                            self.door_open = false;
                            self.handle_event(Event::DoorClosed);
                        }
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
                    self.handle_event(Event::OpenDoor);
                }

                // No orders at this floor, find next direction
                else {
                    self.state.direction = self.choose_direction();

                    if self.state.direction == Stop {
                        self.state.behaviour = Idle;
                        let _ = self.hw_motor_direction_tx.send(self.state.direction.to_u8());
                    } 
                    
                    else {
                        self.state.behaviour = Moving;
                        let _ = self.hw_motor_direction_tx.send(self.state.direction.to_u8());
                    }
                }

                // Send new state to coordinator
                let _ = self.state_tx.send(self.state.clone());

            }
            Event::RequestReceived(floor, request_type) => {
                if request_type == CAB {
                    self.state.cab_requests[floor as usize] = true;
                    let _ = self.state_tx.send(self.state.clone()); // Notify coordinator
                } 
                
                else {
                    self.hall_requests[floor as usize][request_type as usize] = true;
                }

                if self.state.behaviour == Idle {
                    // Handle the request immediately if the elevator is idle
                    self.handle_event(Event::FloorReached(self.state.floor));
                }
            }
            Event::OpenDoor => {
                self.open_door();
            }
            Event::StopPressed => {
                // TBA ;)
            }
            Event::DoorClosed => {
                self.complete_orders();
                self.state.direction = self.choose_direction();
                let _ = self.hw_motor_direction_tx.send(self.state.direction.to_u8());
                self.state.behaviour = if self.state.direction == Stop { Idle } else { Moving };
                let _ = self.state_tx.send(self.state.clone());
            }
        }
    }

    fn choose_direction(&self) -> Direction {
        // Continue in current direction of travel if there are any further orders in that direction
        if self.has_orders_in_direction(self.state.direction.clone()) {
            return self.state.direction.clone();
        }

        // Otherwise change direction if there are orders in the opposite direction
        if self.state.direction == Up && self.has_orders_in_direction(Down) {
            return Down;
        } else if self.state.direction == Down && self.has_orders_in_direction(Up)
        {
            return Up;
        }

        // Start moving if necessary
        if self.state.direction == Stop {
            if self.has_orders_in_direction(Up) {
                return Up;
            } else if self.has_orders_in_direction(Down) {
                return Down;
            }
        }

        // If there are no orders, stop.
        return Stop;
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

        return false;
    }

    // Unused, remove if not needed
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
                return self.has_orders_in_direction(Up);
                
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
                return self.has_orders_in_direction(Down);

            }
            Stop => {
                return true;
            }
        }
    }

    // Returns true if order has been completed
    fn complete_orders(&mut self) -> bool {
        let is_top_floor = self.state.floor == self.n_floors - 1;
        let is_bottom_floor = self.state.floor == 0;
        let mut orders_completed = false;

        // Remove cab orders at current floor.
        if self.state.cab_requests[self.state.floor as usize] {
            // Open the door
            orders_completed = true;

            // Update the state and send it to the coordinator
            self.state.cab_requests[self.state.floor as usize] = false;
            self.complete_order_tx.send((self.state.floor, CAB)).unwrap();
        }
        // Remove hall up orders.
        if (self.state.direction.to_u8() == DIRN_UP || self.state.direction.to_u8() == DIRN_STOP || is_bottom_floor)
            && self.hall_requests[self.state.floor as usize][HALL_UP as usize]
        {
            // Open the door
            orders_completed = true;

            // Update the state and send it to the coordinator
            self.hall_requests[self.state.floor as usize][HALL_UP as usize] = false;
            self.complete_order_tx.send((self.state.floor, HALL_UP)).unwrap();
        }

        // Remove hall down orders.
        if (self.state.direction.to_u8() == DIRN_DOWN || self.state.direction.to_u8() == DIRN_STOP || is_top_floor)
            && self.hall_requests[self.state.floor as usize][HALL_DOWN as usize]
        {
            // Open the door
            orders_completed = true;

            // Update the state and send it to the coordinator
            self.hall_requests[self.state.floor as usize][HALL_DOWN as usize] = false;
            self.complete_order_tx.send((self.state.floor, HALL_DOWN)).unwrap();
        }
        return orders_completed;
    }

    fn open_door(&mut self) {
        let _ = self.hw_door_light_tx.send(true);
        let _ = self.hw_motor_direction_tx.send(DIRN_STOP); // Don't like having this here
        self.door_timer = Instant::now() + Duration::from_millis(self.door_open_time);
        self.door_open = true;
    }
}
