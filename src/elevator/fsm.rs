use driver_rust::elevio::elev::Elevator;
use driver_rust::elevio::elev::{CAB, DIRN_DOWN, DIRN_STOP, DIRN_UP, HALL_DOWN, HALL_UP};
use std::time::{Duration, Instant};

enum Event {
    RequestReceived(u8, u8),
    FloorReached(u8),
    StopPressed,
    DoorClosed,
    NoEvent,
}

pub struct ElevatorFSM {
    elevator: Elevator,
    order_list: Vec<Vec<bool>>,
    floor: Option<u8>,
    direction: u8,
    door_open: bool,
    door_timer: Option<Instant>,
}

impl ElevatorFSM {
    pub fn new(addr: &str, num_floors: u8) -> Result<Self, std::io::Error> {
        Ok(ElevatorFSM {
            elevator: Elevator::init(addr, num_floors)?,
            order_list: vec![vec![false; 3]; num_floors as usize],
            floor: None,
            direction: DIRN_STOP,
            door_open: false,
            door_timer: None,
        })
    }

    pub fn run(&mut self) {
        self.init();
        loop {
            let event: Event = self.wait_for_event();
            self.handle_event(event);
        }
    }

    fn wait_for_event(&mut self) -> Event {
        // Checks if stop button has been pressed.
        if self.elevator.stop_button() {
            return Event::StopPressed;
        }

        // Checks if any buttons have been pressed.
        for floor in 0..self.elevator.num_floors {
            if !self.order_list[floor as usize][HALL_UP as usize]
                && self.elevator.call_button(floor, HALL_UP)
            {
                return Event::RequestReceived(floor, HALL_UP);
            }
            if !self.order_list[floor as usize][HALL_DOWN as usize]
                && self.elevator.call_button(floor, HALL_DOWN)
            {
                return Event::RequestReceived(floor, HALL_DOWN);
            }
            if !self.order_list[floor as usize][CAB as usize]
                && self.elevator.call_button(floor, CAB)
            {
                return Event::RequestReceived(floor, CAB);
            }
        }

        // Checks if the elevator has reached a floor.
        if let Some(current_floor) = self.elevator.floor_sensor() {
            if Some(current_floor) != self.floor {
                self.floor = Some(current_floor);
                return Event::FloorReached(current_floor);
            }
        }

        // If no event is detected, you may want to sleep for a short duration.
        std::thread::sleep(std::time::Duration::from_millis(100));

        return Event::NoEvent;
    }

