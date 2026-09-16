use super::{
    AgentBootstrapState, AgentSessionDispatchAction, AgentSessionDispatchDisposition,
    HostPathObservation, HostTaskObservation, RegistryBindingDisposition,
};

fn resolve(
    registry: RegistryBindingDisposition,
    path: HostPathObservation,
    task: HostTaskObservation,
    bootstrap: AgentBootstrapState,
) -> AgentSessionDispatchDisposition {
    AgentSessionDispatchDisposition::resolve(
        registry,
        path,
        task,
        bootstrap,
        (path == HostPathObservation::Present).then(|| "/root/asp_explorer".to_owned()),
        matches!(
            bootstrap,
            AgentBootstrapState::Pending | AgentBootstrapState::EmptyPayload
        )
        .then(|| "spawn-task-1".to_owned()),
        (registry == RegistryBindingDisposition::Exact)
            .then(|| format!("blake3-256:{}", "0".repeat(64))),
        (matches!(
            task,
            HostTaskObservation::Completed | HostTaskObservation::Errored
        ) && bootstrap == AgentBootstrapState::EmptyPayload)
            .then(|| "terminal-missing-1".to_owned()),
    )
    .expect("valid disposition")
}

#[test]
fn exact_registry_binding_prioritizes_followup() {
    assert_eq!(
        resolve(
            RegistryBindingDisposition::Exact,
            HostPathObservation::Present,
            HostTaskObservation::Idle,
            AgentBootstrapState::Registered
        )
        .required_action,
        AgentSessionDispatchAction::FollowupTask
    );
}

#[test]
fn running_spawn_with_empty_payload_remains_pending() {
    assert_eq!(
        resolve(
            RegistryBindingDisposition::Absent,
            HostPathObservation::Present,
            HostTaskObservation::Running,
            AgentBootstrapState::EmptyPayload
        )
        .required_action,
        AgentSessionDispatchAction::AwaitBootstrapTerminal
    );
}

#[test]
fn completed_spawn_with_empty_payload_repairs_same_child() {
    let disposition = resolve(
        RegistryBindingDisposition::Absent,
        HostPathObservation::Present,
        HostTaskObservation::Completed,
        AgentBootstrapState::EmptyPayload,
    );
    assert_eq!(
        disposition.required_action,
        AgentSessionDispatchAction::RepairExisting
    );
    assert_ne!(
        disposition.required_action,
        AgentSessionDispatchAction::SpawnAgent
    );
}

#[test]
fn unobserved_host_path_selects_low_priority_observation() {
    assert_eq!(
        resolve(
            RegistryBindingDisposition::Absent,
            HostPathObservation::Unobserved,
            HostTaskObservation::Unobserved,
            AgentBootstrapState::NotStarted
        )
        .required_action,
        AgentSessionDispatchAction::ObserveHostPath
    );
}

#[test]
fn present_unregistered_path_is_reused_not_respawned() {
    assert_eq!(
        resolve(
            RegistryBindingDisposition::Absent,
            HostPathObservation::Present,
            HostTaskObservation::Idle,
            AgentBootstrapState::NotStarted
        )
        .required_action,
        AgentSessionDispatchAction::RegisterExisting
    );
}
// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later
