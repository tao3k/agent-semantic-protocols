use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use agent_semantic_context_product::agent_session_delegation_admission::{
    AgentSessionDelegationAdmissionInput, AgentSessionDelegationAdmissionReceipt,
    AgentSessionDelegationCapability, AgentSessionDelegationDecision,
};
use serde::{Deserialize, Serialize};
use turso::transaction::TransactionBehavior;

use super::connect_turso_client_db;

const CONTROL_PLANE_TABLE: &str = "asp_session_control_plane";
const AGENT_TABLE: &str = "asp_session_control_plane_agent";
const DELEGATION_TABLE: &str = "asp_session_control_plane_delegation";
const EVENT_TABLE: &str = "asp_session_control_plane_event";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionControlPlaneAgentRegistration {
    pub project_id: String,
    pub root_session_id: String,
    pub session_id: String,
    pub parent_session_id: Option<String>,
    pub resident_name: String,
    pub capability: AgentSessionDelegationCapability,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionControlPlaneDelegationProposal {
    pub event_id: String,
    pub project_id: String,
    pub root_session_id: String,
    pub current_session_id: String,
    pub proposed_child_session_id: String,
    pub proposed_child_resident_name: String,
    pub proposed_child_capability: AgentSessionDelegationCapability,
    pub expected_generation: u64,
    pub evidence_refs: Vec<String>,
    pub observed_at_ms: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionControlPlaneTransactionReceipt {
    pub admission: AgentSessionDelegationAdmissionReceipt,
    pub replayed: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionControlPlaneSnapshot {
    pub generation: u64,
    pub state_digest: String,
    pub agent_count: u64,
    pub delegation_count: u64,
    pub event_count: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionControlPlaneRuntimeMetricsSnapshot {
    pub resident_lane_count: u64,
    pub durable_admission_transactions: u64,
    pub resident_receipt_replays: u64,
    pub queue_capacity: u64,
    pub queue_depth: u64,
    pub queue_high_watermark: u64,
    pub enqueued_transitions: u64,
    pub completed_transitions: u64,
    pub drained_transitions: u64,
    pub cancelled_transitions: u64,
    pub actor_starts: u64,
    pub actor_stops: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct SessionControlPlaneTransactionOwner {
    db_path: PathBuf,
    resident_lanes:
        Arc<tokio::sync::RwLock<BTreeMap<ResidentSessionKey, Arc<ResidentSessionLane>>>>,
    runtime_metrics: Arc<SessionControlPlaneRuntimeMetrics>,
}

#[path = "session_control_plane_runtime.rs"]
mod runtime;
pub use runtime::{SessionControlPlaneRuntime, SessionControlPlaneRuntimeRegistry};

#[derive(Debug, Default)]
struct SessionControlPlaneRuntimeMetrics {
    durable_admission_transactions: AtomicU64,
    resident_receipt_replays: AtomicU64,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ResidentSessionKey {
    project_id: String,
    root_session_id: String,
    current_session_id: String,
}

#[derive(Debug, Default)]
struct ResidentSessionLane {
    writer: tokio::sync::Mutex<()>,
    committed: tokio::sync::RwLock<Option<ResidentCommittedReceipt>>,
}

#[derive(Clone, Debug)]
struct ResidentCommittedReceipt {
    event_id: String,
    admission: AgentSessionDelegationAdmissionReceipt,
}

impl SessionControlPlaneTransactionOwner {
    // Implementation lives in the transaction_owner module.
    async fn resident_committed_receipt(
        &self,
        proposal: &SessionControlPlaneDelegationProposal,
    ) -> Option<SessionControlPlaneTransactionReceipt> {
        let key = ResidentSessionKey {
            project_id: proposal.project_id.clone(),
            root_session_id: proposal.root_session_id.clone(),
            current_session_id: proposal.current_session_id.clone(),
        };
        let lane = self.resident_lanes.read().await.get(&key).cloned()?;
        let admission = resident_replay(&lane, &proposal.event_id).await?;
        self.runtime_metrics
            .resident_receipt_replays
            .fetch_add(1, Ordering::Relaxed);
        Some(SessionControlPlaneTransactionReceipt {
            admission,
            replayed: true,
        })
    }
}

async fn resident_replay(
    lane: &ResidentSessionLane,
    event_id: &str,
) -> Option<AgentSessionDelegationAdmissionReceipt> {
    let committed = lane.committed.read().await;
    committed
        .as_ref()
        .filter(|committed| committed.event_id == event_id)
        .map(|committed| committed.admission.clone())
}

async fn count_rows(
    connection: &turso::Connection,
    table: &str,
    project_id: &str,
    root_session_id: &str,
) -> Result<u64, String> {
    let mut rows = connection
        .query(
            &format!("SELECT COUNT(*) FROM {table} WHERE project_id = ?1 AND root_session_id = ?2"),
            (project_id, root_session_id),
        )
        .await
        .map_err(|error| format!("failed to count session control-plane rows: {error}"))?;
    let row = rows
        .next()
        .await
        .map_err(|error| format!("failed to read session control-plane count: {error}"))?
        .ok_or_else(|| "session control-plane count returned no row".to_owned())?;
    let count = row
        .get::<i64>(0)
        .map_err(|error| format!("failed to decode session control-plane count: {error}"))?;
    u64::try_from(count).map_err(|_| "session control-plane count must be non-negative".to_owned())
}

async fn register_agent_transaction(
    transaction: &turso::transaction::Transaction<'_>,
    registration: &SessionControlPlaneAgentRegistration,
) -> Result<(), String> {
    let connection = &**transaction;
    let mut rows = connection
        .query(
            &format!(
                "SELECT generation, state_digest FROM {CONTROL_PLANE_TABLE} \
                 WHERE project_id = ?1 AND root_session_id = ?2"
            ),
            (
                registration.project_id.as_str(),
                registration.root_session_id.as_str(),
            ),
        )
        .await
        .map_err(|error| format!("failed to query session control-plane root: {error}"))?;

    if rows
        .next()
        .await
        .map_err(|error| format!("failed to read session control-plane root: {error}"))?
        .is_none()
    {
        let state_digest = digest_serializable(&(
            registration.project_id.as_str(),
            registration.root_session_id.as_str(),
            0_u64,
        ))?;
        connection
            .execute(
                &format!(
                    "INSERT INTO {CONTROL_PLANE_TABLE} \
                     (project_id, root_session_id, generation, state_digest) \
                     VALUES (?1, ?2, 0, ?3)"
                ),
                (
                    registration.project_id.as_str(),
                    registration.root_session_id.as_str(),
                    state_digest,
                ),
            )
            .await
            .map_err(|error| format!("failed to create session control-plane root: {error}"))?;
    }

    let capability = capability_text(registration.capability);
    let changed = connection
        .execute(
            &format!(
                "INSERT INTO {AGENT_TABLE} \
                 (project_id, root_session_id, session_id, parent_session_id, resident_name, delegation_capability) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
                 ON CONFLICT(project_id, root_session_id, session_id) DO NOTHING"
            ),
            (
                registration.project_id.as_str(),
                registration.root_session_id.as_str(),
                registration.session_id.as_str(),
                registration.parent_session_id.as_deref(),
                registration.resident_name.as_str(),
                capability,
            ),
        )
        .await
        .map_err(|error| format!("failed to register session control-plane agent: {error}"))?;
    if changed == 0 {
        let mut rows = connection
            .query(
                &format!(
                    "SELECT parent_session_id, resident_name, delegation_capability \
                     FROM {AGENT_TABLE} WHERE project_id = ?1 AND root_session_id = ?2 \
                     AND session_id = ?3"
                ),
                (
                    registration.project_id.as_str(),
                    registration.root_session_id.as_str(),
                    registration.session_id.as_str(),
                ),
            )
            .await
            .map_err(|error| format!("failed to verify existing control-plane agent: {error}"))?;
        let row = rows
            .next()
            .await
            .map_err(|error| format!("failed to read existing control-plane agent: {error}"))?
            .ok_or_else(|| "control-plane agent conflict returned no row".to_owned())?;
        let parent_session_id = row
            .get::<Option<String>>(0)
            .map_err(|error| format!("failed to decode existing parent session: {error}"))?;
        let resident_name = row
            .get::<String>(1)
            .map_err(|error| format!("failed to decode existing resident name: {error}"))?;
        let delegation_capability = row
            .get::<String>(2)
            .map_err(|error| format!("failed to decode existing capability: {error}"))?;
        if parent_session_id != registration.parent_session_id
            || resident_name != registration.resident_name
            || delegation_capability != capability
        {
            return Err(format!(
                "session control-plane agent identity drift: sessionId={}",
                registration.session_id
            ));
        }
    }
    Ok(())
}

async fn admit_delegation_transaction(
    transaction: &turso::transaction::Transaction<'_>,
    proposal: &SessionControlPlaneDelegationProposal,
) -> Result<SessionControlPlaneTransactionReceipt, String> {
    let connection = &**transaction;
    if let Some(admission) = committed_receipt(connection, proposal).await? {
        return Ok(SessionControlPlaneTransactionReceipt {
            admission,
            replayed: true,
        });
    }

    let (generation, pre_state_digest) = control_plane_state(connection, proposal).await?;
    if generation != proposal.expected_generation {
        return Err(format!(
            "session control-plane generation is stale: expected={} actual={generation}",
            proposal.expected_generation
        ));
    }
    let current_capability = current_agent_capability(connection, proposal).await?;

    let admission = match current_capability {
        AgentSessionDelegationCapability::FocusedLeaf => {
            AgentSessionDelegationAdmissionReceipt::focused_leaf_denied(
                proposal.project_id.clone(),
                proposal.root_session_id.clone(),
                proposal.current_session_id.clone(),
                proposal.proposed_child_session_id.clone(),
                pre_state_digest.clone(),
                proposal.evidence_refs.clone(),
            )?
        }
        AgentSessionDelegationCapability::Standard => {
            let next_generation = generation
                .checked_add(1)
                .ok_or_else(|| "session control-plane generation overflow".to_owned())?;
            let post_state_digest = digest_serializable(&(
                pre_state_digest.as_str(),
                next_generation,
                proposal.current_session_id.as_str(),
                proposal.proposed_child_session_id.as_str(),
                capability_text(proposal.proposed_child_capability),
            ))?;
            connection
                .execute(
                    &format!(
                        "INSERT INTO {AGENT_TABLE} \
                         (project_id, root_session_id, session_id, parent_session_id, resident_name, delegation_capability) \
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6)"
                    ),
                    (
                        proposal.project_id.as_str(),
                        proposal.root_session_id.as_str(),
                        proposal.proposed_child_session_id.as_str(),
                        proposal.current_session_id.as_str(),
                        proposal.proposed_child_resident_name.as_str(),
                        capability_text(proposal.proposed_child_capability),
                    ),
                )
                .await
                .map_err(|error| format!("failed to insert admitted child agent: {error}"))?;
            connection
                .execute(
                    &format!(
                        "INSERT INTO {DELEGATION_TABLE} \
                         (project_id, root_session_id, parent_session_id, child_session_id, generation) \
                         VALUES (?1, ?2, ?3, ?4, ?5)"
                    ),
                    (
                        proposal.project_id.as_str(),
                        proposal.root_session_id.as_str(),
                        proposal.current_session_id.as_str(),
                        proposal.proposed_child_session_id.as_str(),
                        next_generation as i64,
                    ),
                )
                .await
                .map_err(|error| format!("failed to insert admitted delegation edge: {error}"))?;
            let changed = connection
                .execute(
                    &format!(
                        "UPDATE {CONTROL_PLANE_TABLE} SET generation = ?1, state_digest = ?2 \
                         WHERE project_id = ?3 AND root_session_id = ?4 \
                         AND generation = ?5 AND state_digest = ?6"
                    ),
                    (
                        next_generation as i64,
                        post_state_digest.as_str(),
                        proposal.project_id.as_str(),
                        proposal.root_session_id.as_str(),
                        generation as i64,
                        pre_state_digest.as_str(),
                    ),
                )
                .await
                .map_err(|error| format!("failed to publish delegation generation: {error}"))?;
            if changed != 1 {
                return Err("session control-plane generation compare-and-swap failed".to_owned());
            }
            AgentSessionDelegationAdmissionReceipt::new(AgentSessionDelegationAdmissionInput {
                project_id: proposal.project_id.clone(),
                root_session_id: proposal.root_session_id.clone(),
                current_session_id: proposal.current_session_id.clone(),
                proposed_child_session_id: proposal.proposed_child_session_id.clone(),
                capability: AgentSessionDelegationCapability::Standard,
                decision: AgentSessionDelegationDecision::Accepted,
                reason_kind: None,
                pre_state_digest,
                post_state_digest,
                delegation_generation: Some(next_generation),
                evidence_refs: proposal.evidence_refs.clone(),
            })?
        }
    };

    let receipt_json = serde_json::to_string(&admission)
        .map_err(|error| format!("failed to encode delegation receipt: {error}"))?;
    connection
        .execute(
            &format!(
                "INSERT INTO {EVENT_TABLE} \
                 (project_id, root_session_id, event_id, receipt_json, observed_at_ms) \
                 VALUES (?1, ?2, ?3, ?4, ?5)"
            ),
            (
                proposal.project_id.as_str(),
                proposal.root_session_id.as_str(),
                proposal.event_id.as_str(),
                receipt_json,
                proposal.observed_at_ms,
            ),
        )
        .await
        .map_err(|error| format!("failed to append delegation event: {error}"))?;

    Ok(SessionControlPlaneTransactionReceipt {
        admission,
        replayed: false,
    })
}

async fn committed_receipt(
    connection: &turso::Connection,
    proposal: &SessionControlPlaneDelegationProposal,
) -> Result<Option<AgentSessionDelegationAdmissionReceipt>, String> {
    let mut rows = connection
        .query(
            &format!(
                "SELECT receipt_json FROM {EVENT_TABLE} \
                 WHERE project_id = ?1 AND root_session_id = ?2 AND event_id = ?3"
            ),
            (
                proposal.project_id.as_str(),
                proposal.root_session_id.as_str(),
                proposal.event_id.as_str(),
            ),
        )
        .await
        .map_err(|error| format!("failed to query committed delegation event: {error}"))?;
    let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read committed delegation event: {error}"))?
    else {
        return Ok(None);
    };
    let receipt_json = row
        .get::<String>(0)
        .map_err(|error| format!("failed to decode committed delegation event: {error}"))?;
    let receipt: AgentSessionDelegationAdmissionReceipt = serde_json::from_str(&receipt_json)
        .map_err(|error| format!("failed to parse committed delegation receipt: {error}"))?;
    receipt.validate()?;
    Ok(Some(receipt))
}

async fn control_plane_state(
    connection: &turso::Connection,
    proposal: &SessionControlPlaneDelegationProposal,
) -> Result<(u64, String), String> {
    let mut rows = connection
        .query(
            &format!(
                "SELECT generation, state_digest FROM {CONTROL_PLANE_TABLE} \
                 WHERE project_id = ?1 AND root_session_id = ?2"
            ),
            (
                proposal.project_id.as_str(),
                proposal.root_session_id.as_str(),
            ),
        )
        .await
        .map_err(|error| format!("failed to query session control-plane state: {error}"))?;
    let row = rows
        .next()
        .await
        .map_err(|error| format!("failed to read session control-plane state: {error}"))?
        .ok_or_else(|| "session control-plane root is not registered".to_owned())?;
    let generation = row
        .get::<i64>(0)
        .map_err(|error| format!("failed to decode session control-plane generation: {error}"))?;
    let generation = u64::try_from(generation)
        .map_err(|_| "session control-plane generation must be non-negative".to_owned())?;
    let state_digest = row
        .get::<String>(1)
        .map_err(|error| format!("failed to decode session control-plane digest: {error}"))?;
    Ok((generation, state_digest))
}

async fn current_agent_capability(
    connection: &turso::Connection,
    proposal: &SessionControlPlaneDelegationProposal,
) -> Result<AgentSessionDelegationCapability, String> {
    let mut rows = connection
        .query(
            &format!(
                "SELECT delegation_capability FROM {AGENT_TABLE} \
                 WHERE project_id = ?1 AND root_session_id = ?2 AND session_id = ?3"
            ),
            (
                proposal.project_id.as_str(),
                proposal.root_session_id.as_str(),
                proposal.current_session_id.as_str(),
            ),
        )
        .await
        .map_err(|error| format!("failed to query current agent capability: {error}"))?;
    let row = rows
        .next()
        .await
        .map_err(|error| format!("failed to read current agent capability: {error}"))?
        .ok_or_else(|| "current agent is not registered in the session control plane".to_owned())?;
    let capability = row
        .get::<String>(0)
        .map_err(|error| format!("failed to decode current agent capability: {error}"))?;
    parse_capability(&capability)
}

async fn finish_transaction<T>(
    transaction: turso::transaction::Transaction<'_>,
    result: Result<T, String>,
) -> Result<T, String> {
    match result {
        Ok(value) => {
            transaction.commit().await.map_err(|error| {
                format!("failed to commit session control-plane transaction: {error}")
            })?;
            Ok(value)
        }
        Err(error) => match transaction.rollback().await {
            Ok(()) => Err(error),
            Err(rollback_error) => Err(format!("{error}; rollbackError={rollback_error}")),
        },
    }
}

fn validate_agent_registration(
    registration: &SessionControlPlaneAgentRegistration,
) -> Result<(), String> {
    for (field, value) in [
        ("projectId", registration.project_id.as_str()),
        ("rootSessionId", registration.root_session_id.as_str()),
        ("sessionId", registration.session_id.as_str()),
        ("residentName", registration.resident_name.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(format!("{field} must be non-empty text"));
        }
    }
    Ok(())
}

fn validate_delegation_proposal(
    proposal: &SessionControlPlaneDelegationProposal,
) -> Result<(), String> {
    for (field, value) in [
        ("eventId", proposal.event_id.as_str()),
        ("projectId", proposal.project_id.as_str()),
        ("rootSessionId", proposal.root_session_id.as_str()),
        ("currentSessionId", proposal.current_session_id.as_str()),
        (
            "proposedChildSessionId",
            proposal.proposed_child_session_id.as_str(),
        ),
        (
            "proposedChildResidentName",
            proposal.proposed_child_resident_name.as_str(),
        ),
    ] {
        if value.trim().is_empty() {
            return Err(format!("{field} must be non-empty text"));
        }
    }
    if proposal.current_session_id == proposal.proposed_child_session_id {
        return Err("delegation child must differ from current session".to_owned());
    }
    Ok(())
}

fn capability_text(capability: AgentSessionDelegationCapability) -> &'static str {
    match capability {
        AgentSessionDelegationCapability::Standard => "standard",
        AgentSessionDelegationCapability::FocusedLeaf => "focused-leaf",
    }
}

fn parse_capability(value: &str) -> Result<AgentSessionDelegationCapability, String> {
    match value {
        "standard" => Ok(AgentSessionDelegationCapability::Standard),
        "focused-leaf" => Ok(AgentSessionDelegationCapability::FocusedLeaf),
        other => Err(format!("unknown delegation capability: {other}")),
    }
}

fn digest_serializable(value: &impl Serialize) -> Result<String, String> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| format!("failed to encode session control-plane digest input: {error}"))?;
    Ok(format!("blake3-256:{}", blake3::hash(&bytes).to_hex()))
}
#[path = "session_control_plane_transaction_owner.rs"]
mod transaction_owner;
