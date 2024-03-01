use std::thread::sleep;
use std::time::Duration;

use crate::config::HardwareConfig;
use crossbeam_channel as cbc;
use driver_rust::elevio::elev::Elevator;

pub struct ElevatorDriver {
    elevator: Elevator,
    thread_sleep_time: u64,
    hw_motor_direction_rx: cbc::Receiver<u8>,
    hw_button_lights_rx: cbc::Receiver<(u8, u8, bool)>,
    hw_buttons_tx: cbc::Sender<(u8, u8)>,
    hw_floor_sensor_tx: cbc::Sender<u8>,
    hw_door_light_rx: cbc::Receiver<bool>,
    hw_obstruction_tx: cbc::Sender<bool>,
    hw_stop_button_tx: cbc::Sender<bool>,
}

impl ElevatorDriver {
    pub fn new(
        config: &HardwareConfig,
        hw_motor_direction_rx: cbc::Receiver<u8>,
        hw_button_lights_rx: cbc::Receiver<(u8, u8, bool)>,
        hw_buttons_tx: cbc::Sender<(u8, u8)>,
        hw_floor_sensor_tx: cbc::Sender<u8>,
        hw_door_light_rx: cbc::Receiver<bool>,
        hw_obstruction_tx: cbc::Sender<bool>,
        hw_stop_button_tx: cbc::Sender<bool>,
    ) -> ElevatorDriver {
        ElevatorDriver {
            elevator: Elevator::init(&config.driver_address, config.n_floors).unwrap(),
            thread_sleep_time: config.hw_thread_sleep_time,
            hw_motor_direction_rx,
            hw_button_lights_rx,
            hw_buttons_tx,
            hw_floor_sensor_tx,
            hw_door_light_rx,
            hw_obstruction_tx,
            hw_stop_button_tx,
        }
    }

    pub fn run(self) {

        // Handle incoming events
        cbc::select! {
            recv(self.hw_motor_direction_rx) -> msg => {
                self.elevator.motor_direction(msg.unwrap());
            }
            recv(self.hw_button_lights_rx) -> msg => {
                self.elevator.call_button_light(msg.unwrap().0, msg.unwrap().1, msg.unwrap().2);
            }
            recv(self.hw_door_light_rx) -> msg => {
                self.elevator.door_light(msg.unwrap());
            }
        }

        // Check for outgoing events
        if let Some(floor) = self.elevator.floor_sensor() {
            self.hw_floor_sensor_tx.send(floor).unwrap();
        }

        if self.elevator.stop_button() {
            self.hw_stop_button_tx.send(true).unwrap();
        }

        if self.elevator.obstruction() {
            self.hw_obstruction_tx.send(true).unwrap();
        }

        for floor in 0..self.elevator.num_floors {
            if self.elevator.call_button(floor, 0) {
                self.hw_buttons_tx.send((floor, 0)).unwrap();
            }
            if self.elevator.call_button(floor, 1) {
                self.hw_buttons_tx.send((floor, 1)).unwrap();
            }
        }

        sleep(Duration::from_millis(self.thread_sleep_time));
    }

}
