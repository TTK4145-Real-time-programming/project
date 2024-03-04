/* 3rd party libraries */
use crossbeam_channel as cbc;
use network_rust::udpnet;
use std::thread::Builder;
use std::thread::*;

/* Custom libraries */
use network::Network;
use shared_structs::ElevatorData;
use shared_structs::ElevatorState;
use elevator::ElevatorDriver;
use elevator::ElevatorFSM;
use coordinator::Coordinator;

/* Modules */
mod config;
mod elevator;
mod network;
mod shared_structs;
mod coordinator;

/* Main */
fn main() -> std::io::Result<()> {
    // Load the configuration
    let config = config::load_config();

    // Initialize channels
    let (hall_requests_tx, hall_requests_rx) = cbc::unbounded::<Vec<Vec<bool>>>();
    let (cab_request_tx, cab_request_rx) = cbc::unbounded::<u8>();
    let (complete_order_tx, complete_order_rx) = cbc::unbounded::<(u8, u8)>();
    let (state_tx, state_rx) = cbc::unbounded::<ElevatorState>();
    let (data_send_tx, data_send_rx) = cbc::unbounded::<ElevatorData>();
    let (data_recv_tx, data_recv_rx) = cbc::unbounded::<ElevatorData>();
    let (peer_update_tx, peer_update_rx) = cbc::unbounded::<udpnet::peers::PeerUpdate>();
    let (_peer_tx_enable_tx, peer_tx_enable_rx) = cbc::unbounded::<bool>();

    // Hardware channels
    let (hw_motor_direction_tx, hw_motor_direction_rx) = cbc::unbounded::<u8>();
    let (hw_button_light_tx, hw_button_light_rx) = cbc::unbounded::<(u8, u8, bool)>();
    let (hw_request_tx, hw_request_rx) = cbc::unbounded::<(u8, u8)>();
    let (hw_floor_sensor_tx, hw_floor_sensor_rx) = cbc::unbounded::<u8>();
    let (hw_door_light_tx, hw_door_light_rx) = cbc::unbounded::<bool>();
    let (hw_obstruction_tx, hw_obstruction_rx) = cbc::unbounded::<bool>();
    let (hw_stop_button_tx, hw_stop_button_rx) = cbc::unbounded::<bool>();

    // Start the hardware module 
    let elevator_driver = ElevatorDriver::new(
        &config.hardware,
        hw_motor_direction_rx,
        hw_button_light_rx,
        hw_request_tx,
        hw_floor_sensor_tx,
        hw_door_light_rx,
        hw_obstruction_tx,
        hw_stop_button_tx,
    );

    let elevator_driver_thread = Builder::new().name("elevator_driver".into());
    elevator_driver_thread
        .spawn(move || elevator_driver.run())
        .unwrap();

    // Start the network module
    let network = Network::new(
        &config.network,
        data_send_rx,
        data_recv_tx,
        peer_update_tx,
        peer_tx_enable_rx,
    )?;
    let _id = network.id.clone();

    // Start the elevator module
    let elevator_fsm = ElevatorFSM::new(
        &config.elevator,
        hw_motor_direction_tx,
        hw_floor_sensor_rx,
        hw_door_light_tx,
        hw_obstruction_rx,
        hw_stop_button_rx,
        hall_requests_rx,
        cab_request_rx,
        complete_order_tx,
        state_tx,
    );

    let elevator_fsm_thread = Builder::new().name("elevator_fsm".into());
    elevator_fsm_thread
        .spawn(move || elevator_fsm.run())
        .unwrap();

    // Create the elevator state
    let _n_floors = config.hardware.n_floors.clone();
    let mut _elevator_data = ElevatorData::new(_n_floors);
    _elevator_data.states.insert(_id.clone(), ElevatorState::new(_n_floors));

    // Start the coordinator module
    let mut coordinator = Coordinator::new(
        _elevator_data,
        _id,
        _n_floors,
        hw_button_light_tx,
        hw_request_rx,
        hall_requests_tx,
        cab_request_tx,
        state_rx,
        complete_order_rx,
        data_send_tx,
        data_recv_rx,
        peer_update_rx,
    )?;

    let coordinator_thread = Builder::new().name("coordinator".into());
    coordinator_thread
        .spawn(move || coordinator.run())
        .unwrap();

    /*

    --------------------
    | THINGS FOR CHRIS |
    --------------------

    Coordinator FSM communication channels:
    - hall_request_tx;      | Send hall requests to the FSM
    - state_rx;             | Receive state updates from FSM
    - complete_order_rx;    | Receive completed orders from FSM

    Coordinator network communication channels:
    - data_send_tx;         | Send data to the network
    - peer_update_rx;       | Receive peer updates from the network
    - data_recv_rx;         | Receive data from the network
    - peer_tx_enable_tx;    | Enable/disable peer discovery (Optional / low priority)

    Coordinator hardware communication channels:
    - hw_button_light_tx    | Send button light commands
    - hw_hall_request_rx    | Receive hall requests

    Data structures:
    - elevator_data         | ElevatorData struct with local elevator initialized
    - id                    | String with the local elevator id

    */

    loop {
        sleep(std::time::Duration::from_secs(1));
    }

    return Ok(());
}
