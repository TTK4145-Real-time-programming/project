/* Libraries */
use driver_rust::elevio;

/* Modules */
mod control;
mod fsm;

/* Main program */
fn main() {
    fsm::fsm();
    control::control()
}