    fn handle_event(&mut self, event: Event) {
        match event {
            Event::RequestReceived(floor, request_type) => {
                self.order_list[floor as usize][request_type as usize] = true;
                self.update_lights();
                self.print_order_list();
                if self.direction == DIRN_STOP {
                    let next_direction = self.choose_direction(self.floor.unwrap());
                    self.direction = next_direction;
                    self.elevator.motor_direction(next_direction);
                    if next_direction == DIRN_STOP {
                        self.complete_orders(floor);
                    }
                }
            }
            Event::FloorReached(floor) => {
                self.floor = Some(floor);
                self.complete_orders(floor);
                self.print_order_list();
                if !self.door_open {
                    let next_direction = self.choose_direction(floor);
                    self.direction = next_direction;
                    self.elevator.motor_direction(next_direction);
                }
            }
            Event::StopPressed => {
                self.elevator.stop_button_light(true);
                self.elevator.motor_direction(DIRN_STOP);
                while self.elevator.stop_button() {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
                self.elevator.stop_button_light(false);
                self.elevator.motor_direction(self.direction);
            }
            Event::DoorClosed => {
                self.complete_orders(self.floor.unwrap());
                if !self.door_open {
                    let next_direction = self.choose_direction(self.floor.unwrap());
                    self.direction = next_direction;
                    self.elevator.motor_direction(next_direction);
                }
            }
            Event::NoEvent => {
                // Check if the door is open and the timer has elapsed
                if let Some(timer) = self.door_timer {
                    if self.elevator.obstruction() {
                        self.door_timer = Some(Instant::now() + Duration::from_secs(3));
                    } else if timer <= Instant::now() {
                        self.close_door();
                    }
                }
            }
        }
    }

    fn choose_direction(&mut self, floor: u8) -> u8 {
        // Continue in current direction of travel if there are any further orders in that direction
        if self.has_orders_in_direction(floor, self.direction) {
            return self.direction;
        }

        // Otherwise change direction if there are orders in the opposite direction
        if self.direction == DIRN_UP && self.has_orders_in_direction(floor, DIRN_DOWN) {
            return DIRN_DOWN;
        } else if self.direction == DIRN_DOWN && self.has_orders_in_direction(floor, DIRN_UP) {
            return DIRN_UP;
        }

        // Start moving if necessary
        if self.direction == DIRN_STOP {
            if self.has_orders_in_direction(floor, DIRN_UP) {
                return DIRN_UP;
            } else if self.has_orders_in_direction(floor, DIRN_DOWN) {
                return DIRN_DOWN;
            }
        }

        // If there are no orders, stop.
        return DIRN_STOP;
    }

    fn has_orders_in_direction(&self, start_floor: u8, direction: u8) -> bool {
        match direction {
            DIRN_UP => {
                for f in (start_floor + 1)..self.elevator.num_floors {
                    if self.order_list[f as usize][CAB as usize]
                        || self.order_list[f as usize][HALL_UP as usize]
                        || self.order_list[f as usize][HALL_DOWN as usize]
                    {
                        return true;
                    }
                }
            }
            DIRN_DOWN => {
                for f in (0..start_floor).rev() {
                    if self.order_list[f as usize][CAB as usize]
                        || self.order_list[f as usize][HALL_UP as usize]
                        || self.order_list[f as usize][HALL_DOWN as usize]
                    {
                        return true;
                    }
                }
            }
            _ => return false,
        }

        false
    }

    fn complete_orders(&mut self, floor: u8) {
        let is_top_floor = floor == self.elevator.num_floors - 1;
        let is_bottom_floor = floor == 0;

        // Flag to determine if the door needs to be opened.
        let mut should_open_door = false;

        // Remove cab orders at current floor.
        if self.order_list[floor as usize][CAB as usize] {
            self.order_list[floor as usize][CAB as usize] = false;
            should_open_door = true;
        }

        // Remove hall up orders.
        if (self.direction == DIRN_UP || self.direction == DIRN_STOP || is_bottom_floor)
            && self.order_list[floor as usize][HALL_UP as usize]
        {
            self.elevator.call_button_light(floor, HALL_UP, false);
            self.order_list[floor as usize][HALL_UP as usize] = false;
            should_open_door = true;
        }

        // Remove hall down orders.
        if (self.direction == DIRN_DOWN || self.direction == DIRN_STOP || is_top_floor)
            && self.order_list[floor as usize][HALL_DOWN as usize]
        {
            self.elevator.call_button_light(floor, HALL_DOWN, false);
            self.order_list[floor as usize][HALL_DOWN as usize] = false;
            should_open_door = true;
        }

        // Open door if needed and update lights.
        if should_open_door {
            self.open_door();
        }

        // Update order indicators.
        self.update_lights();
    }

    fn open_door(&mut self) {
        self.elevator.motor_direction(DIRN_STOP);
        self.elevator.door_light(true);
        self.door_open = true;
        self.door_timer = Some(Instant::now() + Duration::from_secs(3));
    }

    fn close_door(&mut self) {
        self.elevator.door_light(false);
        self.door_open = false;
        self.door_timer = None;
        self.handle_event(Event::DoorClosed);
    }

    fn init(&mut self) {
        self.update_lights();
        if let Some(floor_sensor) = self.elevator.floor_sensor() {
            self.floor = Some(floor_sensor);
        } else {
            self.elevator.motor_direction(DIRN_UP);
        }
    }

    fn update_lights(&mut self) {
        for floor in 0..self.elevator.num_floors {
            self.elevator.call_button_light(
                floor,
                HALL_UP,
                self.order_list[floor as usize][HALL_UP as usize],
            );
            self.elevator.call_button_light(
                floor,
                HALL_DOWN,
                self.order_list[floor as usize][HALL_DOWN as usize],
            );
            self.elevator.call_button_light(
                floor,
                CAB,
                self.order_list[floor as usize][CAB as usize],
            );
        }
    }

    fn print_order_list(&self) {
        let order_types = ["HALL_UP", "HALL_DOWN", "CAB"];

        // Print header
        println!("\nFloor\t{}", order_types.join("\t\t"));

        // Iterate over each floor's orders in reverse order
        for (floor, orders) in self.order_list.iter().enumerate().rev() {
            // Print floor number
            print!("{}\t", floor);
            for &order in orders.iter() {
                // Print order presence with a more readable format (e.g., Yes/No)
                let presence = if order { "Yes" } else { "No" };
                print!("{}\t\t", presence);
            }
            println!(); // New line after each floor's orders
        }
    }
}
