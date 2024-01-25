use driver_rust::elevio::elev::{HALL_UP, HALL_DOWN, CAB, DIRN_DOWN, DIRN_STOP, DIRN_UP};
use driver_rust::elevio::elev::Elevator;

enum Event {
    RequestReceived(u8, u8),
    FloorReached(u8),
    NoEvent,
}

pub struct ElevatorFSM {
    elevator: Elevator,
}

impl ElevatorFSM {
    pub fn new(addr: &str, num_floors: u8) -> Result<Self, std::io::Error> {
        Ok(ElevatorFSM {
            elevator: Elevator::init(addr, num_floors)?,
        })
    }

    pub fn run(&mut self) -> std::io::Result<()> {
        loop {
            let event: Event = self.wait_for_event();
            self.handle_event(event);
        }
    }

    fn wait_for_event(&self) -> Event {
        // Here you would have logic to wait for an event.
        // This could involve polling the elevator for its state,
        // checking button presses, etc.

        // Example placeholder logic: check if a button has been pressed.
        if self.elevator.floor_sensor() != Some(u8::MAX) {
            return Event::FloorReached(1);
        }

        // You would also check other inputs, such as button presses.
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
                
            },
            Event::FloorReached(floor) => {
                
            },
            Event::NoEvent => {},
        }
    }
}
