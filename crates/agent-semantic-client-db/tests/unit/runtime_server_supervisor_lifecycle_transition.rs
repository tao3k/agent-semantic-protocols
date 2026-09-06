// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

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
