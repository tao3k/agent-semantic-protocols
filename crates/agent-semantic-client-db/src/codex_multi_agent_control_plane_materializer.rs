use agent_semantic_context_product::agent_session_lifecycle::{
    AgentSessionLifecycleProjection, BindingPhase, DispatchLifecycleProjection, DispatchPhase,
    HostBindingProjection, ServerHealth, SessionLifecycleProjection, SessionPhase,
    WorkspaceServerProjection,
};
use agent_semantic_context_product::codex_multi_agent_v2_control_plane::{
    CodexAgentNodeProjection, CodexControlPlaneFreshness, CodexControlPlaneMaterialization,
    CodexMultiAgentV2ControlPlaneProjection,
};
use std::collections::BTreeSet;

use crate::agent_session_registry::{
    AgentSessionRecord, agent_session_message_target_is_live_bound,
};

pub(crate) fn materialize_codex_multi_agent_control_plane(
    workspace_identity: &str,
    project_id: &str,
    root_session_id: &str,
    mut records: Vec<AgentSessionRecord>,
    current: Option<&CodexMultiAgentV2ControlPlaneProjection>,
) -> Result<CodexMultiAgentV2ControlPlaneProjection, String> {
    if workspace_identity.trim().is_empty() {
        return Err("Codex control-plane refresh requires a workspace identity".to_owned());
    }
    if project_id.trim().is_empty() {
        return Err("Codex control-plane refresh requires a project id".to_owned());
    }
    if project_id != workspace_identity {
        return Err(format!(
            "AgentSession Registry project scope must match the Runtime Server workspace: workspaceIdentity={workspace_identity} projectId={project_id}"
        ));
    }
    if root_session_id.trim().is_empty() {
        return Err("Codex control-plane refresh requires a root session id".to_owned());
    }

    records.sort_by(|left, right| left.session_id().cmp(right.session_id()));
    if records.iter().any(|record| {
        record.project_id() != project_id || record.root_session_id() != root_session_id
    }) {
        return Err(
            "AgentSession Registry returned a record outside the requested control-plane scope"
                .to_owned(),
        );
    }

    if records.is_empty() {
        return Err(
            "Codex root task requires a non-empty scoped AgentSession Registry snapshot".to_owned(),
        );
    }
    if records
        .iter()
        .any(|record| record.session_id() == root_session_id)
    {
        return Err(
            "Codex root task must not be encoded as an AgentSession Registry row".to_owned(),
        );
    }

    let canonical_records = serde_json::to_vec(&records)
        .map_err(|error| format!("failed to serialize AgentSession Registry snapshot: {error}"))?;
    let source_digest = format!("blake3-256:{}", blake3::hash(&canonical_records).to_hex());
    let workspace_server = WorkspaceServerProjection {
        workspace_identity: workspace_identity.to_owned(),
        health: ServerHealth::Ready,
    };

    if let Some(current) = current.filter(|projection| {
        projection.materialization.freshness == CodexControlPlaneFreshness::Current
            && projection.materialization.source_digest.as_deref() == Some(source_digest.as_str())
            && projection.workspace_server == workspace_server
    }) {
        return Ok(current.clone());
    }

    let generation = current
        .map(|projection| projection.materialization.generation.saturating_add(1))
        .unwrap_or(1);
    let registry_evidence_ref =
        format!("agent-session-registry://{project_id}/{root_session_id}?digest={source_digest}");
    let evidence_refs = std::iter::once(registry_evidence_ref)
        .chain(
            records
                .iter()
                .filter_map(AgentSessionRecord::last_evidence_ref)
                .map(str::to_owned),
        )
        .chain(
            records
                .iter()
                .filter_map(|record| record.model_evidence_ref.as_deref())
                .map(str::to_owned),
        )
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    let agents = records
        .iter()
        .map(|record| materialize_agent(record, &workspace_server, root_session_id))
        .collect::<Result<Vec<_>, _>>()?;

    CodexMultiAgentV2ControlPlaneProjection::new(
        CodexControlPlaneMaterialization {
            generation,
            source_digest: Some(source_digest),
            evidence_refs,
            freshness: CodexControlPlaneFreshness::Current,
        },
        workspace_server,
        root_session_id.to_owned(),
        agents,
        // The registry is durable evidence for agent identities and bindings.
        // It does not own Codex turn ids or delegation delivery receipts.
        Vec::new(),
        Vec::new(),
    )
}

fn materialize_agent(
    record: &AgentSessionRecord,
    workspace_server: &WorkspaceServerProjection,
    root_session_id: &str,
) -> Result<CodexAgentNodeProjection, String> {
    let generation = u64::try_from(record.physical_generation).map_err(|_| {
        format!(
            "agent session `{}` has non-positive physical generation {}",
            record.session_id(),
            record.physical_generation
        )
    })?;
    if generation == 0 {
        return Err(format!(
            "agent session `{}` has zero physical generation",
            record.session_id()
        ));
    }

    let session_phase = match record.status() {
        "active" | "idle" => SessionPhase::Active,
        "archived" => SessionPhase::Archived,
        "invalid" => SessionPhase::Retired,
        status => {
            return Err(format!(
                "agent session `{}` has unsupported registry status `{status}`",
                record.session_id()
            ));
        }
    };
    let live_bound = agent_session_message_target_is_live_bound(record, root_session_id);
    let binding_phase = if live_bound {
        BindingPhase::Fresh
    } else if record.message_target_id().is_some() {
        BindingPhase::Stale
    } else {
        BindingPhase::Unbound
    };
    let binding_generation = live_bound.then_some(generation);
    let canonical_message_target = record.message_target_id().map(str::to_owned);

    Ok(CodexAgentNodeProjection {
        root_session_id: root_session_id.to_owned(),
        parent_session_id: record.parent_session_id().map(str::to_owned),
        resident_name: record.name().to_owned(),
        role: record.role().to_owned(),
        configured_agent_type: record
            .configured_agent_type
            .as_ref()
            .map(|value| value.as_str().to_owned()),
        lifecycle: AgentSessionLifecycleProjection::new(
            workspace_server.clone(),
            SessionLifecycleProjection {
                session_id: Some(record.session_id().to_owned()),
                generation: Some(generation),
                phase: session_phase,
            },
            HostBindingProjection {
                generation: binding_generation,
                phase: binding_phase,
                child_session_id: live_bound.then(|| record.session_id().to_owned()),
                canonical_message_target,
                termination_receipt_indexed: false,
                path_release_receipt_indexed: false,
            },
            // Registry status is not a durable Codex dispatch claim or turn receipt.
            DispatchLifecycleProjection {
                generation: None,
                phase: DispatchPhase::Unobserved,
            },
        ),
    })
}

#[cfg(test)]
#[path = "../tests/unit/codex_multi_agent_control_plane_materializer.rs"]
mod tests;
