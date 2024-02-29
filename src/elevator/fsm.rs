use crate::config::ElevatorConfig;
use crate::shared_structs::ElevatorState;
use crossbeam_channel as cbc;
use driver_rust::elevio::elev::Elevator;
use driver_rust::elevio::elev::{DIRN_DOWN, DIRN_STOP, DIRN_UP, HALL_DOWN, HALL_UP};
use std::sync::{Arc, Mutex};
use std::thread::spawn;
use std::time::{Duration, Instant};

enum Event {
    FloorReached(u8),
    StopPressed,
    DoorClosed,
}

pub struct ElevatorFSM {
    // Hardware driver
    pub elevator_driver: Arc<Mutex<Elevator>>,

    // Channels
    door_open_tx: Option<cbc::Sender<()>>,
    hall_request_rx: cbc::Receiver<Vec<Vec<bool>>>,
    complete_order_tx: cbc::Sender<(u8, u8)>,
    state_tx: cbc::Sender<ElevatorState>,

    // Private fields
    hall_requests: Vec<Vec<bool>>,
    state: ElevatorState,
    n_floors: u8,
    door_open: bool,
}

impl ElevatorFSM {
    pub fn new(
        config: &ElevatorConfig,
        hall_request_rx: cbc::Receiver<Vec<Vec<bool>>>,
        complete_order_tx: cbc::Sender<(u8, u8)>,
        state_tx: cbc::Sender<ElevatorState>,
    ) -> std::io::Result<ElevatorFSM> {
        // Initialize hardware driver
        let elevator_driver = Arc::new(Mutex::new(Elevator::init(
            &config.driver_address,
            config.n_floors,
        )?));

        Ok(ElevatorFSM {
            elevator_driver,
            hall_requests: vec![vec![false; 2]; config.n_floors as usize],
            state: ElevatorState::new(config.n_floors),
            n_floors: config.n_floors,
            door_open: false,
            door_open_tx: None,
            hall_request_rx,
            complete_order_tx,
            state_tx,
        })
    }

    pub fn run(&mut self) {
        // Channels
        let (floor_tx, floor_rx) = cbc::unbounded::<u8>();
        let (door_open_tx, door_open_rx) = cbc::unbounded::<()>();
        let (door_closed_tx, door_closed_rx) = cbc::unbounded::<()>();
        self.door_open_tx = Some(door_open_tx);

        // Floor sensor thread
        let elevator_driver = self.elevator_driver.clone();
        spawn(move || loop {
            {
                let elevator_driver = elevator_driver.lock().unwrap();
                if let Some(floor_sensor) = elevator_driver.floor_sensor() {
                    floor_tx.send(floor_sensor).unwrap();
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        });

        // Door thread
        let elevator_driver = self.elevator_driver.clone();
        spawn(move || loop {
            door_open_rx.recv().unwrap();
            let mut door_timer = Instant::now() + Duration::from_secs(3);
            loop {
                {
                    let elevator_driver = elevator_driver.lock().unwrap();
                    if elevator_driver.obstruction() {
                        door_timer = Instant::now() + Duration::from_secs(3);
                    } else if door_timer <= Instant::now() {
                        door_closed_tx.send(()).unwrap();
                        break;
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        });

        // Find the initial floor
        let elevator_driver = self.elevator_driver.clone();
        {
            let elevator_driver = elevator_driver.lock().unwrap();
            if let Some(floor_sensor) = elevator_driver.floor_sensor() {
                self.state.floor = floor_sensor;
            } else {
                elevator_driver.motor_direction(DIRN_UP);
            }
        }

        // Main loop
        loop {
            cbc::select! {
                recv(floor_rx) -> floor => {
                    self.handle_event(Event::FloorReached(floor.unwrap()));
                }
                recv(door_closed_rx) -> _ => {
                    self.handle_event(Event::DoorClosed);
                }
                recv(self.hall_request_rx) -> hall_requests => {
                    self.hall_requests = hall_requests.unwrap();
                }
                default(Duration::from_millis(100)) => {
                    let next_dir = self.choose_direction(self.state.floor);
                    if next_dir != self.state.direction {
                        self.state.direction = next_dir;
                        self.elevator_driver.lock().unwrap().motor_direction(next_dir);
                        self.state_tx.send(self.state.clone()).unwrap();
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
                    let next_direction = self.choose_direction(self.state.floor);
                    if next_direction != self.state.direction {
                        self.state.direction = next_direction;
                        self.elevator_driver
                            .lock()
                            .unwrap()
                            .motor_direction(next_direction);
                    }
                }
            }
            Event::StopPressed => {
                let elevator_driver = self.elevator_driver.lock().unwrap();
                elevator_driver.stop_button_light(true);
                elevator_driver.motor_direction(DIRN_STOP);
                while elevator_driver.stop_button() {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
                elevator_driver.stop_button_light(false);
                elevator_driver.motor_direction(self.state.direction);
            }
            Event::DoorClosed => {
                self.complete_orders(self.state.floor);
                let next_direction = self.choose_direction(self.state.floor);
                self.state.direction = next_direction;
                self.elevator_driver
                    .lock()
                    .unwrap()
                    .motor_direction(next_direction);
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
            self.door_open_tx
                .as_ref()
                .expect("Door channel not found!")
                .send(())
                .unwrap();
            self.door_open = true;

            // Update the state and send it to the coordinator
            self.state.cab_requests[floor as usize] = false;
            self.state_tx.send(self.state.clone()).unwrap();
        }

        // Remove hall up orders.
        if (self.state.direction == DIRN_UP || self.state.direction == DIRN_STOP || is_bottom_floor)
            && self.hall_requests[floor as usize][HALL_UP as usize]
        {
            // Open the door
            self.door_open_tx
                .as_ref()
                .expect("Door channel not found!")
                .send(())
                .unwrap();
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
            self.door_open_tx
                .as_ref()
                .expect("Door channel not found!")
                .send(())
                .unwrap();
            self.door_open = true;

            // Update the state and send it to the coordinator
            self.hall_requests[floor as usize][HALL_DOWN as usize] = false;
            self.complete_order_tx.send((floor, HALL_DOWN)).unwrap();
        }
    }
}
