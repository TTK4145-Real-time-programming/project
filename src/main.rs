/***************************************/
/*        3rd party libraries          */
/***************************************/
use crossbeam_channel as cbc;
use network_rust::udpnet;
use std::thread::Builder;
use std::thread::*;
use log::info;
use clap::{App, Arg};

/***************************************/
/*           Local modules             */
/***************************************/
use coordinator::Coordinator;
use elevator::ElevatorDriver;
use elevator::ElevatorFSM;
use network::Network;
use shared::ElevatorData;
use shared::ElevatorState;

mod config;
mod coordinator;
mod elevator;
mod network;
mod shared;

/***************************************/
/*        Program entry point          */
/***************************************/
fn main() -> std::io::Result<()> {

    // Initialize the logger
    env_logger::init();

    // Load the configuration
    let mut config = config::load_config();

    // Parse command line arguments
    let matches = App::new("project")
        .version("1.0")
        .about("Elevator project in TTK4145 distributed systems.")
        .arg(
            Arg::with_name("hardware_address")
                .long("hardware-address")
                .value_name("HARDWARE-ADDRESS")
                .help("Sets the hardware address")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("hardware_port")
                .long("hardware-port")
                .value_name("HARDWARE-PORT")
                .help("Sets the hardware port")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("network_port")
                .long("network-port")
                .value_name("NETWORK-PORT")
                .help("Sets the network data port")
                .takes_value(true),
        )
        .get_matches();

    // Override config with command line arguments if provided
    if let Some(addr) = matches.value_of("hardware_address") {
        config.hardware.driver_address = addr.to_string();
    }

    if let Some(port) = matches.value_of("hardware_port") {
        config.hardware.driver_port = port.parse().expect("Failed to parse hardware port");
    }

    if let Some(port) = matches.value_of("network_port") {
        config.network.msg_port = port.parse().expect("Failed to parse network port");
    }

    info!("Driver address: {}", config.hardware.driver_address.to_string());
    info!("Driver port: {}", config.hardware.driver_port.to_string());
    info!("Network port: {}", config.network.msg_port.to_string());

    // Termination signals
    let (_fsm_terminate_tx, fsm_terminate_rx) = cbc::unbounded::<()>();
    let (_coordinator_terminate_tx, coordinator_terminate_rx) = cbc::unbounded::<()>();
    let (_hw_terminate_tx, hw_terminate_rx) = cbc::unbounded::<()>();

    // FSM channels
    let (fsm_hall_requests_tx, fsm_hall_requests_rx) = cbc::unbounded::<Vec<Vec<bool>>>();
    let (fsm_cab_request_tx, fsm_cab_request_rx) = cbc::unbounded::<u8>();
    let (fsm_order_complete_tx, fsm_order_complete_rx) = cbc::unbounded::<(u8, u8)>();

    // Network channels
    let (fsm_state_tx, fsm_state_rx) = cbc::unbounded::<ElevatorState>();
    let (net_data_send_tx, net_data_send_rx) = cbc::unbounded::<ElevatorData>();
    let (net_data_recv_tx, net_data_recv_rx) = cbc::unbounded::<ElevatorData>();
    let (net_peer_update_tx, net_peer_update_rx) = cbc::unbounded::<udpnet::peers::PeerUpdate>();
    let (_net_peer_tx_enable_tx, net_peer_tx_enable_rx) = cbc::unbounded::<bool>();

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
        hw_terminate_rx,
    );

    let elevator_driver_thread = Builder::new().name("elevator_driver".into());
    elevator_driver_thread
        .spawn(move || elevator_driver.run())
        .unwrap();

    // Start the network module
    let network = Network::new(
        &config.network,
        net_data_send_rx,
        net_data_recv_tx,
        net_peer_update_tx,
        net_peer_tx_enable_rx,
    )?;
    let _id = network.id.clone();

    // Start the fsm module
    let elevator_fsm = ElevatorFSM::new(
        &config.elevator,
        hw_motor_direction_tx,
        hw_floor_sensor_rx,
        hw_door_light_tx,
        hw_obstruction_rx,
        hw_stop_button_rx,
        fsm_hall_requests_rx,
        fsm_cab_request_rx,
        fsm_order_complete_tx,
        fsm_state_tx,
        fsm_terminate_rx,
    );

    let elevator_fsm_thread = Builder::new().name("elevator_fsm".into());
    elevator_fsm_thread
        .spawn(move || elevator_fsm.run())
        .unwrap();

    // Create the elevator data instance
    let _n_floors = config.hardware.n_floors.clone();
    let mut _elevator_data = ElevatorData::new(_n_floors);
    _elevator_data
        .states
        .insert(_id.clone(), ElevatorState::new(_n_floors));

    info!("Elevator data read from file {:?}", _elevator_data);

    // Start the coordinator module
    let mut coordinator = Coordinator::new(
        _elevator_data,
        _id,
        _n_floors,
        hw_button_light_tx,
        hw_request_rx,
        fsm_hall_requests_tx,
        fsm_cab_request_tx,
        fsm_state_rx,
        fsm_order_complete_rx,
        net_data_send_tx,
        net_data_recv_rx,
        net_peer_update_rx,
        coordinator_terminate_rx,
    );

    let coordinator_thread = Builder::new().name("coordinator".into());
    coordinator_thread.spawn(move || coordinator.run()).unwrap();

    loop {
        sleep(std::time::Duration::from_secs(1));
    }
}
