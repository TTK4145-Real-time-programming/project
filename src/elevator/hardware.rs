/**
 * # Elevator Driver
 * Represents an Elevator Driver that interfaces with the physical elevator hardware.
 *
 * This driver manages communication between the software and the physical elevator,
 * handling both incoming and outgoing requests such as elevator calls, motor direction changes,
 * and sensor events. It utilizes crossbeam channels for asynchronous communication with the
 * coordinator thread and fsm thread.
 *
 * # Fields
 *
 * - `elevator`:                Instance of `Elevator` for low-level hardware control.
 * - `thread_sleep_time`:       Duration in milliseconds the driver thread sleeps for in each loop iteration.
 * - `current_floor`:           The current floor the elevator is on.
 * - `obstruction`:             Whether the obstruction sensor is active. Used to only send changes over `hw_obstruction_tx`.
 * - `requests`:                A 2D vector representing the current state of the call buttons. Used to only send changes over `hw_request_tx`.
 * - `hw_motor_direction_rx`:   Receiver for motor direction commands.
 * - `hw_button_light_rx`:      Receiver for button light control commands.
 * - `hw_request_tx`:           Sender for request events.
 * - `hw_floor_sensor_tx`:      Sender for floor sensor events.
 * - `hw_door_light_rx`:        Receiver for door light control commands.
 * - `hw_obstruction_tx`:       Sender for obstruction events.
 * - `hw_stop_button_tx`:       Sender for stop button events.
 * - `terminate_rx`:            Receiver for termination signal.
 */

/***************************************/
/*        3rd party libraries          */
/***************************************/
use driver_rust::elevio::elev::{CAB, HALL_DOWN, HALL_UP};
use driver_rust::elevio::elev::Elevator;
use crossbeam_channel as cbc;
use std::time::Duration;

/***************************************/
/*           Local modules             */
/***************************************/
use crate::config::HardwareConfig;
use crate::unwrap_or_exit;

/***************************************/
/*             Constants               */
/***************************************/
const HW_NUM_REQUEST_TYPES: usize = 3;

/***************************************/
/*             Public API              */
/***************************************/
pub struct ElevatorDriver {
    elevator: Elevator,
    thread_sleep_time: u64,
    current_floor: u8,
    obstruction: bool,
    requests: Vec<Vec<bool>>,
    hw_motor_direction_rx: cbc::Receiver<u8>,
    hw_button_light_rx: cbc::Receiver<(u8, u8, bool)>,
    hw_request_tx: cbc::Sender<(u8, u8)>,
    hw_floor_sensor_tx: cbc::Sender<u8>,
    hw_door_light_rx: cbc::Receiver<bool>,
    hw_obstruction_tx: cbc::Sender<bool>,
    hw_stop_button_tx: cbc::Sender<bool>,
    terminate_rx: cbc::Receiver<()>,
}

impl ElevatorDriver {
    pub fn new(
        config: &HardwareConfig,
        hw_motor_direction_rx: cbc::Receiver<u8>,
        hw_button_light_rx: cbc::Receiver<(u8, u8, bool)>,
        hw_request_tx: cbc::Sender<(u8, u8)>,
        hw_floor_sensor_tx: cbc::Sender<u8>,
        hw_door_light_rx: cbc::Receiver<bool>,
        hw_obstruction_tx: cbc::Sender<bool>,
        hw_stop_button_tx: cbc::Sender<bool>,
        terminate_rx: cbc::Receiver<()>,
    ) -> ElevatorDriver {
        ElevatorDriver {
            elevator: unwrap_or_exit!(Elevator::init(&config.driver_address, config.n_floors)),
            thread_sleep_time: config.hw_thread_sleep_time,
            current_floor: u8::MAX,
            obstruction: false,
            requests: vec![vec![false; HW_NUM_REQUEST_TYPES]; config.n_floors as usize],
            hw_motor_direction_rx,
            hw_button_light_rx,
            hw_request_tx,
            hw_floor_sensor_tx,
            hw_door_light_rx,
            hw_obstruction_tx,
            hw_stop_button_tx,
            terminate_rx,
        }
    }

    pub fn run(mut self) {
        // Reset system
        for floor in 0..self.elevator.num_floors {
            self.elevator.call_button_light(floor, HALL_UP, false);
            self.elevator.call_button_light(floor, HALL_DOWN, false);
            self.elevator.call_button_light(floor, CAB, false);
        }
        self.obstruction = self.elevator.obstruction();

        // Main loop
        loop {
            // Check if new floor is hit
            if let Some(floor) = self.elevator.floor_sensor() {
                if floor != self.current_floor {
                    self.current_floor = floor;
                    match self.hw_floor_sensor_tx.send(floor) {
                        Ok(_) => {}
                        Err(e) => {
                            eprintln!("ERROR - hw_floor_sensor_tx: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
            }

            // Check if stop button is pressed
            if self.elevator.stop_button() {
                unwrap_or_exit!(self.hw_stop_button_tx.send(true));
            }

            // Check if obstruction is toggled
            if self.elevator.obstruction() != self.obstruction {
                self.obstruction = !self.obstruction;
                unwrap_or_exit!(self.hw_obstruction_tx.send(self.obstruction));
            }

            // Check if any call buttons are pressed
            for floor in 0..self.elevator.num_floors {
                if !self.requests[floor as usize][HALL_UP as usize]
                    && self.elevator.call_button(floor, HALL_UP)
                {
                    self.requests[floor as usize][HALL_UP as usize] = true;
                    unwrap_or_exit!(self.hw_request_tx.send((floor, HALL_UP)));
                }
                if !self.requests[floor as usize][HALL_DOWN as usize]
                    && self.elevator.call_button(floor, HALL_DOWN)
                {
                    self.requests[floor as usize][HALL_DOWN as usize] = true;
                    unwrap_or_exit!(self.hw_request_tx.send((floor, HALL_DOWN)));
                }
                if !self.requests[floor as usize][CAB as usize]
                    && self.elevator.call_button(floor, CAB)
                {
                    self.requests[floor as usize][CAB as usize] = true;
                    unwrap_or_exit!(self.hw_request_tx.send((floor, CAB)));
                }
            }

            // Handle incoming events
            cbc::select! {
                recv(self.hw_motor_direction_rx) -> msg => {
                    match msg {
                        Ok(msg) => self.elevator.motor_direction(msg),
                        Err(e) => {
                            eprintln!("ERROR - hw_motor_direction_rx: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
                recv(self.hw_button_light_rx) -> msg => {
                    match msg {
                        Ok(msg) => {
                            self.elevator.call_button_light(msg.0, msg.1, msg.2);  // Turn off button lamp
                            self.requests[msg.0 as usize][msg.1 as usize] = msg.2; // Make new calls possible
                        }
                        Err(e) => {
                            eprintln!("ERROR - hw_button_light_rx: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
                recv(self.hw_door_light_rx) -> msg => {
                    match msg {
                        Ok(msg) => self.elevator.door_light(msg),
                        Err(e) => {
                            eprintln!("ERROR - hw_door_light_rx: {}", e);
                            std::process::exit(1);
                        }
                    }

                }
                recv(self.terminate_rx) -> _ => {
                    break;
                }
                default(Duration::from_millis(self.thread_sleep_time)) => {}
            }
        }
    }
}
