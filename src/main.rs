/* Libraries */

/* Modules */
mod elevator;

/* Main */
fn main() -> std::io::Result<()> {
    let mut fsm = elevator::ElevatorFSM::new("localhost:127.0.0.1",4)?;
    fsm.run();
    return Ok(());
}

