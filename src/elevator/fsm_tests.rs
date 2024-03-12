/*
 * Unit tests for elevator module
 * 
 * The unit tests follows the Arrange, Act, Assert pattern.
 * 
 * Tests:
 * - test_elevator_fsm_new_initial_state 
 * - test_elevator_fsm_new_floor_sensor
 * 
 */

/***************************************/
/*             Unit tests              */
/***************************************/
#[cfg(test)]
mod fsm_tests {
    use std::thread::spawn;
    use crate::ElevatorFSM;
    use crate::ElevatorState;
    use crate::config::ElevatorConfig;
    use crate::shared::Behaviour::{Idle, Moving, DoorOpen};
    use crate::shared::Direction::{Up, Down, Stop};
    use crossbeam_channel::unbounded;
    use crate::shared::Direction;
    use driver_rust::elevio::elev::DIRN_STOP;

    fn setup_fsm() -> (ElevatorFSM,
        crossbeam_channel::Receiver<u8>,
        crossbeam_channel::Sender<u8>,
        crossbeam_channel::Receiver<bool>,
        crossbeam_channel::Sender<bool>,
        crossbeam_channel::Sender<bool>,
        crossbeam_channel::Sender<Vec<Vec<bool>>>,
        crossbeam_channel::Sender<u8>,
        crossbeam_channel::Receiver<(u8, u8)>,
        crossbeam_channel::Receiver<ElevatorState>,
        crossbeam_channel::Sender<()>) {

        // Arrange mock channels
        let (hw_motor_direction_tx, hw_motor_direction_rx) = unbounded::<u8>();
        let (hw_floor_sensor_tx, hw_floor_sensor_rx) = unbounded::<u8>();
        let (hw_door_light_tx, hw_door_light_rx) = unbounded::<bool>();
        let (hw_obstruction_tx, hw_obstruction_rx) = unbounded::<bool>();
        let (hw_stop_button_tx, hw_stop_button_rx) = unbounded::<bool>();
        let (fsm_hall_requests_tx, fsm_hall_requests_rx) = unbounded::<Vec<Vec<bool>>>();
        let (fsm_cab_request_tx, fsm_cab_request_rx) = unbounded::<u8>();
        let (fsm_order_complete_tx, fsm_order_complete_rx) = unbounded::<(u8, u8)>();
        let (fsm_state_tx, fsm_state_rx) = unbounded::<ElevatorState>();
        let (fsm_terminate_tx, fsm_terminate_rx) = unbounded::<()>();

        // Default configuration
        let config = ElevatorConfig { 
            n_floors: 4,
            door_open_time: 3000,
            motor_driving_timeout: 10000,
            door_timeout: 20000,
        };

        // Create the FSM and return it with the channels
        (ElevatorFSM::new(
            &config,
            hw_motor_direction_tx,
            hw_floor_sensor_rx,
            hw_door_light_tx,
            hw_obstruction_rx,
            hw_stop_button_rx,
            fsm_hall_requests_rx,
            fsm_cab_request_rx,
            fsm_order_complete_tx,
            fsm_state_tx,
            fsm_terminate_rx,
        ),
        hw_motor_direction_rx,
        hw_floor_sensor_tx,
        hw_door_light_rx,
        hw_obstruction_tx,
        hw_stop_button_tx,
        fsm_hall_requests_tx,
        fsm_cab_request_tx,
        fsm_order_complete_rx,
        fsm_state_rx,
        fsm_terminate_tx)
    }

    #[test]
    fn test_fsm_init() {
        // Purpose: Verify that the FSM is in the expected initial state after creation

        // Arrange
        let (fsm,
            _hw_motor_direction_rx,
            hw_floor_sensor_tx,
            _hw_door_light_rx,
            _hw_obstruction_tx,
            _hw_stop_button_tx,
            _fsm_hall_requests_tx,
            _fsm_cab_request_tx,
            _fsm_order_complete_rx,
            fsm_state_rx,
            terminate_tx) = setup_fsm();

        let fsm_thread = spawn(move || fsm.run());

        // Act
        match fsm_state_rx.recv_timeout(std::time::Duration::from_secs(3)) {
            Ok(state) => {
                //Disregarding 
            },
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                panic!("Timed out waiting for fsm_state_rx");
            },
            Err(e) => {
                panic!("Error receiving from fsm_state_rx: {:?}", e);
            }
        }
        
        // Simulate the elevator hitting floor 0 after creation
        hw_floor_sensor_tx.send(1).unwrap();

        // Assert

