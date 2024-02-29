use crate::config::ElevatorConfig;
use crate::shared_structs::ElevatorState;
use crossbeam_channel as cbc;
use driver_rust::elevio::elev::Elevator;
use driver_rust::elevio::elev::{DIRN_DOWN, DIRN_STOP, DIRN_UP, HALL_DOWN, HALL_UP};
use std::thread::spawn;
use std::time::{Duration, Instant};

enum Event {
    FloorReached(u8),
    StopPressed,
    DoorClosed,
}

pub struct ElevatorFSM {
    elevator: Elevator,
    hall_requests: Vec<Vec<bool>>,
    state: ElevatorState,
    door_open: bool,
    door_open_tx: cbc::Sender<()>,
    door_open_rx: cbc::Receiver<()>,
    door_closed_rx: cbc::Receiver<()>,
    door_closed_tx: cbc::Sender<()>,
    pub hall_request_tx: cbc::Sender<Vec<Vec<bool>>>,
    hall_request_rx: cbc::Receiver<Vec<Vec<bool>>>,
    complete_order_tx: cbc::Sender<(u8, u8)>,
    pub complete_order_rx: cbc::Receiver<(u8, u8)>,
    state_tx: cbc::Sender<ElevatorState>,
    pub state_rx: cbc::Receiver<ElevatorState>,
}

impl ElevatorFSM {
    pub fn new(config: &ElevatorConfig) -> std::io::Result<ElevatorFSM> {
        // Initialize hardware driver
        let elevator = Elevator::init(&config.driver_address, config.n_floors)?;

        // Initialize channels
        let (door_open_tx, door_open_rx) = cbc::unbounded::<()>();
        let (door_closed_tx, door_closed_rx) = cbc::unbounded::<()>();
        let (hall_request_tx, hall_request_rx) = cbc::unbounded::<Vec<Vec<bool>>>();
        let (complete_order_tx, complete_order_rx) = cbc::unbounded::<(u8, u8)>();
        let (state_tx, state_rx) = cbc::unbounded::<ElevatorState>();

        Ok(ElevatorFSM {
            elevator,
            hall_requests: vec![vec![false; 2]; config.n_floors as usize],
            state: ElevatorState::new(config.n_floors),
            door_open: false,
            door_open_tx,
            door_open_rx,
            door_closed_rx,
            door_closed_tx,
            hall_request_tx,
            hall_request_rx,
            complete_order_tx,
            complete_order_rx,
            state_tx,
            state_rx,
        })
    }

    pub fn run(&mut self) -> std::io::Result<()> {
        // Floor sensor thread
        let (floor_tx, floor_rx) = cbc::unbounded::<u8>();
        spawn(move || loop {
            if let Some(floor_sensor) = self.elevator.floor_sensor() {
                floor_tx.send(floor_sensor).unwrap();
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        });

        // Door thread
        let (door_open_tx, door_open_rx) = cbc::unbounded::<()>();
        let (door_closed_tx, door_closed_rx) = cbc::unbounded::<()>();
        spawn(move || loop {
            door_open_rx.recv().unwrap();
            let mut door_timer = Instant::now() + Duration::from_secs(3);
            loop {
                if self.elevator.obstruction() {
                    door_timer = Instant::now() + Duration::from_secs(3);
                } else if door_timer <= Instant::now() {
                    door_closed_tx.send(()).unwrap();
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        });

        // Main loop
        spawn(move || {
            loop {
                cbc::select! {
                    recv(floor_rx) -> floor => {
                        self.handle_event(Event::FloorReached(floor.unwrap()));
                    }
                    recv(door_closed_rx) -> _ => {
                        self.handle_event(Event::DoorClosed);
                    }
                    default(Duration::from_millis(100)) => {
                        let next_dir = self.choose_direction(self.state.floor);
                            // Update the state here if necassary
                    }
                }
            }
        });

        // Find the initial floor
        if let Some(floor_sensor) = self.elevator.floor_sensor() {
            self.state.floor = floor_sensor;
        } else {
            self.elevator.motor_direction(DIRN_UP);
        }

        Ok(())
    }

    fn handle_event(&mut self, event: Event) {
        match event {
            Event::FloorReached(floor) => {
                self.state.floor = floor;
                self.complete_orders(floor);
            }
            Event::StopPressed => {
                self.elevator.stop_button_light(true);
                self.elevator.motor_direction(DIRN_STOP);
                while self.elevator.stop_button() {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
                self.elevator.stop_button_light(false);
                self.elevator.motor_direction(self.state.direction);
            }
            Event::DoorClosed => {
                self.complete_orders(self.state.floor);
                let next_direction = self.choose_direction(self.state.floor);
                self.state.direction = next_direction;
                self.elevator.motor_direction(next_direction);
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
                for f in (current_floor + 1)..self.elevator.num_floors {
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

            _ => {return false;}
        }

        return false;
    }

    fn complete_orders(&mut self, floor: u8) {
        let is_top_floor = floor == self.elevator.num_floors - 1;
        let is_bottom_floor = floor == 0;

        // Remove cab orders at current floor.
        if self.state.cab_requests[floor as usize] {
            // Open the door
            self.door_open_tx.send(()).unwrap();
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
            self.door_open_tx.send(()).unwrap();
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
            self.door_open_tx.send(()).unwrap();
            self.door_open = true;

            // Update the state and send it to the coordinator
            self.hall_requests[floor as usize][HALL_DOWN as usize] = false;
            self.complete_order_tx.send((floor, HALL_DOWN)).unwrap();
        }
    }
}
