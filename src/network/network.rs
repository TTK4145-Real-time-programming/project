/**
 * Facilitates network communications for the elevator system.
 *
 * This module sets up networking capabilities, allowing for the sending and receiving
 * of elevator data and peer updates over UDP with acknowledgements. It manages network interactions necessary
 * for the distributed operation of elevator controllers. It communicates with the
 * coordinator thread. 
 *
 * # Network
 * Struct for initializing network communications.
 *
 * # Fields
 * - `id`: Unique identifier for the network node, based on the local IP and port.
 *
 * # Constructor arguments
 * - `config`:                  Network configuration settings.
 * - `net_data_send_rx`:        Receiver for elevator data to be sent.
 * - `net_data_recv_tx`:        Sender for forwarding received elevator data to coordinator.
 * - `net_peer_update_tx`:      Sender for forwarding received peer updates to coordinator.
 * - `net_peer_tx_enable_rx`:   Receiver to enable/disable peer ID broadcasting.
 *
 */

/***************************************/
/*             Libraries               */
/***************************************/
use crossbeam_channel as cbc;
use network_rust::udpnet;
use std::net::UdpSocket;
use std::thread::{Builder, sleep};
use std::time::{Duration, Instant};
use std::process;
use std::net;
use log::{info, error};

/***************************************/
/*           Local modules             */
/***************************************/
use crate::config::NetworkConfig;
use crate::shared::ElevatorData;

/***************************************/
/*             Public API              */
/***************************************/
pub struct Network {
    pub id: String,
}

impl Network {
    pub fn new(
        config: &NetworkConfig,
        net_data_send_rx: cbc::Receiver<ElevatorData>,
        net_data_recv_tx: cbc::Sender<ElevatorData>,
        net_peer_update_tx: cbc::Sender<udpnet::peers::PeerUpdate>,
        net_peer_tx_enable_rx: cbc::Receiver<bool>,
    ) -> std::io::Result<Network> {

        let msg_port = config.msg_port;
        let peer_port = config.peer_port;
        let ack_timeout = config.ack_timeout;
        let max_retries = config.max_retries;

        let local_ip_result = find_local_ip(
            config.id_gen_address.clone(),
            config.max_attempts_id_generation,
            Duration::from_millis(config.delay_between_attempts_id_generation),
        );

        let id = match local_ip_result {
            Some(ip) => format!("{}:{}", ip, msg_port.clone()),
            None => {
                error!("Failed to generate ID, elevator is offline, running single elevator mode");
                return Ok(Network { id: "Offline Elevator".to_string() });
            }
        };

        info!("ID: {}", id);
        let id_tx = id.clone();

        // Thread for broadcasting peer ID
        let peer_tx_thread = Builder::new().name("peer_tx".into());
        peer_tx_thread
            .spawn(move || {
                if udpnet::peers::tx(peer_port, id_tx, net_peer_tx_enable_rx).is_err() {
                    process::exit(1);
                }
            })
            .unwrap();

        // Thread for receiving and forwarding peer updates on port 'peer_port'
        let peer_rx_thread = Builder::new().name("peer_rx".into());
        peer_rx_thread
            .spawn(move || {
                if udpnet::peers::rx(peer_port, net_peer_update_tx).is_err() {
                    process::exit(1);
                }
            })
            .unwrap();


        // Thread for sending out data
        let data_tx_thread = Builder::new().name("data_tx".into());
        data_tx_thread
            .spawn(move || {
                let max_retries = max_retries;
                let ack_timeout = ack_timeout;
                loop {
                    match net_data_send_rx.recv() {
                        Ok(data) => {
                            let peer_addresses = data.states.keys().cloned().collect::<Vec<String>>();
                            send_ack(peer_addresses, data, max_retries, ack_timeout);
                        }
                        Err(e) => error!("Error receiving data to send: {}", e),
                    }
                }

            })
            .unwrap();


        // Thread for receiving data packets
        let data_rx_thread = Builder::new().name("data_rx".into());
        data_rx_thread.spawn(move || {
            let socket = match UdpSocket::bind(format!("0.0.0.0:{}", msg_port)) {
                Ok(s) => s,
                Err(e) => {
                    error!("Failed to bind UDP socket on port {}: {}", msg_port, e);
                    process::exit(1);
                }
            };

            loop {
                match recv_ack(&socket) {
                    Some(data) => {
                        net_data_recv_tx.send(data).unwrap();
                    }
                    None => {}
                }
            }
        }).unwrap();

        Ok(Network { id })
    }
}


