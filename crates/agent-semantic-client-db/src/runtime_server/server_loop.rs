// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Runtime Server connection admission, lifecycle refresh, and bounded drain loop.

use std::sync::Arc;

use tokio::sync::watch;
use tokio::task::JoinSet;

use super::core::{
    AspPythonGraphsStatusHandle, CONNECTION_DRAIN_BOUNDARY, DURABILITY_DRAIN_BOUNDARY,
    RuntimeServer, RuntimeServerEvent, RuntimeServerExit, publish_connection_completion,
};
use crate::runtime_server_agent_session_status::AgentSessionStatusHandle;
use crate::runtime_server_observability::publish_event;

impl RuntimeServer {
    pub(super) async fn serve_inner(self) -> Result<RuntimeServerExit, String> {
        let Self {
            artifact_catalog: _artifact_catalog,
            workspace_registry,
            runtime_search_service: _,
            endpoint,
            listener,
            provider_register: _provider_register,
            registry,
            mut workspace_count,
            mut shutdown,
            shutdown_handle: _,
            mut status_memory,
            events,
            generation_admission,
            asp_python_graphs_status,
            agent_session_registry_owner,
            agent_session_status,
            telemetry_sender: _,
            readiness_sender,
            generation_publication,
            durability_tasks,
        } = self;
        let (_lifecycle_state, lifecycle) =
            watch::channel(crate::runtime_server_control::RuntimeServerState::Healthy);
        generation_publication.clear();
        // Socket liveness is Global; generation readiness is workspace-keyed.
        // Durable catalog locators stay lazy. A workspace request restores its
        // published pointer or admits one scope-local rebuild; startup never
        // opens every historical generation segment.
        let entry_counts = registry.workspace_entry_counts();
        let slot_count = entry_counts.slot_count;
        let loaded_entry_count = entry_counts.loaded_entry_count;
        status_memory.publish(
            crate::runtime_server_control::RuntimeServerState::Healthy,
            slot_count
                .max(loaded_entry_count)
                .max(*workspace_count.borrow()),
        )?;
        readiness_sender.send_replace(crate::runtime_server_control::RuntimeServerState::Healthy);
        let mut connections = JoinSet::new();
        let connection_supervisor =
            crate::runtime_server_connection::RuntimeServerConnectionSupervisor::for_current_runtime(
                "runtime-server-ipc",
            );
        let control_replay_guard = Arc::new(tokio::sync::Mutex::new(
            super::control_connection::RuntimeServerControlReplayGuard::default(),
        ));
        let (drain_sender, drain_receiver) = watch::channel(false);
        let mut retirement_sweep = tokio::time::interval(std::time::Duration::from_secs(60));
        retirement_sweep.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        retirement_sweep.tick().await;
        let mut asp_python_graphs_status_changes = asp_python_graphs_status
            .as_ref()
            .map(AspPythonGraphsStatusHandle::subscribe);
        let mut agent_session_status_changes = agent_session_status
            .as_ref()
            .map(AgentSessionStatusHandle::subscribe);
        if let (Some(status), Some(owner)) = (
            agent_session_status.as_ref(),
            agent_session_registry_owner.as_ref(),
        ) {
            status.refresh(owner).await?;
        }
        let exit = loop {
            tokio::select! {
                connection = listener.accept(), if connection_supervisor.has_capacity() => {
                    let (stream, peer) = connection.map_err(|error| {
                        format!("failed to accept runtime server request: {error}")
                    })?;
                    if !peer.ip().is_loopback() {
                        return Err("Runtime Server rejected a non-loopback control peer".to_owned());
                    }
                    let lease = connection_supervisor
                        .try_admit()
                        .expect("capacity guard must admit one control connection");
                    let connection_endpoint = endpoint.clone();
                    let connection_registry = Arc::clone(&registry);
                    let connection_generation_admission = generation_admission.clone();
                    let connection_lifecycle = lifecycle.clone();
                    let connection_asp_python_graphs_status = asp_python_graphs_status.clone();
                    let connection_drain = drain_receiver.clone();
                    let connection_replay_guard = Arc::clone(&control_replay_guard);
                    connections.spawn(async move {
                        let result = super::control_connection::serve_connection(
                            stream,
                            connection_endpoint,
                            connection_registry,
                            connection_generation_admission,
                            connection_lifecycle,
                            connection_asp_python_graphs_status,
                            connection_drain,
                            connection_replay_guard,
                        )
                        .await;
                        (lease, result)
                    });
                }
                completed = connections.join_next(), if !connections.is_empty() => {
                    match completed {
                        Some(Ok((_lease, Ok(true)))) => {
                            let entry_counts = registry.workspace_entry_counts();
                            let slot_count = entry_counts.slot_count;
                            let loaded_entry_count = entry_counts.loaded_entry_count;
                            status_memory.publish(
                                crate::runtime_server_control::RuntimeServerState::Draining,
                                slot_count
                                    .max(loaded_entry_count)
                                    .max(*workspace_count.borrow()),
                            )?;
                            let _ = drain_sender.send(true);
                            break RuntimeServerExit::RestartRequested;
                        }
                        Some(Ok((_lease, Ok(false)))) => {}
                        Some(Ok((_lease, Err(error)))) => {
                            publish_event(
                                events.as_ref(),
                                RuntimeServerEvent::ConnectionRejected(error),
                            );
                        }
                        Some(Err(error)) => {
                            publish_event(
                                events.as_ref(),
                                RuntimeServerEvent::ConnectionTaskFailed(error.to_string()),
                            );
                        }
                        None => break RuntimeServerExit::ListenerClosed,
                    }
                }
                changed = workspace_count.changed() => {
                    if changed.is_err() {
                        break RuntimeServerExit::ListenerClosed;
                    }
                    let entry_counts = registry.workspace_entry_counts();
                    let slot_count = entry_counts.slot_count;
                    let loaded_entry_count = entry_counts.loaded_entry_count;
                    status_memory.publish(
                        crate::runtime_server_control::RuntimeServerState::Healthy,
                        slot_count
                            .max(loaded_entry_count)
                            .max(*workspace_count.borrow_and_update()),
                    )?;
                }
                _ = retirement_sweep.tick() => {
                    if let (Some(status), Some(owner)) =
                        (agent_session_status.as_ref(), agent_session_registry_owner.as_ref())
                    {
                        status.refresh(owner).await?;
                    }
                    let retirement_receipts = workspace_registry.retire_inactive().await?;
                    for receipt in retirement_receipts {
                        eprintln!(
                            "[runtime-server-workspace-retirement] schemaId={} schemaVersion={} workspaceIdentity={} reason={:?} checkpointCompleted={} writerLaneDrained={} endpointRetired={}",
                            receipt.schema_id,
                            receipt.schema_version,
                            receipt.workspace_identity,
                            receipt.reason,
                            receipt.checkpoint_completed,
                            receipt.writer_lane_drained,
                            receipt.endpoint_retired,
                        );
                    }
                }
                changed = async {
                    match asp_python_graphs_status_changes.as_mut() {
                        Some(changes) => Some(changes.changed().await),
                        None => std::future::pending().await,
                    }
                }, if asp_python_graphs_status_changes.is_some() => {
                    changed
                        .expect("asp-python-graphs status branch requires a receiver")
                        .map_err(|_| "asp-python-graphs status owner closed".to_owned())?;
                    let entry_counts = registry.workspace_entry_counts();
                    let slot_count = entry_counts.slot_count;
                    let loaded_entry_count = entry_counts.loaded_entry_count;
                    status_memory.publish(
                        crate::runtime_server_control::RuntimeServerState::Healthy,
                        slot_count
                            .max(loaded_entry_count)
                            .max(*workspace_count.borrow()),
                    )?;
                    if let Some(status) = agent_session_status.as_ref() {
                        status.mark_current_published();
                    }
                }
                changed = async {
                    match agent_session_status_changes.as_mut() {
                        Some(changes) => Some(changes.changed().await),
                        None => std::future::pending().await,
                    }
                }, if agent_session_status_changes.is_some() => {
                    changed
                        .expect("Agent session status branch requires a receiver")
                        .map_err(|_| "Agent session status owner closed".to_owned())?;
                    let entry_counts = registry.workspace_entry_counts();
                    let slot_count = entry_counts.slot_count;
                    let loaded_entry_count = entry_counts.loaded_entry_count;
                    status_memory.publish(
                        crate::runtime_server_control::RuntimeServerState::Healthy,
                        slot_count
                            .max(loaded_entry_count)
                            .max(*workspace_count.borrow()),
                    )?;
                    if let Some(status) = agent_session_status.as_ref() {
                        status.mark_current_published();
                    }
                }
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow_and_update() {
                        let entry_counts = registry.workspace_entry_counts();
                        let slot_count = entry_counts.slot_count;
                        let loaded_entry_count = entry_counts.loaded_entry_count;
                        status_memory.publish(
                            crate::runtime_server_control::RuntimeServerState::Draining,
                            slot_count
                                .max(loaded_entry_count)
                                .max(*workspace_count.borrow()),
                        )?;
                        let _ = drain_sender.send(true);
                        break RuntimeServerExit::ShutdownRequested;
                    }
                }
            }
        };
        let _ = drain_sender.send(true);
        drop(listener);
        if tokio::time::timeout(CONNECTION_DRAIN_BOUNDARY, async {
            while let Some(completed) = connections.join_next().await {
                publish_connection_completion(events.as_ref(), completed);
            }
        })
        .await
        .is_err()
        {
            publish_event(
                events.as_ref(),
                RuntimeServerEvent::ConnectionTaskFailed(format!(
                    "Runtime Server connection drain exceeded {}ms; aborting remaining connection tasks",
                    CONNECTION_DRAIN_BOUNDARY.as_millis()
                )),
            );
            connections.abort_all();
            while let Some(completed) = connections.join_next().await {
                if !matches!(&completed, Err(error) if error.is_cancelled()) {
                    publish_connection_completion(events.as_ref(), completed);
                }
            }
        }
        let mut durability_tasks = durability_tasks.lock().await;
        if tokio::time::timeout(DURABILITY_DRAIN_BOUNDARY, async {
            while durability_tasks.join_next().await.is_some() {}
        })
        .await
        .is_err()
        {
            durability_tasks.abort_all();
            while durability_tasks.join_next().await.is_some() {}
        }
        Ok(exit)
    }
}

#[cfg(test)]
#[path = "../../tests/unit/runtime_server_connection_completion.rs"]
mod connection_completion_tests;
