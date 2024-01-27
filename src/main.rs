/* Libraries */

/* Modules */
mod elevator;

/* Main */
fn main() -> std::io::Result<()> {
    let mut fsm = elevator::ElevatorFSM::new("localhost:15657", 4)?;
    fsm.run();
    return Ok(());
}
