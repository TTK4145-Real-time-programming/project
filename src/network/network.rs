use std::env;
use std::net;
use std::process;
use std::thread::*;
use std::time::Duration;

use crossbeam_channel as cbc;
use network_rust::udpnet;
use crate::config::NetworkConfig;

// Data types to be sent on the network must derive traits for serialization
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
struct CustomDataType {
    message: String,
    iteration: u64,
}

pub fn network(config: &NetworkConfig) -> std::io::Result<()> {
    // Genreate id: either from command line, or a default rust@ip#pid
    let args: Vec<String> = env::args().collect();
    let id = if args.len() > 1 {
        args[1].clone()
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


    // The sender for peer discovery
    let (peer_tx_enable_tx, peer_tx_enable_rx) = cbc::unbounded::<bool>();
    let _handler = {
        let id = id.clone();
        spawn(move || {
            if udpnet::peers::tx(peer_port, id, peer_tx_enable_rx).is_err() {
                // crash program if creating the socket fails (`peers:tx` will always block if the
                // initialization succeeds)
                process::exit(1);
            }
        })
    };

    // The receiver for peer discovery updates
    let (peer_update_tx, peer_update_rx) = cbc::unbounded::<udpnet::peers::PeerUpdate>();
    {
        spawn(move || {
            if udpnet::peers::rx(peer_port, peer_update_tx).is_err() {
                // crash program if creating the socket fails (`peers:rx` will always block if the
                // initialization succeeds)
                process::exit(1);
            }
        });
    }

    // Data transmission channel
    // Use:
    // custom_data_send_tx.send(data).unwrap();
    // To send data
    let (custom_data_send_tx, custom_data_send_rx) = cbc::unbounded::<CustomDataType>();

    // The sender for our custom data
    {
        spawn(move || {
            if udpnet::bcast::tx(msg_port, custom_data_send_rx).is_err() {
                // crash program if creating the socket fails (`bcast:tx` will always block if the
                // initialization succeeds)
                process::exit(1);
            }
        });
    }

    // The receiver for our custom data
    let (custom_data_recv_tx, custom_data_recv_rx) = cbc::unbounded::<CustomDataType>();
    spawn(move || {
        if udpnet::bcast::rx(msg_port, custom_data_recv_tx).is_err() {
            // crash program if creating the socket fails (`bcast:rx` will always block if the
            // initialization succeeds)
            process::exit(1);
        }
    });

    // main body: receive peer updates and data from the network
    loop {
        cbc::select! {
            recv(peer_update_rx) -> a => {
                let update = a.unwrap();
                println!("{:#?}", update);
            }
            recv(custom_data_recv_rx) -> a => {
                let cd = a.unwrap();
                println!("{:#?}", cd);
            }
        }
    }
}