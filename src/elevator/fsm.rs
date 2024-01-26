use driver_rust::elevio::elev::Elevator;
use driver_rust::elevio::elev::{CAB, DIRN_DOWN, DIRN_STOP, DIRN_UP, HALL_DOWN, HALL_UP};
use std::time::{Duration, Instant};

enum Event {
    RequestReceived(u8, u8),
    FloorReached(u8),
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
                if floor != self.floor.unwrap() {
                    self.order_list[floor as usize][request_type as usize] = true;
                    self.update_lights();
                    self.print_order_list();
                    if self.direction == DIRN_STOP {
                        let next_direction = self.choose_direction(self.floor.unwrap());
                        self.direction = next_direction;
                        self.elevator.motor_direction(next_direction);
                    }
                }
            }
            Event::FloorReached(floor) => {
                self.complete_orders(floor);
                self.print_order_list();
                self.floor = Some(floor);
                if !self.door_open {
                    let next_direction = self.choose_direction(floor);
                    self.direction = next_direction;
                    self.elevator.motor_direction(next_direction);
                }
            }
            Event::DoorClosed => {
                let next_direction = self.choose_direction(self.floor.unwrap());
                self.direction = next_direction;
                self.elevator.motor_direction(next_direction);
            }
            Event::NoEvent => {
                // Check if the door is open and the timer has elapsed
                // println!("No event at time {:?}", Instant::now());
                if let Some(timer) = self.door_timer {
                    if timer <= Instant::now() {
                        self.close_door();
                    }
                }
            }
        }
    }

    fn choose_direction(&mut self, floor: u8) -> u8 {
        // Continue in current direction of travel if there are any further orders in that direction
        if self.direction == DIRN_UP && floor < self.elevator.num_floors - 1 {
            for f in floor..self.elevator.num_floors {
                if self.order_list[f as usize][CAB as usize]
                    || self.order_list[f as usize][HALL_UP as usize]
                {
                    return DIRN_UP;
                }
            }
        } else if self.direction == DIRN_DOWN && floor > 0 {
            for f in (0..floor).rev() {
                if self.order_list[f as usize][CAB as usize]
                    || self.order_list[f as usize][HALL_DOWN as usize]
                {
                    return DIRN_DOWN;
                }
            }
        }

        // Otherwise change direction if there are orders in the opposite direction
        if self.direction == DIRN_UP && floor > 0 {
            for f in (0..floor).rev() {
                if self.order_list[f as usize][CAB as usize]
                    || self.order_list[f as usize][HALL_DOWN as usize]
                {
                    return DIRN_DOWN;
                }
            }
        } else if self.direction == DIRN_DOWN && floor < self.elevator.num_floors - 1 {
            for f in floor..self.elevator.num_floors {
                if self.order_list[f as usize][CAB as usize]
                    || self.order_list[f as usize][HALL_UP as usize]
                {
                    return DIRN_UP;
                }
            }
        }

        // Start moving if necessary
        if self.direction == DIRN_STOP {
            if floor < self.elevator.num_floors - 1 {
                for f in (floor + 1)..(self.elevator.num_floors) {
                    if self.order_list[f as usize][CAB as usize]
                        || self.order_list[f as usize][HALL_UP as usize]
                        || self.order_list[f as usize][HALL_DOWN as usize]
                    {
                        return DIRN_UP;
                    }
                }
            }
            if floor > 0 {
                for f in (0..floor).rev() {
                    if self.order_list[f as usize][CAB as usize]
                        || self.order_list[f as usize][HALL_UP as usize]
                        || self.order_list[f as usize][HALL_DOWN as usize]
                    {
                        return DIRN_DOWN;
                    }
                }
            }
        }

        // If there are no orders, stop.
        return DIRN_STOP;
    }

    fn complete_orders(&mut self, floor: u8) {
        // Remove cab orders at current floor.
        if self.order_list[floor as usize][CAB as usize] {
            self.open_door();
            self.order_list[floor as usize][CAB as usize] = false;
        }

        // Remove hall orders at current floor if elevator is moving in the same direction.
        if self.direction == DIRN_UP && self.order_list[floor as usize][HALL_UP as usize] {
            self.open_door();
            self.elevator.call_button_light(floor, HALL_UP, false);
            self.order_list[floor as usize][HALL_UP as usize] = false;
        } else if self.direction == DIRN_DOWN && self.order_list[floor as usize][HALL_DOWN as usize]
        {
            self.open_door();
            self.order_list[floor as usize][HALL_DOWN as usize] = false;
            self.elevator.call_button_light(floor, HALL_DOWN, false);
        }

        // Remove hall orders at the top
        if floor == self.elevator.num_floors {
            self.open_door();
            self.order_list[floor as usize][HALL_DOWN as usize] = false;
        }

        // Remove hall orders at the bottom
        if floor == 0 {
            self.open_door();
            self.order_list[floor as usize][HALL_UP as usize] = false;
        }

        self.update_lights();
    }

    fn open_door(&mut self) {
        self.elevator.motor_direction(DIRN_STOP);
        self.direction = DIRN_STOP;
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
