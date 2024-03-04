use crate::config::HardwareConfig;
use crossbeam_channel as cbc;
use driver_rust::elevio::elev::Elevator;
use driver_rust::elevio::elev::{CAB, HALL_DOWN, HALL_UP};
use std::time::Duration;

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
 * - `hw_motor_direction_rx`:   Receiver for motor direction commands.
 * - `hw_button_light_rx`:      Receiver for button light control commands.
 * - `hw_hall_request_tx`:      Sender for hall request events.
 * - `hw_cab_request_tx`:       Sender for cabin request events.
 * - `hw_floor_sensor_tx`:      Sender for floor sensor events.
 * - `hw_door_light_rx`:        Receiver for door light control commands.
 * - `hw_obstruction_tx`:       Sender for obstruction events.
 * - `hw_stop_button_tx`:       Sender for stop button events.
 */

pub struct ElevatorDriver {
    elevator: Elevator,
    thread_sleep_time: u64,
    current_floor: u8,
    obstruction: bool,
    hw_motor_direction_rx: cbc::Receiver<u8>,
    hw_button_light_rx: cbc::Receiver<(u8, u8, bool)>,
    hw_hall_request_tx: cbc::Sender<(u8, u8)>,
    hw_cab_request_tx: cbc::Sender<Vec<bool>>,
    hw_floor_sensor_tx: cbc::Sender<u8>,
    hw_door_light_rx: cbc::Receiver<bool>,
    hw_obstruction_tx: cbc::Sender<bool>,
    hw_stop_button_tx: cbc::Sender<bool>,
}

impl ElevatorDriver {
    pub fn new(
        config: &HardwareConfig,
        hw_motor_direction_rx: cbc::Receiver<u8>,
        hw_button_light_rx: cbc::Receiver<(u8, u8, bool)>,
        hw_hall_request_tx: cbc::Sender<(u8, u8)>,
        hw_cab_request_tx: cbc::Sender<Vec<bool>>,
        hw_floor_sensor_tx: cbc::Sender<u8>,
        hw_door_light_rx: cbc::Receiver<bool>,
        hw_obstruction_tx: cbc::Sender<bool>,
        hw_stop_button_tx: cbc::Sender<bool>,
    ) -> ElevatorDriver {
        ElevatorDriver {
            elevator: Elevator::init(&config.driver_address, config.n_floors).unwrap(),
            thread_sleep_time: config.hw_thread_sleep_time,
            current_floor: u8::MAX,
            obstruction: false,
            hw_motor_direction_rx,
            hw_button_light_rx,
            hw_hall_request_tx,
            hw_cab_request_tx,
            hw_floor_sensor_tx,
            hw_door_light_rx,
            hw_obstruction_tx,
            hw_stop_button_tx,
        }
    }

    pub fn run(mut self) {
        loop {
            // Check if new floor is hit
            if let Some(floor) = self.elevator.floor_sensor() {
                if floor != self.current_floor {
                    self.current_floor = floor;
                    self.hw_floor_sensor_tx.send(floor).unwrap();
                }
            }

            // Check if stop button is pressed
            if self.elevator.stop_button() {
                self.hw_stop_button_tx.send(true).unwrap();
            }

            // Check if obstruction is toggled
            if self.elevator.obstruction() != self.obstruction {
                self.obstruction = !self.obstruction;
                self.hw_obstruction_tx.send(self.obstruction).unwrap();
            }

            // Check if any call buttons are pressed
            for floor in 0..self.elevator.num_floors {
                if self.elevator.call_button(floor, HALL_UP) {
                    self.hw_hall_request_tx.send((floor, 0)).unwrap();
                }
                if self.elevator.call_button(floor, HALL_DOWN) {
                    self.hw_hall_request_tx.send((floor, 1)).unwrap();
                }
                if self.elevator.call_button(floor, CAB) {
                    self.hw_cab_request_tx
                        .send(vec![true; self.elevator.num_floors as usize])
                        .unwrap();
                }
            }

            // Handle incoming events
            cbc::select! {
                recv(self.hw_motor_direction_rx) -> msg => {
                    self.elevator.motor_direction(msg.unwrap());
                }
                recv(self.hw_button_light_rx) -> msg => {
                    self.elevator.call_button_light(msg.unwrap().0, msg.unwrap().1, msg.unwrap().2);
                }
                recv(self.hw_door_light_rx) -> msg => {
                    self.elevator.door_light(msg.unwrap());
                }
                default(Duration::from_millis(self.thread_sleep_time)) => {}
            }
        }
    }
}
