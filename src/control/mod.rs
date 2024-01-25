// Making elevator struct
pub struct Elevator {
    pub state: State,
    pub current_floor: i32,
    pub direction: Direction,
    pub door_timer: i32,
}

impl Elevator{
    //Innitilize elevator function
    fn init() -> Elevator {
        Elevator {
            current_floor: -1,
            direction: Direction::Up,
            state: State::Idle,

            // Run 
        }
    }




}



//Elevator direction
enum Direction {
    Up,
    Down,
}

//Elevator state
enum State {
    Moving,
    Idle,
    DoorOpen,
}









