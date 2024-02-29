/* 3rd party libraries */
use crossbeam_channel as cbc;
use network_rust::udpnet;
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

    // Current bug:
    // When the elevator fsm is started, a "connection refused" panic
    // is thrown in the network module.

    // Initialize channels
    let (hall_request_tx, hall_request_rx) = cbc::unbounded::<Vec<Vec<bool>>>();
    let (complete_order_tx, complete_order_rx) = cbc::unbounded::<(u8, u8)>();
    let (state_tx, state_rx) = cbc::unbounded::<ElevatorState>();
    let (data_send_tx, data_send_rx) = cbc::unbounded::<ElevatorData>();
    let (data_recv_tx, data_recv_rx) = cbc::unbounded::<ElevatorData>();
    let (peer_update_tx, peer_update_rx) = cbc::unbounded::<udpnet::peers::PeerUpdate>();
    let (peer_tx_enable_tx, peer_tx_enable_rx) = cbc::unbounded::<bool>();

    // Create the elevator state
    let n_floors = config.elevator.n_floors.clone();
    let elevator_data = ElevatorData::new(n_floors);

    // Start the network module
    let network = Network::new(
        &config.network,
        data_send_rx,
        data_recv_tx,
        peer_update_tx,
        peer_tx_enable_rx,
    )?;
    let id = network.id.clone();

    // Start the elevator module
    let mut elevator_fsm = elevator::ElevatorFSM::new(
        &config.elevator,
        hall_request_rx,
        complete_order_tx,
        state_tx,
    )?;

    // Clone for coordinator
    let elevator_driver = elevator_fsm.elevator_driver.clone();

    spawn(move || loop {
        elevator_fsm.run()
    });


    // Things Chris must use:

    hall_request_tx;
    state_rx;
    complete_order_rx;

    data_send_tx;
    peer_update_rx;
    data_recv_rx;
    peer_tx_enable_tx; // Only if you want to enable/disable the peer discovery, not necessary

    elevator_data;
    elevator_driver;
    id;

    return Ok(());
}
