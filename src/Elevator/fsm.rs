use driver_rust::elevio::elev::Elevator;
use driver_rust::elevio::elev::{CAB, DIRN_DOWN, DIRN_STOP, DIRN_UP, HALL_DOWN, HALL_UP};

enum Event {
    RequestReceived(u8, u8),
    FloorReached(u8),
    NoEvent,
}

pub struct ElevatorFSM {
    elevator: Elevator,
    order_list: Vec<Vec<bool>>,
    direction: u8,
    door_open: bool,
}

impl ElevatorFSM {
    pub fn new(addr: &str, num_floors: u8) -> Result<Self, std::io::Error> {
        Ok(ElevatorFSM {
            elevator: Elevator::init(addr, num_floors)?,
            order_list: vec![vec![false; num_floors as usize]; num_floors as usize],
            direction: DIRN_STOP,
            door_open: false,
        })
    }

    pub fn run(&mut self) {
        loop {
            let event: Event = self.wait_for_event();
            self.handle_event(event);
        }
    }

    fn wait_for_event(&self) -> Event {
        // Checks if the elevator has reached a floor.
        if let Some(current_floor) = self.elevator.floor_sensor() {
            return Event::FloorReached(current_floor);
        }

        // Checks if any buttons have been pressed.
        for floor in 0..self.elevator.num_floors {
            if self.elevator.call_button(floor, HALL_UP) {
                return Event::RequestReceived(floor, HALL_UP);
            }
            if self.elevator.call_button(floor, HALL_DOWN) {
                return Event::RequestReceived(floor, HALL_DOWN);
            }
            if self.elevator.call_button(floor, CAB) {
                return Event::RequestReceived(floor, CAB);
            }
        }

        // If no event is detected, you may want to sleep for a short duration.
        std::thread::sleep(std::time::Duration::from_millis(10));

        return Event::NoEvent;
    }

    fn handle_event(&mut self, event: Event) {
        match event {
            Event::RequestReceived(floor, request_type) => {
                self.order_list[floor as usize][request_type as usize] = true;
            }
            Event::FloorReached(floor) => {
                self.complete_orders(floor);
                let next_direction = self.choose_direction(floor);
                self.elevator.motor_direction(next_direction);
            }
            Event::NoEvent => {}
        }
    }

    fn choose_direction(&mut self, floor: u8) -> u8 {
        // Continue up if there are orders above the elevator.
        if self.direction == DIRN_UP {
            for f in floor..self.elevator.num_floors {
                if self.order_list[f as usize][CAB as usize]
                    || self.order_list[f as usize][HALL_UP as usize]
                    || self.order_list[f as usize][HALL_DOWN as usize]
                {
                    return DIRN_UP;
                }
            }
            // If there are no orders above, check if there are orders below.
            for f in (0..floor).rev() {
                if self.order_list[f as usize][CAB as usize]
                    || self.order_list[f as usize][HALL_UP as usize]
                    || self.order_list[f as usize][HALL_DOWN as usize]
                {
                    return DIRN_DOWN;
                }
            }
        }
        // Continue down if there are orders below the elevator.
        else if self.direction == DIRN_DOWN {
            for f in (0..floor).rev() {
                if self.order_list[f as usize][CAB as usize]
                    || self.order_list[f as usize][HALL_UP as usize]
                    || self.order_list[f as usize][HALL_DOWN as usize]
                {
                    return DIRN_DOWN;
                }
            }
            // If there are no orders below, check if there are orders above.
            for f in floor..self.elevator.num_floors {
                if self.order_list[f as usize][CAB as usize]
                    || self.order_list[f as usize][HALL_UP as usize]
                    || self.order_list[f as usize][HALL_DOWN as usize]
                {
                    return DIRN_UP;
                }
            }
        }

        // If there are no orders, stop.
        return DIRN_STOP;
    }

    fn complete_orders(&mut self, floor: u8) {
        // Remove cab orders at current floor.
        self.order_list[floor as usize][CAB as usize] = false;

        // Remove hall orders at current floor if elevator is moving in the same direction.
        if self.direction == DIRN_UP {
            self.order_list[floor as usize][HALL_UP as usize] = false;
            self.elevator.call_button_light(floor, HALL_UP, false);
        } else if self.direction == DIRN_DOWN {
            self.order_list[floor as usize][HALL_DOWN as usize] = false;
            self.elevator.call_button_light(floor, HALL_DOWN, false);
        }
    }
}
