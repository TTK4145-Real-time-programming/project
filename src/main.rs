/* 3rd party libraries */
use crossbeam_channel as cbc;
use std::sync::{Arc, Mutex};
use std::thread::*;

/* Custom libraries */
use network::Network;
use shared_structs::ElevatorData;
use shared_structs::ElevatorState;

/* Modules */
mod config;
mod elevator;
mod network;
mod shared_structs;

/* Main */
fn main() -> std::io::Result<()> {
    // Load the configuration
    let config = config::load_config();

    // Create the elevator state
    let n_floors = config.elevator.n_floors.clone();
    let elevator_data = ElevatorData::new(n_floors);

    // Start the network module
    let network = Network::new(&config.network)?;
    let id = network.id.clone();

    // Start the elevator module
    let elevator = elevator::ElevatorFSM::new(&config.elevator)?;
    
    // To Chris
    
    //elevator.hall_request_tx
    //elevator.state_rx
    //elevator.complete_order_rx

    //network.data_send_tx
    //network.peer_update_rx
    //network.custom_data_recv_rx

    // elevator_data

    // let elevator = Elevator::init(&config.driver_address, config.n_floors)?;

    

    // Din greien(network.riktig_kanal)

    loop {
        cbc::select! {
            recv(network.peer_update_rx) -> a => {
                let update = a.unwrap();
                println!("{:#?}", update);
            }
            recv(network.custom_data_recv_rx) -> a => {
                let cd = a.unwrap();
                println!("{:#?}", cd);
            }
        }
    }

    return Ok(());
}
