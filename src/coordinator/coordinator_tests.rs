/*
 * Unit tests for coordinator module
 * 
 * The unit tests follows the Arrange, Act, Assert pattern.
 * 
 * Tests:
 *  - test_coordinator_init
 *  - test_coordinator_update_lights
 *  - test_coordinator_check_version
 *  - test_coordinator_hall_request_assigner
 *  - test_coordinator_handle_event_new_package
 *  - test_coordinator_handle_event_request_received
 *  - test_coordinator_handle_event_new_peer_update
 *  - test_coordinator_handle_event_new_elevator_state
 *  - test_coordinator_handle_event_order_complete
 * 
 */

/***************************************/
/*             Unit tests              */
/***************************************/
#[cfg(test)]
mod coordinator_tests {
    use crate::coordinator::coordinator::Event;
    use crate::Coordinator;
    use crate::ElevatorState;
    use crate::ElevatorData;
    use crate::shared::Direction::Up;
    use std::time::Duration;
    use std::thread::Builder;
    use core::panic;
    use driver_rust::elevio::elev::{HALL_DOWN, HALL_UP, CAB};
    use network_rust::udpnet::peers::PeerUpdate;
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
            coordinator,
            hw_button_light_rx,
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

        let n_floors = coordinator.test_get_n_floors().clone();
        let timeout = Duration::from_millis(500);

