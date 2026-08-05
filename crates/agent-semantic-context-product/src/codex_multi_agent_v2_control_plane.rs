use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::agent_session_lifecycle::{AgentSessionLifecycleProjection, WorkspaceServerProjection};

pub const CODEX_MULTI_AGENT_V2_CONTROL_PLANE_SCHEMA_ID: &str =
    "agent.semantic-protocols.codex-multi-agent-v2-control-plane-projection";
pub const CODEX_MULTI_AGENT_V2_CONTROL_PLANE_SCHEMA_VERSION: &str = "1";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CodexTurnPhase {
    Unobserved,
    Idle,
    Queued,
    Running,
    Waiting,
    Completed,
    Failed,
    Interrupted,
    Quarantined,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CodexDelegationPhase {
    Unobserved,
    IntentDurable,
    Dispatched,
    Delivered,
    Quarantined,
    Rejected,
    Completed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CodexControlPlaneFreshness {
    Unobserved,
    Current,
    Stale,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodexControlPlanePublicationAdmission {
    Advance,
    Idempotent,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexControlPlaneMaterialization {
    pub generation: u64,
    pub source_digest: Option<String>,
    pub evidence_refs: Vec<String>,
    pub freshness: CodexControlPlaneFreshness,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexRootTaskProjection {
    pub session_id: String,
    pub evidence_ref: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexAgentNodeProjection {
    pub root_session_id: String,
    pub parent_session_id: Option<String>,
    pub resident_name: String,
    pub role: String,
    pub configured_agent_type: Option<String>,
    pub lifecycle: AgentSessionLifecycleProjection,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexTurnNodeProjection {
    pub turn_id: String,
    pub session_id: String,
    pub generation: Option<u64>,
    pub phase: CodexTurnPhase,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexDelegationEdgeProjection {
    pub parent_session_id: String,
    pub parent_turn_id: Option<String>,
    pub child_session_id: String,
    pub child_generation: u64,
    pub phase: CodexDelegationPhase,
    pub delivered_receipt_ref: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexMultiAgentV2ControlPlaneProjection {
    pub schema_id: String,
    pub schema_version: String,
    pub materialization: CodexControlPlaneMaterialization,
    pub workspace_server: WorkspaceServerProjection,
    pub root_session_id: String,
    pub root_task: CodexRootTaskProjection,
    pub agents: Vec<CodexAgentNodeProjection>,
    pub turns: Vec<CodexTurnNodeProjection>,
    pub delegations: Vec<CodexDelegationEdgeProjection>,
}

impl CodexMultiAgentV2ControlPlaneProjection {
    pub fn new(
        materialization: CodexControlPlaneMaterialization,
        workspace_server: WorkspaceServerProjection,
        root_session_id: String,
        agents: Vec<CodexAgentNodeProjection>,
        turns: Vec<CodexTurnNodeProjection>,
        delegations: Vec<CodexDelegationEdgeProjection>,
    ) -> Result<Self, String> {
        let root_task = CodexRootTaskProjection {
            session_id: root_session_id.clone(),
            evidence_ref: materialization.evidence_refs.first().cloned(),
        };
        let projection = Self {
            schema_id: CODEX_MULTI_AGENT_V2_CONTROL_PLANE_SCHEMA_ID.to_owned(),
            schema_version: CODEX_MULTI_AGENT_V2_CONTROL_PLANE_SCHEMA_VERSION.to_owned(),
            materialization,
            workspace_server,
            root_session_id,
            root_task,
            agents,
            turns,
            delegations,
        };
        projection.validate()?;
        Ok(projection)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self
            .materialization
            .evidence_refs
            .iter()
            .any(|reference| reference.is_empty())
        {
            return Err("Codex control-plane evidence reference must not be empty".to_owned());
        }
        if self.materialization.freshness != CodexControlPlaneFreshness::Unobserved
            && self
                .materialization
                .source_digest
                .as_deref()
                .is_none_or(str::is_empty)
        {
            return Err("observed Codex control plane requires a source digest".to_owned());
        }
        if self.materialization.freshness == CodexControlPlaneFreshness::Current
            && self.materialization.evidence_refs.is_empty()
        {
            return Err("current Codex control plane requires indexed evidence".to_owned());
        }
        if self.workspace_server.workspace_identity.is_empty() {
            return Err("workspace identity must not be empty".to_owned());
        }
        if self.root_session_id.is_empty() {
            return Err("Codex root session id must not be empty".to_owned());
        }
        if self.root_task.session_id != self.root_session_id {
            return Err("Codex root-task identity does not match rootSessionId".to_owned());
        }
        if self.materialization.freshness == CodexControlPlaneFreshness::Current
            && self
                .root_task
                .evidence_ref
                .as_deref()
                .is_none_or(str::is_empty)
        {
            return Err("current Codex root task requires indexed evidence".to_owned());
        }
        if let Some(root_evidence_ref) = self.root_task.evidence_ref.as_ref()
            && !self
                .materialization
                .evidence_refs
                .contains(root_evidence_ref)
        {
            return Err(
                "Codex root-task evidence must be indexed by the materialization".to_owned(),
            );
        }

        let mut agents = HashMap::with_capacity(self.agents.len());
        for agent in &self.agents {
            if agent.root_session_id != self.root_session_id {
                return Err(format!(
                    "agent root session {} does not match control-plane root {}",
                    agent.root_session_id, self.root_session_id
                ));
            }
            if agent.resident_name.is_empty() || agent.role.is_empty() {
                return Err("Codex agent resident name and role must not be empty".to_owned());
            }
            if agent.lifecycle.workspace_server != self.workspace_server {
                return Err("agent lifecycle observes a different Workspace Server".to_owned());
            }
            let session_id = agent
                .lifecycle
                .session
                .session_id
                .as_deref()
                .ok_or_else(|| "Codex agent node requires an observed session id".to_owned())?;
            let generation = agent
                .lifecycle
                .session
                .generation
                .ok_or_else(|| "Codex agent node requires an observed generation".to_owned())?;
            if agents.insert(session_id.to_owned(), generation).is_some() {
                return Err(format!("duplicate Codex agent session id `{session_id}`"));
            }
            if session_id == self.root_session_id {
                return Err("Codex root task must not be encoded as an agent node".to_owned());
            }
        }
        for agent in &self.agents {
            let session_id = agent
                .lifecycle
                .session
                .session_id
                .as_deref()
                .expect("validated observed session id");
            match agent.parent_session_id.as_deref() {
                None => {
                    return Err(format!(
                        "Codex agent `{session_id}` requires a parent task or agent"
                    ));
                }
                Some(parent) if parent == session_id => {
                    return Err(format!("Codex agent `{session_id}` cannot parent itself"));
                }
                Some(parent) if parent != self.root_session_id && !agents.contains_key(parent) => {
                    return Err(format!(
                        "Codex agent `{session_id}` references missing parent `{parent}`"
                    ));
                }
                _ => {}
            }
        }

        let mut turns = HashMap::with_capacity(self.turns.len());
        for turn in &self.turns {
            if turn.turn_id.is_empty() || turn.session_id.is_empty() {
                return Err("Codex turn identity must not be empty".to_owned());
            }
            if turns
                .insert(turn.turn_id.as_str(), turn.session_id.as_str())
                .is_some()
            {
                return Err(format!("duplicate Codex turn id `{}`", turn.turn_id));
            }
            let agent_generation = agents.get(&turn.session_id).ok_or_else(|| {
                format!(
                    "Codex turn `{}` references missing agent `{}`",
                    turn.turn_id, turn.session_id
                )
            })?;
            if turn.phase != CodexTurnPhase::Unobserved && turn.generation.is_none() {
                return Err(format!(
                    "observed Codex turn `{}` requires a generation",
                    turn.turn_id
                ));
            }
            if turn
                .generation
                .is_some_and(|generation| generation != *agent_generation)
            {
                return Err(format!(
                    "Codex turn `{}` generation does not match agent `{}`",
                    turn.turn_id, turn.session_id
                ));
            }
        }

        let mut delegation_keys = HashSet::with_capacity(self.delegations.len());
        for edge in &self.delegations {
            if edge.parent_session_id == edge.child_session_id {
                return Err("Codex delegation cannot target its parent session".to_owned());
            }
            if edge.parent_session_id != self.root_session_id
                && !agents.contains_key(&edge.parent_session_id)
            {
                return Err(format!(
                    "Codex delegation references missing parent `{}`",
                    edge.parent_session_id
                ));
            }
            let child_generation = agents.get(&edge.child_session_id).ok_or_else(|| {
                format!(
                    "Codex delegation references missing child `{}`",
                    edge.child_session_id
                )
            })?;
            if edge.child_generation != *child_generation {
                return Err(format!(
                    "Codex delegation child generation is stale for `{}`",
                    edge.child_session_id
                ));
            }
            if let Some(parent_turn_id) = edge.parent_turn_id.as_deref() {
                let turn_owner = turns.get(parent_turn_id).ok_or_else(|| {
                    format!("Codex delegation references missing turn `{parent_turn_id}`")
                })?;
                if *turn_owner != edge.parent_session_id {
                    return Err(format!(
                        "Codex delegation turn `{parent_turn_id}` belongs to another agent"
                    ));
                }
            }
            if matches!(
                edge.phase,
                CodexDelegationPhase::Delivered | CodexDelegationPhase::Completed
            ) && edge
                .delivered_receipt_ref
                .as_deref()
                .is_none_or(str::is_empty)
            {
                return Err(
                    "delivered Codex delegation requires an indexed receipt reference".to_owned(),
                );
            }
            let key = (
                edge.parent_session_id.as_str(),
                edge.child_session_id.as_str(),
                edge.child_generation,
            );
            if !delegation_keys.insert(key) {
                return Err("duplicate Codex delegation edge".to_owned());
            }
        }
        Ok(())
    }

    pub fn with_workspace_server(mut self, server: WorkspaceServerProjection) -> Self {
        if self.workspace_server != server {
            self.materialization.freshness = CodexControlPlaneFreshness::Stale;
        }
        self.workspace_server = server.clone();
        for agent in &mut self.agents {
            agent.lifecycle.workspace_server = server.clone();
        }
        self
    }

    pub fn downstream_dispatch_authorized(&self, session_id: &str) -> bool {
        self.materialization.freshness == CodexControlPlaneFreshness::Current
            && self.agents.iter().any(|agent| {
                agent.lifecycle.session.session_id.as_deref() == Some(session_id)
                    && agent.lifecycle.durable_dispatch_authorized
            })
    }

    pub fn publication_admission(
        &self,
        candidate: &Self,
    ) -> Result<CodexControlPlanePublicationAdmission, String> {
        self.validate()?;
        candidate.validate()?;
        if self.workspace_server.workspace_identity != candidate.workspace_server.workspace_identity
            || self.root_session_id != candidate.root_session_id
        {
            return Err("Codex control-plane publication authority does not match".to_owned());
        }
        if candidate.materialization.generation < self.materialization.generation {
            return Err(format!(
                "stale Codex control-plane generation rejected: current={} candidate={}",
                self.materialization.generation, candidate.materialization.generation
            ));
        }
        if candidate.materialization.generation == self.materialization.generation {
            if self == candidate {
                return Ok(CodexControlPlanePublicationAdmission::Idempotent);
            }
            return Err(format!(
                "conflicting Codex control-plane materialization for generation {}",
                candidate.materialization.generation
            ));
        }
        Ok(CodexControlPlanePublicationAdmission::Advance)
    }
}
