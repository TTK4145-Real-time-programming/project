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
mod tests {
    use std::thread::spawn;
    use crate::ElevatorFSM;
    use crate::ElevatorState;
    use crate::config::ElevatorConfig;
    use crate::shared::Behaviour::{Idle, Moving, DoorOpen};
    use crate::shared::Direction::{Up, Down, Stop};
    use crossbeam_channel::unbounded;

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
            fsm_terminate_rx
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
        // Simulate the elevator hitting floor 0 after creation
        hw_floor_sensor_tx.send(0).unwrap();

        // Assert
        match fsm_state_rx.recv_timeout(std::time::Duration::from_secs(3)) {
            Ok(state) => {
                assert_eq!(state.behaviour, Idle);
                assert_eq!(state.direction, Stop);
                assert_eq!(state.floor, 0);
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

        let state1 = ElevatorState {
            behaviour: Moving,
            floor: 0,
            direction: Stop,
            cab_requests: vec![false; 4],
        };

        let state2 = ElevatorState {
            behaviour: Moving,
            floor: 1,
            direction: Stop,
            cab_requests: vec![false; 4],
        };

        // Act
        fsm.test_set_state(state1);
        fsm.test_set_state(state2);
        let direction1 = fsm.test_choose_direction();
        let direction2 = fsm.test_choose_direction();

        // Assert
        assert_eq!(direction1, Up);
        assert_eq!(direction2, Down);

    }
}
