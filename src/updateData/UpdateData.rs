/*
    Purpose of this module is to update the order list from the dirffrent 'channels' in the system.
    The channels are:
    - Hall buttons (up and down)
    - Cab calls
    - Network messages

    Into a JSON file that is used by HRA to calculate the orders for the elevators.
*/


use driver_rust::elevio::elev::Elevator;
use driver_rust::elevio::elev::{CAB, DIRN_DOWN, DIRN_STOP, DIRN_UP, HALL_DOWN, HALL_UP};

enum GlobalEvent {
    RequestReceived(u8, u8),
    GlobalRequestReceived(),
    StopPressed,
}

HallRequests = Vec<BooleanPair>;

pub struct ElevatorState {
    behaviour: Behaviour,
    floor: Option<u8>,
    direction: Direction,
}

type States = HasshMap<String, ElevatorState>;

pub struct GlobalOrders{
    version: u64,
    hallRequests: HallRequests,
    states: States,
}

impl UpdateData{
    // Have a new function here

    fn wait_for_event(&mut self) -> Event {
        /*
            TODO: Should this function look for event by itself?
                Or will network and local buttons trigger this?
         */
        // Wait for event from the channels
        // Return the event
    }

    fn update_data(&mut self, event: Event) {
        // Update the data based on the event and Current JSON
        // Write the updated data to the JSON
    }

    //Write the updated data to the JSON
    fn write_to_json(&self) {
        // Write the data to the JSON file
    }

    //Read local data from JSON
    fn read_from_json(&self) -> GlobalOrders{
        // Read the data from the JSON file
        // Return the data
    }
}
