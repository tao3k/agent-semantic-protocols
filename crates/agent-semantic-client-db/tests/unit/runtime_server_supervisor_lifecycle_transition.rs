use super::runtime_server_status_requires_drain;
use crate::runtime_server_control::RuntimeServerState;

#[test]
fn a_published_draining_state_never_emits_a_second_drain_request() {
    assert!(!runtime_server_status_requires_drain(
        &RuntimeServerState::Draining
    ));
    assert!(runtime_server_status_requires_drain(
        &RuntimeServerState::Starting
    ));
    assert!(runtime_server_status_requires_drain(
        &RuntimeServerState::Healthy
    ));
}
