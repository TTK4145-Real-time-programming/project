/***************************************/
/*        3rd party libraries          */
/***************************************/
use serde::{Deserialize, Serialize};
use std::fs;

/***************************************/
/*       Public data structures        */
/***************************************/
#[derive(Deserialize, Serialize, Clone)]
pub struct Config {
    pub network: NetworkConfig,
    pub elevator: ElevatorConfig,
    pub hardware: HardwareConfig,
    pub orders: CabOrders,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct NetworkConfig {
    pub id_gen_address: String,
    pub msg_port: u16,
    pub peer_port: u16,
    pub max_retries: u32,
    pub ack_timeout: u64,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct ElevatorConfig {
    pub n_floors: u8,
    pub door_open_time: u64,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct HardwareConfig {
    pub n_floors: u8,
    pub driver_address: String,
    pub driver_port: u16,
    pub hw_thread_sleep_time: u64,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct CabOrders {
    pub cab_calls: Vec<bool>,
}

/***************************************/
/*             Public API              */
/***************************************/
pub fn load_config() -> Config {
    let config_str = fs::read_to_string("config.toml").expect("Failed to read configuration file");
    toml::from_str(&config_str).expect("Failed to parse configuration file")
}