        match fsm_state_rx.recv_timeout(std::time::Duration::from_secs(3)) {
            Ok(state) => {
                assert_eq!(state.behaviour, Idle);
                assert_eq!(state.direction, Stop);
                assert_eq!(state.floor, 1);
            },
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                panic!("Timed out waiting for fsm_state_rx");
            },
            Err(e) => {
                panic!("Error receiving from fsm_state_rx: {:?}", e);
            }
        }

        // Cleanup
        terminate_tx.send(()).unwrap();
        fsm_thread.join().unwrap();
    }

    #[test]
    fn test_fsm_floor_hit() {
        // Purpose: Verify that the FSM updates the floor when the floor sensor is triggered

        // Arrange
        let (fsm,
            _hw_motor_direction_rx,
            hw_floor_sensor_tx,
            _hw_door_light_rx,
            _hw_obstruction_tx,
            _hw_stop_button_tx,
            _fsm_hall_requests_tx,
            _fsm_cab_request_tx,
            _fsm_order_complete_rx,
            fsm_state_rx,
            terminate_tx) = setup_fsm();

        let fsm_thread = spawn(move || fsm.run());

        // Act
        // Simulate the elevator hitting floor 1
        hw_floor_sensor_tx.send(1).unwrap();

        // Assert
        match fsm_state_rx.recv_timeout(std::time::Duration::from_secs(3)) {
            Ok(state) => {
                //Disregarding first update as this is part of init 
            },
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                panic!("Timed out waiting for fsm_state_rx");
            },
            Err(e) => {
                panic!("Error receiving from fsm_state_rx: {:?}", e);
            }
        }

        match fsm_state_rx.recv_timeout(std::time::Duration::from_secs(3)) {
            Ok(state) => {
                assert_eq!(state.behaviour, Idle);
                assert_eq!(state.direction, Stop);
                assert_eq!(state.floor, 1);
            },
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                panic!("Timed out waiting for fsm_state_rx");
            },
            Err(e) => {
                panic!("Error receiving from fsm_state_rx: {:?}", e);
            }
        }

        // Cleanup
        terminate_tx.send(()).unwrap();
        fsm_thread.join().unwrap();
    }

    //#[test]
    fn test_fsm_choose_direction() {
        // Purpose: Verify that the FSM chooses the correct direction when the floor sensor is triggered

        // Arrange
        let (mut fsm,
            _hw_motor_direction_rx,
            _hw_floor_sensor_tx,
            _hw_door_light_rx,
            _hw_obstruction_tx,
            _hw_stop_button_tx,
            _fsm_hall_requests_tx,
            _fsm_cab_request_tx,
            _fsm_order_complete_rx,
            _fsm_state_rx,
            _terminate_tx) = setup_fsm();

        //Testing no orders
        let state1 = ElevatorState {
            behaviour: Moving,
            floor: 0,
            direction: Stop,
            cab_requests: [false, false, false, false].to_vec(),
        };
        //Testing orders above
        let state2 = ElevatorState {
            behaviour: Moving,
            floor: 1,
            direction: Stop,
            cab_requests: [false, false, true, true].to_vec(),
        };
        //testing orders below
        let state3 = ElevatorState {
            behaviour: Moving,
            floor: 1,
            direction: Stop,
            cab_requests: [true, false, false, false].to_vec(),
        };
        //testing orders at current floor
        let state4 = ElevatorState {
            behaviour: Moving,
            floor: 3,
            direction: Stop,
            cab_requests: [false, false, false, true].to_vec(),
        };

        // Act
        fsm.test_set_state(state1);
        let direction1 = fsm.test_choose_direction();
        fsm.test_set_state(state2);
        let direction2 = fsm.test_choose_direction();
        fsm.test_set_state(state3);
        let direction3 = fsm.test_choose_direction();
        fsm.test_set_state(state4);
        let direction4 = fsm.test_choose_direction();

        // Assert
        assert_eq!(direction1, Stop);
        assert_eq!(direction2, Up);
        assert_eq!(direction3, Down);
        assert_eq!(direction4, Stop);

    }

    #[test]
    fn test_fsm_has_orders_in_directions() {
        // Arrange
        let (mut fsm,
            _hw_motor_direction_rx,
            _hw_floor_sensor_tx,
            _hw_door_light_rx,
            _hw_obstruction_tx,
            _hw_stop_button_tx,
            _fsm_hall_requests_tx,
            _fsm_cab_request_tx,
            _fsm_order_complete_rx,
            _fsm_state_rx,
            _terminate_tx) = setup_fsm();

        //Testing no orders
        let state1 = ElevatorState {
            behaviour: Moving,
            floor: 0,
            direction: Stop,
            cab_requests: [false, false, false, false].to_vec(),
        };
        //Testing above
        let state2 = ElevatorState {
            behaviour: Moving,
            floor: 0,
            direction: Stop,
            cab_requests: [false, true, false, false].to_vec(),
        };
        //Testing below
        let state3 = ElevatorState {
            behaviour: Moving,
            floor: 2,
            direction: Stop,
            cab_requests: [true, false, false, false].to_vec(),
        };
        //Testing at current floor
        let state4 = ElevatorState {
            behaviour: Moving,
            floor: 1,
            direction: Stop,
            cab_requests: [true, false, false, false].to_vec(),
        };

        let test_direction1 = Direction::Up;
        let test_direction2 = Direction::Up;
        let test_direction3 = Direction::Down;
        let test_direction4 = Direction::Up;
        
        // Act
        fsm.test_set_state(state1);
        let direction1 = fsm.test_has_orders_in_direction(test_direction1);
        fsm.test_set_state(state2);
        let direction2 = fsm.test_has_orders_in_direction(test_direction2);
        fsm.test_set_state(state3);
        let direction3 = fsm.test_has_orders_in_direction(test_direction3);
        fsm.test_set_state(state4);
        let direction4 = fsm.test_has_orders_in_direction(test_direction4);

        // Assert
        assert_eq!(direction1, false);
        assert_eq!(direction2, true);
        assert_eq!(direction3, true);
        assert_eq!(direction4, false);
    }

    #[test]
    fn test_fsm_complete_orders() {
        // Arrange
        let (mut fsm,
            _hw_motor_direction_rx,
            _hw_floor_sensor_tx,
            _hw_door_light_rx,
            _hw_obstruction_tx,
            _hw_stop_button_tx,
            _fsm_hall_requests_tx,
            _fsm_cab_request_tx,
            _fsm_order_complete_rx,
            _fsm_state_rx,
            _terminate_tx) = setup_fsm();

        //Checking for completing of cab buttons (Been tested for all types of directions types)
        let state1 = ElevatorState {
            behaviour: Moving,
            floor: 1,
            direction: Up,
            cab_requests: [false, true, false, false].to_vec(),
        };

        let hall_requests1 = [[false, false].to_vec(),
                              [false, false].to_vec(),
                              [false, false].to_vec(),
                              [false, false].to_vec()
                              ].to_vec();

        //Checking for completing of hall up orders (Tested for all types of direction types)
        let state2 = ElevatorState {
            behaviour: Moving,
            floor: 2,
            direction: Up,
            cab_requests: [false, false, false, false].to_vec(),
        };

        let hall_requests2 = [[false, true].to_vec(),
                              [false, true].to_vec(),
                              [false, true].to_vec(),
                              [false, false].to_vec()
                              ].to_vec();

        //Checking for completing of hall down orders (Tested for all direction types)
        let state3 = ElevatorState {
            behaviour: Moving,
            floor: 1,
            direction: Stop,
            cab_requests: [false, false, false, false].to_vec(),
        };

        let hall_requests3 = [[false, false].to_vec(),
                              [true, false].to_vec(),
                              [false, false].to_vec(),
                              [false, false].to_vec()
                            ].to_vec();

        // Act 
        fsm.test_set_state(state1);
        fsm.test_set_hall_requests(hall_requests1);
        let result1 = fsm.test_complete_orders();

        fsm.test_set_state(state2);
        fsm.test_set_hall_requests(hall_requests2);
        let result2 = fsm.test_complete_orders();

        fsm.test_set_state(state3);
        fsm.test_set_hall_requests(hall_requests3);
        let result3 = fsm.test_complete_orders();

        // Assert
        assert_eq!(result1, true);
        assert_eq!(result2, true);
        assert_eq!(result3, true);
    }

    #[test]
    fn test_open_door() {
        // Arrange
        let (mut fsm,
            _hw_motor_direction_rx,
            _hw_floor_sensor_tx,
            _hw_door_light_rx,
            _hw_obstruction_tx,
            _hw_stop_button_tx,
            _fsm_hall_requests_tx,
            _fsm_cab_request_tx,
            _fsm_order_complete_rx,
            _fsm_state_rx,
            _terminate_tx) = setup_fsm();

        // Act
        fsm.test_open_door();
        let door_light = _hw_door_light_rx.recv();
        let motor_direction = _hw_motor_direction_rx.recv();
        let status = fsm.test_get_state().behaviour.clone();

        // Assert
        assert_eq!(door_light, Ok(true));
        assert_eq!(motor_direction, Ok(DIRN_STOP));
        assert_eq!(status, DoorOpen);
    }

    #[test]
    fn test_close_door() {
        // Arrange
        let (mut fsm,
            _hw_motor_direction_rx,
            _hw_floor_sensor_tx,
            _hw_door_light_rx,
            _hw_obstruction_tx,
            _hw_stop_button_tx,
            _fsm_hall_requests_tx,
            _fsm_cab_request_tx,
            _fsm_order_complete_rx,
            _fsm_state_rx,
            _terminate_tx) = setup_fsm();

        //Checking for completing of cab buttons (Been tested for all types of directions types)
        let state1 = ElevatorState {
            behaviour: Idle,
            floor: 1,
            direction: Up,
            cab_requests: [false, true, false, false].to_vec(),
        };

        let state1_1 = ElevatorState {
            behaviour: Idle,
            floor: 1,
            direction: Stop,
            cab_requests: [false, false, false, false].to_vec(),
        };

        // Act
        fsm.test_set_state(state1.clone());
        fsm.test_close_door();
        let door_light_1 = _hw_door_light_rx.recv();
        let motor_direction_1 = _hw_motor_direction_rx.recv();
        let fsm_state_rx_1 = _fsm_state_rx.recv();
        let state_1 = fsm.test_get_state().behaviour.clone();

        // Assert
        assert_eq!(door_light_1, Ok(false));
        assert_eq!(motor_direction_1, Ok(Idle as u8));
        assert_eq!(fsm_state_rx_1, Ok(state1_1));
        assert_eq!(state_1, Idle);
    }
}
