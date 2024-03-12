/***************************************/
/*        3rd party libraries          */
/***************************************/
use serde::Deserialize;
use serde::Serialize;
use std::fs;
use std::io::Write;

#[derive(Deserialize, Serialize, Clone)]
pub struct CabOrders {
    pub cab_calls: Vec<bool>,
}

pub fn load_cab_orders() -> CabOrders {
    let config_str = fs::read_to_string("src/elevator/cab_orders.toml").expect("Failed to read configuration file");
    toml::from_str(&config_str).expect("Failed to parse configuration file")
}

pub fn save_cab_orders(cab_orders: Vec<bool>){
    // Create a CabOrders instance 
    let cab_orders_struct = CabOrders { cab_calls: cab_orders };

    // Serialize the CabOrders instance to a TOML string
    let toml_string = toml::to_string(&cab_orders_struct)
        .expect("Failed to serialize cab orders");

    // Write the TOML string to a file
    let mut file = fs::File::create("src/elevator/cab_orders.toml")
        .expect("Failed to create/open the file");

    file.write_all(toml_string.as_bytes())
        .expect("Failed to write to the file");
}