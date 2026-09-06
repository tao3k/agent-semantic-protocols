use crate::runtime_asp_client::AspClientOperationError;
use crate::runtime_asp_client::classify_exact_query_failure;
use crate::runtime_asp_client::elapsed_micros;
use crate::runtime_asp_client::record_runtime_route_performance;
use crate::runtime_query_generation::RuntimeQueryGeneration;
use agent_semantic_client_protocol::AspClientExactQueryFailure;
use agent_semantic_client_protocol::AspClientExactQueryRequest;
use agent_semantic_client_protocol::AspClientExactQueryResponse;
use agent_semantic_client_protocol::AspClientRuntimeWorkCounters;
use agent_semantic_client_server::AspClientDispatchError;

#[allow(clippy::too_many_arguments)]
pub(super) fn dispatch_source_index_lookup(
    workspace_id: &str,
    request_id: &str,
    params: serde_json::Value,
    project_root: &std::path::Path,
    language_id: &str,
    provider_id: &str,
    generation: &RuntimeQueryGeneration,
    telemetry_sender: &agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryBusSender,
) -> Result<serde_json::Value, AspClientOperationError> {
    let started = tokio::time::Instant::now();
    let params: agent_semantic_client_protocol::AspClientSourceIndexLookupRequest =
        serde_json::from_value(params)
            .map_err(|error| format!("decode ASP client source-index lookup request: {error}"))?;
    params.validate_schema_identity()?;
    if params.query.trim().is_empty() || !(1..=100).contains(&params.limit) {
        return Err(AspClientOperationError::Message(
            "source-index lookup requires a non-empty query and limit in 1..=100".to_owned(),
        ));
    }
    let requested_root = std::path::PathBuf::from(&params.index_root)
        .canonicalize()
        .map_err(|error| format!("canonicalize source-index indexRoot: {error}"))?;
    let admitted_root = project_root
        .canonicalize()
        .map_err(|error| format!("canonicalize admitted workspace root: {error}"))?;
    if requested_root != admitted_root {
        return Err(AspClientOperationError::Terminal(AspClientDispatchError {
            reason_kind: "source-index-workspace-mismatch".to_owned(),
            message: "source-index indexRoot is not the initialized workspace".to_owned(),
            details: Some(serde_json::json!({
                "requestedIndexRoot": requested_root,
                "admittedWorkspaceRoot": admitted_root,
            })),
        }));
    }
    let language = agent_semantic_client_core::LanguageId::try_from(language_id)
        .map_err(|error| format!("decode language id: {error}"))?;
    let authority = agent_semantic_search::ResidentSearchAuthority {
        language_id: language,
        provider_id: provider_id.into(),
    };
    let lookup =
        generation
            .resident()
            .read_source_index(&params.query, Some(&authority), params.limit)?;
    record_runtime_route_performance(
        &telemetry_sender,
        workspace_id,
        language_id,
        generation.generation_digest(),
        request_id,
        "source-index",
        "runtime-source-index-read",
        "lookup",
        elapsed_micros(started),
    )?;
    Ok(serde_json::to_value(lookup.as_ref())
        .map_err(|error| format!("encode source-index lookup: {error}"))?)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn dispatch_exact_query(
    project_id: &str,
    workspace_id: &str,
    request_id: &str,
    params: serde_json::Value,
    language_id: &str,
    provider_id: &str,
    generation: &RuntimeQueryGeneration,
    telemetry_sender: &agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryBusSender,
) -> Result<serde_json::Value, AspClientOperationError> {
    let started = tokio::time::Instant::now();
    let params: AspClientExactQueryRequest = serde_json::from_value(params)
        .map_err(|error| format!("decode ASP client exact-query request: {error}"))?;
    params.validate_schema_identity()?;
    let projection_kind =
        agent_semantic_client_db::runtime_server_workspace::ExactProjectionKind::try_from(
            params.projection.as_str(),
        )?;
    let resident_started = tokio::time::Instant::now();
    if generation.native_syntax_state() != "ready" {
        return Err(AspClientOperationError::Terminal(AspClientDispatchError {
            reason_kind: "native-syntax-not-ready".to_owned(),
            message:
                "parser-owned exact projections are not ready for the admitted content generation"
                    .to_owned(),
            details: Some(serde_json::json!({
                "projectId": project_id,
                "workspaceId": workspace_id,
                "generationDigest": generation.generation_digest(),
                "contentGenerationDigest": generation.content_generation_digest(),
                "attachmentState": generation.native_syntax_state(),
            })),
        }));
    }
    let projection = generation.read_runtime_selector(projection_kind, &params.selector)?;
    let resident_read_elapsed_micros = elapsed_micros(resident_started);
    let elapsed_micros = elapsed_micros(started);
    let service_elapsed_micros = elapsed_micros.saturating_sub(resident_read_elapsed_micros);
    record_runtime_route_performance(
        &telemetry_sender,
        workspace_id,
        language_id,
        generation.generation_digest(),
        request_id,
        "query",
        "runtime-selector-read",
        projection_kind.as_str(),
        elapsed_micros,
    )?;
    if let Some(failure) = classify_exact_query_failure(&projection) {
        let selector_details =
            serde_json::to_value(&projection).map_err(|error| error.to_string())?;
        let selector_state = selector_details
            .get("state")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "Runtime selector failure has no typed state".to_owned())?;
        let failure_terminal = AspClientExactQueryFailure {
            schema_id: "agent.semantic-protocols.asp-client-exact-query-failure".to_owned(),
            schema_version: "1".to_owned(),
            state: "failed".to_owned(),
            operation_id: request_id.to_owned(),
            project_id: project_id.to_owned(),
            workspace_id: workspace_id.to_owned(),
            language_id: language_id.to_owned(),
            provider_id: provider_id.to_owned(),
            requested_selector: Some(params.selector),
            resolved_selector: failure.resolved_selector,
            projection_kind: Some(params.projection),
            phase: "resident-selector-read".to_owned(),
            reason_kind: failure.reason_kind.to_owned(),
            generation_digest: Some(generation.generation_digest().to_owned()),
            root_digest: Some(generation.resident().source_root_digest()),
            resident_read_elapsed_micros,
            service_elapsed_micros,
            elapsed_micros,
            work_counters: AspClientRuntimeWorkCounters::default(),
            details: serde_json::json!({
                "selectorState": selector_state,
                "selectorRead": selector_details,
            }),
        };
        failure_terminal.validate()?;
        let details = serde_json::to_value(failure_terminal).map_err(|error| error.to_string())?;
        return Err(AspClientOperationError::Terminal(AspClientDispatchError {
            reason_kind: failure.reason_kind.to_owned(),
            message: format!("exact projection terminal: {}", failure.reason_kind),
            details: Some(details),
        }));
    }
    let response = AspClientExactQueryResponse {
        schema_id: "agent.semantic-protocols.asp-client-exact-query-response".to_owned(),
        schema_version: "1".to_owned(),
        operation_id: request_id.to_owned(),
        project_id: project_id.to_owned(),
        workspace_id: workspace_id.to_owned(),
        language_id: language_id.to_owned(),
        provider_id: provider_id.to_owned(),
        generation_digest: generation.generation_digest().to_owned(),
        root_digest: generation.resident().source_root_digest(),
        result: serde_json::to_value(projection)
            .map_err(|error| format!("encode query result: {error}"))?,
        resident_read_elapsed_micros,
        service_elapsed_micros,
        elapsed_micros,
        work_counters: AspClientRuntimeWorkCounters::default(),
    };
    response.validate()?;
    Ok(
        serde_json::to_value(response)
            .map_err(|error| format!("encode query response: {error}"))?,
    )
}
