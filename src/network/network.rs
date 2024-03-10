/**
 * Facilitates network communications for the elevator system.
 *
 * This module sets up networking capabilities, allowing for the sending and receiving
 * of elevator data and peer updates over UDP. It manages network interactions necessary
 * for the distributed operation of elevator controllers. It communicates with the
 * coordinator thread.
 *
 * # Network
 * Struct for managing network communications.
 *
 * # Fields
 * - `id`: Unique identifier for the network node, based on the local IP and process ID, or a custom argument.
 *
 * # Constructor arguments
 * - `config`:                  Network configuration settings.
 * - `custom_net_data_send_rx`:     Receiver for elevator data to be sent.
 * - `custom_net_data_recv_tx`:     Sender for forwarding received elevator data.
 * - `net_peer_update_tx`:          Sender for forwarding received peer updates.
 * - `net_peer_tx_enable_rx`:       Receiver to enable/disable peer ID broadcasting.
 *
 */

/***************************************/
/*        3rd party libraries          */
/***************************************/
use crossbeam_channel as cbc;
use network_rust::udpnet;
use std::thread::Builder;
use std::process;
use std::net;
use log::info;

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
        custom_net_data_send_rx: cbc::Receiver<ElevatorData>,
        custom_net_data_recv_tx: cbc::Sender<ElevatorData>,
        net_peer_update_tx: cbc::Sender<udpnet::peers::PeerUpdate>,
        net_peer_tx_enable_rx: cbc::Receiver<bool>,
    ) -> std::io::Result<Network> {
        let local_ip = net::TcpStream::connect(config.id_gen_address.clone())
            .unwrap()
            .local_addr()
            .unwrap()
            .ip();
        let id = format!("rust@{}#{}", local_ip, process::id());
        info!("ID: {}", id);

        let msg_port = config.msg_port;
        let peer_port = config.peer_port;
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

        // Thread for sending out data packets. Packets are receiver from
        let data_tx_thread = Builder::new().name("data_tx".into());
        data_tx_thread
            .spawn(move || {
                if udpnet::bcast::tx(msg_port, custom_net_data_send_rx).is_err() {
                    process::exit(1);
                }
            })
            .unwrap();

        // Thread for receiving data packets. Packets are forwarded to
        let data_rx_thread = Builder::new().name("data_rx".into());
        data_rx_thread
            .spawn(move || {
                if udpnet::bcast::rx(msg_port, custom_net_data_recv_tx).is_err() {
                    process::exit(1);
                }
            })
            .unwrap();

        Ok(Network { id: id })
    }
}
