// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Connection completion and process-signal lifecycle support.

pub(super) fn publish_connection_completion(
    events: Option<&crate::runtime_server_observability::RuntimeServerEventPublisher>,
    completed: Result<
        (
            crate::runtime_server_runtime::RuntimeServerConnectionLease,
            Result<bool, String>,
        ),
        tokio::task::JoinError,
    >,
) {
    match completed {
        Ok((_lease, Ok(_))) => {}
        Ok((_lease, Err(error))) => crate::runtime_server_observability::publish_event(
            events,
            crate::runtime_server_observability::RuntimeServerEvent::ConnectionRejected(error),
        ),
        Err(error) => crate::runtime_server_observability::publish_event(
            events,
            crate::runtime_server_observability::RuntimeServerEvent::ConnectionTaskFailed(
                error.to_string(),
            ),
        ),
    }
}

#[cfg(unix)]
pub(crate) async fn runtime_server_shutdown_signal() -> Result<(), String> {
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .map_err(|error| {
        format!("failed to install Runtime Server SIGTERM handler: {error}")
    })?;
    tokio::select! {
        result = tokio::signal::ctrl_c() => {
            result.map_err(|error| format!("failed to await Runtime Server Ctrl-C signal: {error}"))
        }
        _ = terminate.recv() => Ok(()),
    }
}

#[cfg(not(unix))]
pub(crate) async fn runtime_server_shutdown_signal() -> Result<(), String> {
    tokio::signal::ctrl_c()
        .await
        .map_err(|error| format!("failed to await Runtime Server Ctrl-C signal: {error}"))
}
