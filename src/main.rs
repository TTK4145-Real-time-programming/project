/* Libraries */
use std::thread::*;

/* Modules */
mod elevator;
mod network;
mod config;

/* Main */
fn main() -> std::io::Result<()> {

    // Load the configuration
    let config = config::load_config();

    // Start the elevator module
    // let mut fsm = elevator::ElevatorFSM::new("localhost:15657", 4)?;
    // fsm.run();

    // Start the network module
    spawn(move || {
        if let Err(e) = network::network(&config.network) {
            // Handle the error as needed, e.g., log it or panic
            panic!("Network initialization failed: {}", e);
        }
    });

    loop {
        sleep(std::time::Duration::from_secs(1));
    }

    return Ok(());
}
