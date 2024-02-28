use serde::Deserialize;
use std::fs;

#[derive(Deserialize)]
pub struct Config {
    pub network: NetworkConfig,
}

#[derive(Deserialize)]
pub struct NetworkConfig {
    pub id_gen_address: String,
    pub msg_port: u16,
    pub peer_port: u16,
}

pub fn load_config() -> Config {
    let config_str = fs::read_to_string("config.toml")
        .expect("Failed to read configuration file");
    toml::from_str(&config_str)
        .expect("Failed to parse configuration file")
}