/*
* This file contains fsm for the elevator
*/

use driver_rust::elevio::{elev, poll};

pub const N_FLOORS : usize = 4;
pub const N_BUTTONS : usize = 3;

fn fsm_task(e: Elevator){
    //Inittilize to defined state
    fsm_init();

    //Start FSM
    loop{
        match e.state {
            State::Idle => {
                FSM_idleUpdate();
            }
            State::Moving => {
                FSM_movingUpdate();
            }
            State::DoorOpen => {
                FSM_doorOpenUpdate();
            }
        }
    }
}



// Starts all lights on elevator
fn startAlllights(e: Elevetor){
    for i in 0..e.requests.len() {
        for j in 0..e.requests.len() {
            elev::set_button_lamp(i, j, e.requests[i][j]);
        }
    }
}

// Inittilize fsm
fn fsm_init(){
    let e = Elevator::init();
    
    e.current_floor = elev::floor_sensor();
    while(e.current_floor == -1){
        elev::motor_direction(elev::DIRN_UP);
        e.current_floor = elev::floor_sensor();
    }
}

fn FSM_idleUpdate(){

    //Updating orders
    setOrders(e);

    if(elev::stop_button()){
        elev::stop_button_light(true);
        elev::motor_direction(elev::DIRN_STOP);

        //TODO: Open door if at a floor
    }

    let mut newDirection = requests_chooseDirection(e);
    
    if(newDirection.direction != Direction::Stop){
        e.direction = newDirection.direction;
        e.state = State::Moving;

        //Start motor
        if(e.direction == Direction::Up){
            elev::motor_direction(elev::DIRN_UP);
        }
        else if(e.direction == Direction::Down){
            elev::motor_direction(elev::DIRN_DOWN);
        }

    }
}

fn FSM_doorOpenUpdate(){
    //Updating orders
    setOrders(e);
    e.state = State::Idle;
}

fn FSM_movingUpdate(){
    //Updating orders
    setOrders(e);

    //Check if elevator should stop
    if(elev::floor_sensor() != -1){
        e.current_floor = elev::floor_sensor();

        elev::floor_indicator(e.current_floor);

        if(elev::should_stop(e)){
            // Stop motor
            elev::motor_direction(elev::DIRN_STOP); 
            
            request_clear_current_floor(e);
            
            startAlllights(e);
            
            e.state = State::DoorOpen;
            
            elev::door_light(true);

        }
    }
    else{
        return;
    }

}

//Find all orders
fn setOrders(e: Elevator){
    for i in 0..e.requests.len() {
        for j in 0..e.requests.len() {
            e.requests[i][j] = elev::get_button_signal(i, j);
        }
    }
}