use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    runtime_server_workspace::ExactProjectionKind,
    workspace_db_ipc::{WorkspaceDbIpcOperation, WorkspaceDbIpcRequest, WorkspaceDbIpcResult},
};

use super::{
    IncidentSurface, RequestedProjection, ResourceObservation, SearchIncidentTerminalContext,
};

pub(crate) fn workspace_ipc_terminal_context(
    request: &WorkspaceDbIpcRequest,
) -> Option<SearchIncidentTerminalContext> {
    let (language_id, surface, requested_projection, stage) = match &request.operation {
        WorkspaceDbIpcOperation::ReadRuntimeSelector {
            language_id,
            projection_kind,
            ..
        } => (
            language_id.to_string(),
            IncidentSurface::Query,
            match projection_kind {
                ExactProjectionKind::Source => RequestedProjection::Source,
                ExactProjectionKind::CallableSkeleton => RequestedProjection::CallableSkeleton,
            },
            "runtime-selector-read",
        ),
        WorkspaceDbIpcOperation::ReadSourceIndex { request } => (
            request.language_id.as_ref()?.to_string(),
            IncidentSurface::Search,
            RequestedProjection::Seeds,
            "source-index-read",
        ),
        WorkspaceDbIpcOperation::ReadRuntimeSearchGenerationAuthority { .. } => (
            "runtime".to_owned(),
            IncidentSurface::Search,
            RequestedProjection::Seeds,
            "resident-exact-generation-open",
        ),
        WorkspaceDbIpcOperation::ReadRuntimeOwner { .. } => (
            "runtime".to_owned(),
            IncidentSurface::Search,
            RequestedProjection::Seeds,
            "runtime-owner-read",
        ),
        WorkspaceDbIpcOperation::ReadRuntimeGraphFacts { .. } => (
            "runtime".to_owned(),
            IncidentSurface::Search,
            RequestedProjection::Seeds,
            "runtime-graph-facts-read",
        ),
        _ => return None,
    };
    let canonical_request = serde_json::to_vec(&request.operation).ok()?;
    Some(SearchIncidentTerminalContext {
        workspace_identity: request.workspace_identity.clone(),
        language_id,
        surface,
        canonical_request_digest: format!(
            "blake3-256:{}",
            blake3::hash(&canonical_request).to_hex()
        ),
        requested_projection,
        stage: stage.to_owned(),
        runtime_artifact_digest: None,
        provider_contract_digest: None,
        generation_digest: None,
        budget_micros: None,
        elapsed_micros: None,
        observed_at_unix_micros: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_micros() as u64)
            .unwrap_or_default(),
        resources: ResourceObservation::default(),
    })
}

pub(crate) fn record_workspace_ipc_terminal(
    sender: Option<&crate::runtime_telemetry_bus::RuntimeTelemetryBusSender>,
    context: Option<SearchIncidentTerminalContext>,
    result: &WorkspaceDbIpcResult,
    elapsed: std::time::Duration,
) -> Result<
    Option<super::IncidentRecord>,
    crate::runtime_telemetry_bus::RuntimeIncidentAdmissionError,
> {
    let (Some(sender), Some(mut context)) = (sender, context) else {
        return Ok(None);
    };
    context.elapsed_micros = Some(elapsed.as_micros() as u64);
    let reason_kind = match result {
        WorkspaceDbIpcResult::Failed { code, .. } => code.clone(),
        _ => return Ok(None),
    };
    sender.try_record_search_terminal(
        context,
        super::SearchIncidentTerminalOutcome::Failed { reason_kind },
    )
}

#[cfg(test)]
#[path = "../../tests/unit/search_incident_workspace_ipc.rs"]
mod tests;
