use driver_rust::elevio;

// Making elevator struct
pub struct Elevator {
    pub state: State,
    pub current_floor: i32,
    pub direction: Direction,
    pub door_timer: i32,
    pub requests: [[bool; 3]; 3],
}

impl Elevator {
    //Innitilize elevator function
    fn init() -> Elevator {
        Elevator {
            current_floor: -1,
            direction: Direction::Stop,
            state: State::Idle,
            door_timer: 0,
            requests: [[false; 3]; 3],
        }
    }
}

//Elevator direction
enum Direction {
    Up,
    Down,
    Stop,
}

//Elevator state
enum State {
    Moving,
    Idle,
    DoorOpen,
}

//desired direction
pub struct DesiredDirection {
    pub state: State,
    pub direction: Direction,
}
