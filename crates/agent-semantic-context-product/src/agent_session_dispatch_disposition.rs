pub const AGENT_SESSION_DISPATCH_DISPOSITION_SCHEMA_ID: &str =
    "agent.semantic-protocols.agent-session-dispatch-disposition";
pub const AGENT_SESSION_DISPATCH_DISPOSITION_SCHEMA_VERSION: &str = "1";

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RegistryBindingDisposition {
    Exact,
    Absent,
    Stale,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum HostPathObservation {
    Present,
    Absent,
    Unobserved,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum HostTaskObservation {
    Absent,
    Idle,
    Running,
    Completed,
    Errored,
    Unobserved,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentBootstrapState {
    NotStarted,
    Pending,
    Registered,
    TypedFailure,
    EmptyPayload,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentSessionDispatchAction {
    FollowupTask,
    RegisterExisting,
    SpawnAgent,
    ObserveHostPath,
    AwaitBootstrapTerminal,
    RepairExisting,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionDispatchDisposition {
    pub schema_id: String,
    pub schema_version: String,
    pub registry_binding: RegistryBindingDisposition,
    pub host_path_observation: HostPathObservation,
    pub host_task_observation: HostTaskObservation,
    pub bootstrap_state: AgentBootstrapState,
    pub required_action: AgentSessionDispatchAction,
    pub reason_kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spawn_task_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registration_receipt_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_identity: Option<String>,
}

impl AgentSessionDispatchDisposition {
    #[allow(clippy::too_many_arguments)]
    pub fn resolve(
        registry_binding: RegistryBindingDisposition,
        host_path_observation: HostPathObservation,
        host_task_observation: HostTaskObservation,
        bootstrap_state: AgentBootstrapState,
        agent_path: Option<String>,
        spawn_task_id: Option<String>,
        registration_receipt_digest: Option<String>,
        terminal_identity: Option<String>,
    ) -> Result<Self, String> {
        let (required_action, reason_kind) = match (
            registry_binding,
            host_path_observation,
            host_task_observation,
            bootstrap_state,
        ) {
            (
                _,
                _,
                HostTaskObservation::Completed | HostTaskObservation::Errored,
                AgentBootstrapState::EmptyPayload,
            ) => (
                AgentSessionDispatchAction::RepairExisting,
                "agent-bootstrap-terminal-missing",
            ),
            (
                RegistryBindingDisposition::Exact,
                HostPathObservation::Present,
                _,
                AgentBootstrapState::Registered,
            ) => (
                AgentSessionDispatchAction::FollowupTask,
                "agent-session-binding-current",
            ),
            (_, _, _, AgentBootstrapState::Pending | AgentBootstrapState::EmptyPayload) => (
                AgentSessionDispatchAction::AwaitBootstrapTerminal,
                "agent-bootstrap-terminal-pending",
            ),
            (_, _, _, AgentBootstrapState::TypedFailure) => (
                AgentSessionDispatchAction::RepairExisting,
                "agent-bootstrap-typed-failure",
            ),
            (_, HostPathObservation::Unobserved, _, AgentBootstrapState::NotStarted) => (
                AgentSessionDispatchAction::ObserveHostPath,
                "agent-host-path-unobserved",
            ),
            (
                RegistryBindingDisposition::Absent,
                HostPathObservation::Absent,
                _,
                AgentBootstrapState::NotStarted,
            ) => (
                AgentSessionDispatchAction::SpawnAgent,
                "agent-session-and-host-path-absent",
            ),
            (
                RegistryBindingDisposition::Absent | RegistryBindingDisposition::Stale,
                HostPathObservation::Present,
                _,
                AgentBootstrapState::NotStarted,
            ) => (
                AgentSessionDispatchAction::RegisterExisting,
                "agent-host-path-requires-registry-binding",
            ),
            (
                RegistryBindingDisposition::Exact | RegistryBindingDisposition::Stale,
                HostPathObservation::Absent,
                _,
                AgentBootstrapState::NotStarted,
            ) => (
                AgentSessionDispatchAction::RepairExisting,
                "agent-session-binding-host-path-absent",
            ),
            _ => return Err("agent-session-dispatch-disposition-inconsistent".to_owned()),
        };
        let disposition = Self {
            schema_id: AGENT_SESSION_DISPATCH_DISPOSITION_SCHEMA_ID.to_owned(),
            schema_version: AGENT_SESSION_DISPATCH_DISPOSITION_SCHEMA_VERSION.to_owned(),
            registry_binding,
            host_path_observation,
            host_task_observation,
            bootstrap_state,
            required_action,
            reason_kind: reason_kind.to_owned(),
            agent_path,
            spawn_task_id,
            registration_receipt_digest,
            terminal_identity,
        };
        disposition.validate()?;
        Ok(disposition)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != AGENT_SESSION_DISPATCH_DISPOSITION_SCHEMA_ID
            || self.schema_version != AGENT_SESSION_DISPATCH_DISPOSITION_SCHEMA_VERSION
        {
            return Err("agent-session-dispatch-disposition-schema-mismatch".to_owned());
        }
        if matches!(
            self.required_action,
            AgentSessionDispatchAction::FollowupTask
                | AgentSessionDispatchAction::RegisterExisting
                | AgentSessionDispatchAction::RepairExisting
        ) && self.agent_path.as_deref().is_none_or(str::is_empty)
        {
            return Err("agent-session-dispatch-agent-path-missing".to_owned());
        }
        if self.required_action == AgentSessionDispatchAction::FollowupTask
            && self
                .registration_receipt_digest
                .as_deref()
                .is_none_or(|digest| !is_blake3_digest(digest))
        {
            return Err("agent-session-dispatch-registration-receipt-missing".to_owned());
        }
        if matches!(
            self.bootstrap_state,
            AgentBootstrapState::Pending | AgentBootstrapState::EmptyPayload
        ) && self.spawn_task_id.as_deref().is_none_or(str::is_empty)
        {
            return Err("agent-bootstrap-spawn-task-identity-missing".to_owned());
        }
        if self.bootstrap_state == AgentBootstrapState::EmptyPayload
            && matches!(
                self.host_task_observation,
                HostTaskObservation::Completed | HostTaskObservation::Errored
            )
            && (self.required_action != AgentSessionDispatchAction::RepairExisting
                || self.terminal_identity.as_deref().is_none_or(str::is_empty))
        {
            return Err("agent-bootstrap-terminal-missing-receipt-invalid".to_owned());
        }
        if self.required_action == AgentSessionDispatchAction::SpawnAgent
            && (self.registry_binding != RegistryBindingDisposition::Absent
                || self.host_path_observation != HostPathObservation::Absent
                || self.bootstrap_state != AgentBootstrapState::NotStarted)
        {
            return Err("agent-session-dispatch-spawn-not-authorized".to_owned());
        }
        Ok(())
    }
}

fn is_blake3_digest(value: &str) -> bool {
    value.strip_prefix("blake3-256:").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
