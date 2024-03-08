/*
 * Unit tests for coordinator module
 * 
 * The unit tests follows the Arrange, Act, Assert pattern.
 * 
 * Currently the tests doesn't work as the coordinator needs to run on amd64 architecture.
 * 
 * Tests:
 *  - test_coordinator_init
 *  - test_coordinator_update_lights
 */

/***************************************/
/*             Unit tests              */
/***************************************/
#[cfg(test)]
mod coordinator_tests {
    use crate::Coordinator;
    use crate::ElevatorState;
    use crate::ElevatorData;
    use crate::shared::Behaviour::{Idle, Moving, DoorOpen};
    use crate::shared::Direction::{Up, Down, Stop};
    use driver_rust::elevio::elev::{HALL_DOWN, HALL_UP, CAB};
    use network_rust::udpnet::peers::PeerUpdate;
    use std::thread::Builder;
    use crossbeam_channel::unbounded;
    use crossbeam_channel::Receiver;
    use crossbeam_channel::Sender;

    fn setup_coordinator() -> (
        Coordinator,
        Receiver<(u8, u8, bool)>,   // hw_button_light_rx
        Sender<(u8, u8)>,           // hw_request_tx
        Receiver<Vec<Vec<bool>>>,   // fsm_hall_requests_rx
        Receiver<u8>,               // fsm_cab_request_rx
        Sender<ElevatorState>,      // fsm_state_tx
        Sender<(u8, u8)>,           // fsm_order_complete_tx
        Receiver<ElevatorData>,     // net_data_send_rx
        Sender<ElevatorData>,       // net_data_recv_tx
        Sender<PeerUpdate>,         // net_peer_update_tx
        Sender<()>) {               // coordinator_terminate_tx

        // Arrange mock channels
        let (hw_button_light_tx, hw_button_light_rx) = unbounded::<(u8, u8, bool)>();
        let (hw_request_tx, hw_request_rx) = unbounded::<(u8, u8)>();
        let (fsm_hall_requests_tx, fsm_hall_requests_rx) = unbounded::<Vec<Vec<bool>>>();
        let (fsm_cab_request_tx, fsm_cab_request_rx) = unbounded::<u8>();
        let (fsm_state_tx, fsm_state_rx) = unbounded::<ElevatorState>();
        let (fsm_order_complete_tx, fsm_order_complete_rx) = unbounded::<(u8, u8)>();
        let (net_data_send_tx, net_data_send_rx) = unbounded::<ElevatorData>();
        let (net_data_recv_tx, net_data_recv_rx) = unbounded::<ElevatorData>();
        let (net_peer_update_tx, net_peer_update_rx) = unbounded::<PeerUpdate>();
        let (coordinator_terminate_tx, coordinator_terminate_rx) = unbounded::<()>();
        
        // Default configuration
        let n_floors = 4;
        let id = "elevator".to_string();
        let mut elevator_data = ElevatorData::new(n_floors.clone());
        elevator_data.states.insert(id.clone(), ElevatorState::new(n_floors.clone()));

        (Coordinator::new(
            elevator_data,
            id,
            n_floors,
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
        ),
        hw_button_light_rx,
        hw_request_tx,
        fsm_hall_requests_rx,
        fsm_cab_request_rx,
        fsm_state_tx,
        fsm_order_complete_tx,
        net_data_send_rx,
        net_data_recv_tx,
        net_peer_update_tx,
        coordinator_terminate_tx)
    }

    #[test]
    fn test_coordinator_init() {
        // Arrange
        let (
            coordinator,
            _hw_button_light_rx,
            _hw_request_tx,
            _fsm_hall_requests_rx,
            _fsm_cab_request_rx,
            _fsm_state_tx,
            _fsm_order_complete_tx,
            _net_data_send_rx,
            _net_data_recv_tx,
            _net_peer_update_tx,
            _coordinator_terminate_tx
        ) = setup_coordinator();

        // Default configuration
        let n_floors = 4;
        let id = "elevator".to_string();
        let mut elevator_data = ElevatorData::new(n_floors.clone());
        elevator_data.states.insert(id.clone(), ElevatorState::new(n_floors.clone()));

        // Assert
        assert_eq!(*coordinator.test_get_data(), elevator_data);
        assert_eq!(*coordinator.test_get_local_id(), id);
        assert_eq!(*coordinator.test_get_n_floors(), 4);
    }

    #[test]
    fn test_coordinator_update_lights() {
        // Arrange
        let (
            mut coordinator,
            hw_button_light_rx,
            hw_request_tx,
            _fsm_hall_requests_rx,
            _fsm_cab_request_rx,
            _fsm_state_tx,
            _fsm_order_complete_tx,
            _net_data_send_rx,
            _net_data_recv_tx,
            _net_peer_update_tx,
            coordinator_terminate_tx
        ) = setup_coordinator();

        let n_floors = coordinator.test_get_n_floors().clone();
        let coordinator_thread = Builder::new().name("coordinator".into()).spawn(move || coordinator.run()).unwrap();

        // Act
        for floor in 0..n_floors {
            hw_request_tx.send((floor, HALL_UP)).unwrap();
            hw_request_tx.send((floor, HALL_DOWN)).unwrap();
            hw_request_tx.send((floor, CAB)).unwrap();
        }

        // Assert
        for floor in 0..n_floors {
            assert_eq!(hw_button_light_rx.recv().unwrap(), (floor, HALL_UP, true));
            assert_eq!(hw_button_light_rx.recv().unwrap(), (floor, HALL_DOWN, true));
            assert_eq!(hw_button_light_rx.recv().unwrap(), (floor, CAB, true));
        }

        // Cleanup
        coordinator_terminate_tx.send(()).unwrap();
        coordinator_thread.join().unwrap();
    }
    
}
