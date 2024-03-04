use crate::config::ElevatorConfig;
use crate::shared_structs::ElevatorState;
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
 * - `hw_cab_request_rx`:       Receives cabin request inputs (e.g., buttons pressed inside the elevator).
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

enum Event {
    FloorReached(u8),
    StopPressed,
    DoorClosed,
}

pub struct ElevatorFSM {
    // Hardware channels
    hw_motor_direction_tx: cbc::Sender<u8>,
    hw_floor_sensor_rx: cbc::Receiver<u8>,
    hw_door_light_tx: cbc::Sender<bool>,
    hw_obstruction_rx: cbc::Receiver<bool>,
    hw_stop_button_rx: cbc::Receiver<bool>,
    hw_cab_request_rx: cbc::Receiver<Vec<bool>>,

    // Coordinator channels
    hall_request_rx: cbc::Receiver<Vec<Vec<bool>>>,
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
        hw_cab_request_rx: cbc::Receiver<Vec<bool>>,
        hall_request_rx: cbc::Receiver<Vec<Vec<bool>>>,
        complete_order_tx: cbc::Sender<(u8, u8)>,
        state_tx: cbc::Sender<ElevatorState>,
    ) -> ElevatorFSM {
        ElevatorFSM {
            hw_motor_direction_tx,
            hw_floor_sensor_rx,
            hw_door_light_tx,
            hw_obstruction_rx,
            hw_stop_button_rx,
            hw_cab_request_rx,
            hall_request_rx,
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
                        Err(e) => eprintln!("Error receiving from hw_floor_sensor_rx: {}", e),
                    }
                }
                recv(self.hall_request_rx) -> hall_requests => {
                    match hall_requests {
                        Ok(requests) => {
                            self.hall_requests = requests;
                            if self.state.behaviour == "idle" {
                                let next_direction = self.choose_direction(self.state.floor);
                                if next_direction != self.state.direction {
                                    self.state.direction = next_direction;
                                    let _ = self.hw_motor_direction_tx.send(next_direction);
                                }
                            }
                        }
                        Err(e) => eprintln!("Error receiving from hall_request_rx: {}", e),
                    }
                }
                recv(self.hw_cab_request_rx) -> cab_requests => {
                    match cab_requests {
                        Ok(requests) => {
                            self.state.cab_requests = requests;
                            if self.state.behaviour == "idle" {
                                let next_direction = self.choose_direction(self.state.floor);
                                if next_direction != self.state.direction {
                                    self.state.direction = next_direction;
                                    let _ = self.hw_motor_direction_tx.send(next_direction);
                                }
                            }
                        }
                        Err(e) => eprintln!("Error receiving from hw_cab_request_rx: {}", e),
                    }
                }
                recv(self.hw_stop_button_rx) -> stop_button => {
                    match stop_button {
                        Ok(true) => self.handle_event(Event::StopPressed),
                        Ok(false) => (),
                        Err(e) => eprintln!("Error receiving from hw_stop_button_rx: {}", e),
                    }
                }
                recv(self.hw_obstruction_rx) -> obstruction => {
                    match obstruction {
                        Ok(value) => self.obstruction = value,
                        Err(e) => eprintln!("Error receiving from hw_obstruction_rx: {}", e),
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

                // If orders at this floor, open the door and let the DoorClosed event handle the rest
                self.complete_orders(floor);

                // No orders at this floor, find next direction
                if !self.door_open {
                    self.state.direction = self.choose_direction(self.state.floor);
                    let _ = self.hw_motor_direction_tx.send(self.state.direction);
                }
            }
            Event::StopPressed => {
                // TBA ;)
            }
            Event::DoorClosed => {
                self.complete_orders(self.state.floor);
                self.state.direction = self.choose_direction(self.state.floor);
                let _ = self.hw_motor_direction_tx.send(self.state.direction);
            }
        }
    }

    fn choose_direction(&mut self, floor: u8) -> u8 {
        // Continue in current direction of travel if there are any further orders in that direction
        if self.has_orders_in_direction(floor, self.state.direction) {
            return self.state.direction;
        }

        // Otherwise change direction if there are orders in the opposite direction
        if self.state.direction == DIRN_UP && self.has_orders_in_direction(floor, DIRN_DOWN) {
            return DIRN_DOWN;
        } else if self.state.direction == DIRN_DOWN && self.has_orders_in_direction(floor, DIRN_UP)
        {
            return DIRN_UP;
        }

        // Start moving if necessary
        if self.state.direction == DIRN_STOP {
            if self.has_orders_in_direction(floor, DIRN_UP) {
                return DIRN_UP;
            } else if self.has_orders_in_direction(floor, DIRN_DOWN) {
                return DIRN_DOWN;
            }
        }

        // If there are no orders, stop.
        return DIRN_STOP;
    }

    fn has_orders_in_direction(&self, current_floor: u8, direction: u8) -> bool {
        match direction {
            // Check all orders above the current floor
            DIRN_UP => {
                for f in (current_floor + 1)..self.n_floors {
                    if self.state.cab_requests[f as usize]
                        || self.hall_requests[f as usize][HALL_UP as usize]
                        || self.hall_requests[f as usize][HALL_DOWN as usize]
                    {
                        return true;
                    }
                }
            }

            // Check all orders below the current floor
            DIRN_DOWN => {
                for f in (0..current_floor).rev() {
                    if self.state.cab_requests[f as usize]
                        || self.hall_requests[f as usize][HALL_UP as usize]
                        || self.hall_requests[f as usize][HALL_DOWN as usize]
                    {
                        return true;
                    }
                }
            }

            _ => {
                return false;
            }
        }

        return false;
    }

    fn complete_orders(&mut self, floor: u8) {
        let is_top_floor = floor == self.n_floors - 1;
        let is_bottom_floor = floor == 0;

        // Remove cab orders at current floor.
        if self.state.cab_requests[floor as usize] {
            // Open the door
            let _ = self.hw_door_light_tx.send(true);
            let _ = self.hw_motor_direction_tx.send(DIRN_STOP);
            self.door_open = true;

            // Update the state and send it to the coordinator
            self.state.cab_requests[floor as usize] = false;
            self.complete_order_tx.send((floor, CAB)).unwrap();
        }

        // Remove hall up orders.
        if (self.state.direction == DIRN_UP || self.state.direction == DIRN_STOP || is_bottom_floor)
            && self.hall_requests[floor as usize][HALL_UP as usize]
        {
            // Open the door
            let _ = self.hw_door_light_tx.send(true);
            let _ = self.hw_motor_direction_tx.send(DIRN_STOP);
            self.door_open = true;

            // Update the state and send it to the coordinator
            self.hall_requests[floor as usize][HALL_UP as usize] = false;
            self.complete_order_tx.send((floor, HALL_UP)).unwrap();
        }

        // Remove hall down orders.
        if (self.state.direction == DIRN_DOWN || self.state.direction == DIRN_STOP || is_top_floor)
            && self.hall_requests[floor as usize][HALL_DOWN as usize]
        {
            // Open the door
            let _ = self.hw_door_light_tx.send(true);
            let _ = self.hw_motor_direction_tx.send(DIRN_STOP);
            self.door_open = true;

            // Update the state and send it to the coordinator
            self.hall_requests[floor as usize][HALL_DOWN as usize] = false;
            self.complete_order_tx.send((floor, HALL_DOWN)).unwrap();
        }
    }
}
