// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::publish_connection_completion;
use crate::runtime_server_observability::{RuntimeServerEvent, RuntimeServerEventPublisher};
use crate::runtime_server_runtime::RuntimeServerConnectionSupervisor;

fn completion_lease() -> crate::runtime_server_runtime::RuntimeServerConnectionLease {
    RuntimeServerConnectionSupervisor::new("connection-completion-test", 1)
        .try_admit()
        .expect("one test connection")
}

#[test]
fn successful_connection_completion_is_not_a_query_terminal() {
    let (sender, mut receiver) = tokio::sync::mpsc::channel(2);
    let publisher = RuntimeServerEventPublisher::new(sender, 2);
    publish_connection_completion(Some(&publisher), Ok((completion_lease(), Ok(false))));
    assert!(
        matches!(
            receiver.try_recv(),
            Err(tokio::sync::mpsc::error::TryRecvError::Empty)
        ),
        "a successful connection lease is lifecycle bookkeeping, not a query terminal"
    );
}

#[test]
fn failed_connection_completion_publishes_one_failure_event() {
    let (sender, mut receiver) = tokio::sync::mpsc::channel(2);
    let publisher = RuntimeServerEventPublisher::new(sender, 2);
    publish_connection_completion(
        Some(&publisher),
        Ok((completion_lease(), Err("typed-terminal-failure".to_owned()))),
    );
    assert_eq!(
        receiver.try_recv(),
        Ok(RuntimeServerEvent::ConnectionRejected(
            "typed-terminal-failure".to_owned()
        ))
    );
    assert!(matches!(
        receiver.try_recv(),
        Err(tokio::sync::mpsc::error::TryRecvError::Empty)
    ));
}