/***************************************/
/*           Local functions           */
/***************************************/
fn send_ack(peer_addresses: Vec<String>, data: ElevatorData, max_retries: u32, ack_timeout: u64) {
    let socket = match UdpSocket::bind("0.0.0.0:0") {
        Ok(s) => s,
        Err(_) => process::exit(1),
    };

    for peer_address in peer_addresses {
        let mut retries = 0;
        let serialized_data_string = serde_json::to_string(&data).unwrap();
        let serialized_data = serialized_data_string.as_bytes();

        // Try until max_retries or ACK received
        while retries < max_retries {
            
            if socket.send_to(&serialized_data, &peer_address).is_ok() {
                let start = Instant::now();
                let mut ack_received = false;
                socket.set_read_timeout(Some(Duration::from_millis(ack_timeout))).unwrap();

                while start.elapsed() < Duration::from_millis(ack_timeout) {
                    let mut buffer = [0; 1024];

                    match socket.recv_from(&mut buffer) {
                        Ok((_number_of_bytes, src_addr)) => {
                            if src_addr.to_string() == peer_address {

                                // Verify if the received message is an ACK
                                let msg = String::from_utf8_lossy(&buffer[.._number_of_bytes]);
                                let ack = msg.trim();
                                if ack == "ACK" {
                                    ack_received = true;
                                    break;
                                }
                            }
                        },
                        Err(_) => continue, // Timeout
                    }
                }

                if ack_received {
                    break;
                }
                info!("No ACK received, retrying...");
                retries += 1;
            } 
            
            else {
                info!("Failed to send data to {}", peer_address);
                retries += 1;
            }
        
            if retries == max_retries {
                info!("Failed to send data to {} after {} retries", peer_address, max_retries);
            }
        }
    }
}

fn recv_ack(socket: &UdpSocket) -> Option<ElevatorData> {
    let mut buffer = [0; 1024];
    match socket.recv_from(&mut buffer) {
        Ok((number_of_bytes, src_addr)) => {
            let received_data = &buffer[..number_of_bytes];
            let message = match std::str::from_utf8(received_data) {
                Ok(v) => v,
                Err(e) => {
                    error!("Invalid UTF-8 sequence: {}", e);
                    return None;
                }
            };

            let deserialized_message: Result<ElevatorData, _> = serde_json::from_str(message);
            match deserialized_message {
                Ok(data) => {
                    if let Err(e) = socket.send_to(b"ACK", src_addr) {
                        error!("Failed to send ACK to {}: {}", src_addr, e);
                    }
                    Some(data)
                },
                Err(e) => {
                    error!("Failed to deserialize message: {}", e);
                    None
                }
            }
        },
        Err(e) => {
            error!("Failed to receive a message: {}", e);
            None
        },
    }
}

fn find_local_ip(address: String, max_attempts: u32, delay_between_attempts: Duration) -> Option<std::net::IpAddr> {
    let mut attempts = 0;
    while attempts < max_attempts {
        match net::TcpStream::connect(address.clone()) {
            Ok(stream) => match stream.local_addr() {
                Ok(addr) => return Some(addr.ip()),
                Err(e) => error!("Failed to get local address: {}", e),
            },
            Err(e) => {
                error!("Attempt {} to generate ID failed: {}", attempts + 1, e);
                sleep(delay_between_attempts);
            },
        }
        attempts += 1;
    }
    None
}
