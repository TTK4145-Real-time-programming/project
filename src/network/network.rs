use std::env;
use std::net;
use std::process;
use std::thread::*;

use crate::config::NetworkConfig;
use crossbeam_channel as cbc;
use network_rust::udpnet;

// Data types to be sent on the network must derive traits for serialization
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct CustomDataType {
    message: String,
    iteration: u64,
}

pub struct Network {
    pub id: String,
    pub peer_tx_enable_tx: cbc::Sender<bool>,
    pub custom_data_send_tx: cbc::Sender<CustomDataType>,
    pub custom_data_recv_rx: cbc::Receiver<CustomDataType>,
    pub peer_update_rx: cbc::Receiver<udpnet::peers::PeerUpdate>,
}

impl Network {
    pub fn new(config: &NetworkConfig) -> std::io::Result<Network> {
        let (peer_tx_enable_tx, peer_tx_enable_rx) = cbc::unbounded::<bool>();
        let (peer_update_tx, peer_update_rx) = cbc::unbounded::<udpnet::peers::PeerUpdate>();
        let (custom_data_send_tx, custom_data_send_rx) = cbc::unbounded::<CustomDataType>();
        let (custom_data_recv_tx, custom_data_recv_rx) = cbc::unbounded::<CustomDataType>();

        let id = if env::args().len() > 1 {
            env::args().nth(1).unwrap()
        } else {
            let local_ip = net::TcpStream::connect(config.id_gen_address.clone())
                .unwrap()
                .local_addr()
                .unwrap()
                .ip();
            format!("rust@{}#{}", local_ip, process::id())
        };

        let msg_port = config.msg_port;
        let peer_port = config.peer_port;
        let id_tx = id.clone();

        // Thread for broadcasting peer ID
        spawn(move || {
            if udpnet::peers::tx(peer_port, id_tx, peer_tx_enable_rx).is_err() {
                process::exit(1);
            }
        });

        // Thread for receiving and forwarding peer updates on port 'peer_port'
        spawn(move || {
            if udpnet::peers::rx(peer_port, peer_update_tx).is_err() {
                process::exit(1);
            }
        });

        // Thread for sending out data packets. Packets are receiver from
        // the 'custom_data_send_tx' channel
        spawn(move || {
            if udpnet::bcast::tx(msg_port, custom_data_send_rx).is_err() {
                process::exit(1);
            }
        });

        // Thread for receiving data packets. Packets are forwarded to
        // the 'custom_data_recv_rx' channel
        spawn(move || {
            if udpnet::bcast::rx(msg_port, custom_data_recv_tx).is_err() {
                process::exit(1);
            }
        });

        Ok(Network {
            id: id,
            peer_tx_enable_tx: peer_tx_enable_tx,
            custom_data_send_tx: custom_data_send_tx,
            custom_data_recv_rx: custom_data_recv_rx,
            peer_update_rx: peer_update_rx,
        })
    }
}