        // Act / Assert
        for floor in 0..n_floors {
            coordinator.test_update_lights((floor, HALL_UP, true));
            match hw_button_light_rx.recv_timeout(timeout) {
                Ok(msg) => assert_eq!(msg, (floor, HALL_UP, true), "Mismatch for floor {} HALL_UP", floor),
                Err(e) => panic!("Error receiving HALL_UP for floor {}: {:?}", floor, e),
            }
    
            coordinator.test_update_lights((floor, HALL_DOWN, true));
            match hw_button_light_rx.recv_timeout(timeout) {
                Ok(msg) => assert_eq!(msg, (floor, HALL_DOWN, true), "Mismatch for floor {} HALL_DOWN", floor),
                Err(e) => panic!("Error receiving HALL_DOWN for floor {}: {:?}", floor, e),
            }
    
            coordinator.test_update_lights((floor, CAB, true));
            match hw_button_light_rx.recv_timeout(timeout) {
                Ok(msg) => assert_eq!(msg, (floor, CAB, true), "Mismatch for floor {} CAB", floor),
                Err(e) => panic!("Error receiving CAB for floor {}: {:?}", floor, e),
            }
        }
    }

    #[test]
    fn test_coordinator_hall_request_assigner() {
        // Arrange
        let (
            mut coordinator,
            _hw_button_light_rx,
            _hw_request_tx,
            fsm_hall_requests_rx,
            _fsm_cab_request_rx,
            _fsm_state_tx,
            _fsm_order_complete_tx,
            net_data_send_rx,
            _net_data_recv_tx,
            _net_peer_update_tx,
            _coordinator_terminate_tx
        ) = setup_coordinator();

        let n_floors = coordinator.test_get_n_floors().clone();
        let timeout = Duration::from_millis(500);

        // Floor above going up
        let mut hall_requests = vec![vec![false; 2]; n_floors as usize];
        hall_requests[2][HALL_UP as usize] = true;

        // Set state of local elevator
        let id = "elevator".to_string();
        let state = ElevatorState::new(n_floors.clone());
        
        // Act
        coordinator.test_set_state(id.clone(), state.clone());
        coordinator.test_set_hall_requests(hall_requests.clone());

        // Hall requests should be assigned to local elevator
        coordinator.test_hall_request_assigner(false);
        match fsm_hall_requests_rx.recv_timeout(timeout) {
            Ok(msg) => assert_eq!(msg, hall_requests.clone(), "Mismatch for hall_requests"),
            Err(e) => panic!("Error receiving hall_requests: {:?}", e),
        }

        // Hall request should not be transmitted to net_data_send_rx
        match fsm_hall_requests_rx.try_recv() {
            Ok(_) => panic!("hall_requests should not be transmitted to net_data_send_rx"),
            Err(_) => (),
        }

        // Reset state and hall requests and perform test with network transmission
        coordinator.test_set_state(id.clone(), state.clone());
        coordinator.test_set_hall_requests(hall_requests.clone());

        // Hall requests should be assigned to local elevator
        coordinator.test_hall_request_assigner(true);
        match fsm_hall_requests_rx.recv_timeout(timeout) {
            Ok(msg) => assert_eq!(msg, hall_requests.clone(), "Mismatch for hall_requests"),
            Err(e) => panic!("Error receiving hall_requests: {:?}", e),
        }

        // Hall request should be transmitted to net_data_send_rx
        match net_data_send_rx.recv_timeout(timeout) {
            Ok(msg) => {
                let mut expected_data = ElevatorData::new(n_floors.clone());
                expected_data.version = 1;
                expected_data.hall_requests = hall_requests.clone();
                expected_data.states.insert(id.clone(), state.clone());
                assert_eq!(msg, expected_data, "Mismatch for net_data_send_rx");
            },
            Err(e) => panic!("Error receiving net_data_send_rx: {:?}", e),
        }
        
    }

    #[test]
    fn test_coordinator_handle_event_new_package() {
        // Arrange
        let (
            mut coordinator,
            hw_button_light_rx,
            _hw_request_tx,
            fsm_hall_requests_rx,
            _fsm_cab_request_rx,
            _fsm_state_tx,
            _fsm_order_complete_tx,
            _net_data_send_rx,
            net_data_recv_tx,
            _net_peer_update_tx,
            coordinator_terminate_tx
        ) = setup_coordinator();

        let timeout = Duration::from_millis(500);
        let n_floors = coordinator.test_get_n_floors().clone();
        let mut new_package = ElevatorData::new(n_floors);
        new_package.states.insert("elevator".to_string(), ElevatorState::new(n_floors));
        new_package.version = 1;
        new_package.hall_requests = vec![vec![false; 2]; n_floors as usize];
        new_package.hall_requests[2][HALL_UP as usize] = true;

        let coordinator_thread = Builder::new().name("coordinator".into()).spawn(move || coordinator.run()).unwrap();
            
        // Act
        net_data_recv_tx.send(new_package.clone()).unwrap();

        // Assert
        match hw_button_light_rx.recv_timeout(timeout) {
            Ok(msg) => assert_eq!(msg, (2, HALL_UP, true), "Mismatch for hw_button_light_rx"),
            Err(e) => panic!("Error receiving hw_button_light_rx: {:?}", e),
        }

        match fsm_hall_requests_rx.recv_timeout(timeout) {
            Ok(msg) => assert_eq!(msg, new_package.hall_requests, "Mismatch for fsm_hall_requests_rx"),
            Err(e) => panic!("Error receiving fsm_hall_requests_rx: {:?}", e),
        }

        // Cleanup
        coordinator_terminate_tx.send(()).unwrap();
        coordinator_thread.join().unwrap();
        
    }

    #[test]
    fn test_coordinator_handle_event_request_received() {
        // Arrange
        let (
            mut coordinator,
            hw_button_light_rx,
            hw_request_tx,
            fsm_hall_requests_rx,
            fsm_cab_request_rx,
            _fsm_state_tx,
            _fsm_order_complete_tx,
            net_data_send_rx,
            _net_data_recv_tx,
            _net_peer_update_tx,
            coordinator_terminate_tx
        ) = setup_coordinator();

        let timeout = Duration::from_millis(500);
        let n_floors = coordinator.test_get_n_floors().clone();
        let coordinator_thread = Builder::new().name("coordinator".into()).spawn(move || coordinator.run()).unwrap();
            
        // Act / Assert
        // New hall request
        hw_request_tx.send((2, HALL_UP)).unwrap();

        match fsm_hall_requests_rx.recv_timeout(timeout) {
            Ok(msg) => {
                let mut expected_hall_requests = vec![vec![false; 2]; n_floors as usize];
                expected_hall_requests[2][HALL_UP as usize] = true;
                assert_eq!(msg, expected_hall_requests, "Mismatch for fsm_hall_requests_rx");
            },
            Err(e) => panic!("Error receiving fsm_hall_requests_rx: {:?}", e),
        }

        match net_data_send_rx.recv_timeout(timeout) {
            Ok(msg) => {
                let mut expected_data = ElevatorData::new(n_floors);
                expected_data.version = 1;
                expected_data.hall_requests = vec![vec![false; 2]; n_floors as usize];
                expected_data.hall_requests[2][HALL_UP as usize] = true;
                expected_data.states.insert("elevator".to_string(), ElevatorState::new(n_floors));
                assert_eq!(msg, expected_data, "Mismatch for net_data_send_rx");
            },
            Err(e) => panic!("Error receiving net_data_send_rx: {:?}", e),
        }

        match hw_button_light_rx.recv_timeout(timeout) {
            Ok(msg) => assert_eq!(msg, (2, HALL_UP, true), "Mismatch for hw_button_light_rx"),
            Err(e) => panic!("Error receiving hw_button_light_rx: {:?}", e),
        }

        // New cab request
        hw_request_tx.send((2, CAB)).unwrap();

        match fsm_cab_request_rx.recv_timeout(timeout) {
            Ok(msg) => assert_eq!(msg, 2, "Mismatch for fsm_cab_request_rx"),
            Err(e) => panic!("Error receiving fsm_cab_request_rx: {:?}", e),
        }

        match hw_button_light_rx.recv_timeout(timeout) {
            Ok(msg) => assert_eq!(msg, (2, CAB, true), "Mismatch for hw_button_light_rx"),
            Err(e) => panic!("Error receiving hw_button_light_rx: {:?}", e),
        }

        // Cleanup
        coordinator_terminate_tx.send(()).unwrap();
        coordinator_thread.join().unwrap();
    }

    #[test]
    fn test_coordinator_handle_event_new_peer_update() {
        // Arrange
        let (
            mut coordinator,
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

        let mut expected_peer_list = vec!["peer1".to_string(), "peer2".to_string(), "elevator".to_string()];
        let peer_update = PeerUpdate {
            peers: expected_peer_list.clone(),
            new: Some("peer1".to_string()),
            lost: vec!["peer3".to_string()],
        };

        let coordinator_peer_list = PeerUpdate {
            peers: vec!["peer2".to_string(), "peer3".to_string(), "elevator".to_string()],
            new: None,
            lost: Vec::new(),
        };

        coordinator.test_set_peer_list(coordinator_peer_list);
            
        // Act
        coordinator.test_handle_event(Event::NewPeerUpdate(peer_update));

        // Assert
        let mut peer_list = coordinator.test_get_peer_list();
        peer_list.sort();
        expected_peer_list.sort();
        assert_eq!(peer_list, expected_peer_list, "Mismatch for peer_list.peers");
    }

    #[test]
    fn test_coordinator_handle_event_new_elevator_state() {
        // Arrange
        let (
            mut coordinator,
            hw_button_light_rx,
            _hw_request_tx,
            fsm_hall_requests_rx,
            _fsm_cab_request_rx,
            fsm_state_tx,
            _fsm_order_complete_tx,
            net_data_send_rx,
            _net_data_recv_tx,
            _net_peer_update_tx,
            coordinator_terminate_tx
        ) = setup_coordinator();

        let timeout = Duration::from_millis(500);
        let n_floors = coordinator.test_get_n_floors().clone();
        let mut new_state = ElevatorState::new(n_floors);
        new_state.floor = 2;
        new_state.direction = Up;
        new_state.cab_requests = vec![false; n_floors as usize];
        new_state.cab_requests[3] = true;

        let expected_hall_requests = vec![vec![false; 2]; n_floors as usize];
        let mut expected_elevator_data = ElevatorData::new(n_floors);
        expected_elevator_data.version = 1;
        expected_elevator_data.hall_requests = expected_hall_requests.clone();
        expected_elevator_data.states.insert("elevator".to_string(), new_state.clone());

        let coordinator_thread = Builder::new().name("coordinator".into()).spawn(move || coordinator.run()).unwrap();
            
        // Act
        fsm_state_tx.send(new_state.clone()).unwrap();

        // Assert
        match hw_button_light_rx.recv_timeout(timeout) {
            Ok(msg) => assert_eq!(msg, (3, CAB, true), "Mismatch for hw_button_light_rx"),
            Err(e) => panic!("Error receiving hw_button_light_rx: {:?}", e),
        }

        match fsm_hall_requests_rx.recv_timeout(timeout) {
            Ok(msg) => assert_eq!(msg, expected_hall_requests, "Mismatch for fsm_hall_requests_rx"),
            Err(e) => panic!("Error receiving fsm_hall_requests_rx: {:?}", e),
        }

        match net_data_send_rx.recv_timeout(timeout) {
            Ok(msg) => assert_eq!(msg, expected_elevator_data, "Mismatch for net_data_send_rx"),
            Err(e) => panic!("Error receiving net_data_send_rx: {:?}", e),
        }
        
        // Cleanup
        coordinator_terminate_tx.send(()).unwrap();
        coordinator_thread.join().unwrap();
    }

    #[test]
    fn test_coordinator_handle_event_order_complete() {
        // Arrange
        let (
            mut coordinator,
            hw_button_light_rx,
            _hw_request_tx,
            fsm_hall_requests_rx,
            _fsm_cab_request_rx,
            _fsm_state_tx,
            fsm_order_complete_tx,
            net_data_send_rx,
            _net_data_recv_tx,
            _net_peer_update_tx,
            coordinator_terminate_tx
        ) = setup_coordinator();

        let timeout = Duration::from_millis(500);
        let n_floors = coordinator.test_get_n_floors().clone();

        let coordinator_thread = Builder::new().name("coordinator".into()).spawn(move || coordinator.run()).unwrap();
            
        // Act
        fsm_order_complete_tx.send((2, HALL_DOWN)).unwrap();

        // Assert
        match hw_button_light_rx.recv_timeout(timeout) {
            Ok(msg) => assert_eq!(msg, (2, HALL_DOWN, false), "Mismatch for hw_button_light_rx"),
            Err(e) => panic!("Error receiving hw_button_light_rx: {:?}", e),
        }

        match fsm_hall_requests_rx.recv_timeout(timeout) {
            Ok(msg) => assert_eq!(msg, vec![vec![false; 2]; n_floors.clone() as usize], "Mismatch for fsm_hall_requests_rx"),
            Err(e) => panic!("Error receiving fsm_hall_requests_rx: {:?}", e),
        }

        match net_data_send_rx.recv_timeout(timeout) {
            Ok(msg) => {
                let mut expected_elevator_data = ElevatorData::new(n_floors);
                expected_elevator_data.version = 1;
                expected_elevator_data.hall_requests = vec![vec![false; 2]; n_floors.clone() as usize];
                expected_elevator_data.states.insert("elevator".to_string(), ElevatorState::new(n_floors));
                assert_eq!(msg, expected_elevator_data, "Mismatch for net_data_send_rx");
            },
            Err(e) => panic!("Error receiving net_data_send_rx: {:?}", e),
        }

        // Cleanup
        coordinator_terminate_tx.send(()).unwrap();
        coordinator_thread.join().unwrap();
    }

}
