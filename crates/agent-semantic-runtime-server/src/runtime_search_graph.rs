//! Runtime-owned graph ranking for one admitted resident search generation.

use std::path::Path;

use agent_semantic_client_db::{
    runtime_resident_read::RuntimeResidentReadClient,
    runtime_search_service::RuntimeSearchServiceHandle,
};
use agent_semantic_search::{ResidentGraphSearchRequest, ResidentGraphSearchStage};
use agent_semantic_search_projection::{
    GraphTurboEvaluationRequest, GraphTurboResultPacketV1, ResidentSearchHit,
};
use serde_json::{Value, json};

/// A typed boundary error that the public ClientFrame route terminalizes.
pub(crate) struct RuntimeSearchGraphFailure {
    pub(crate) reason_kind: &'static str,
    pub(crate) message: String,
    pub(crate) details: Option<Value>,
}

impl RuntimeSearchGraphFailure {
    fn invalid(message: impl Into<String>) -> Self {
        Self {
            reason_kind: "graph-search-invalid",
            message: message.into(),
            details: None,
        }
    }

    fn service(
        message: impl Into<String>,
        generation_digest: &str,
        source_root_digest: &str,
    ) -> Self {
        Self {
            reason_kind: "graph-search-failed",
            message: message.into(),
            details: Some(json!({
                "generationDigest": generation_digest,
                "sourceRootDigest": source_root_digest,
            })),
        }
    }
}

/// Rank the resident lexical frontier through the one Runtime-owned graph
/// session. The immutable generation graph is opened once and shared by `Arc`;
/// warm queries carry only bounded query terms and owner-node seed IDs.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn rank_resident_search_frontier(
    runtime_search_service: &RuntimeSearchServiceHandle,
    project_root: &Path,
    workspace_identity: &str,
    request_id: &str,
    operation: &str,
    query: &str,
    language_id: &str,
    provider_id: &str,
    generation_digest: &str,
    generation_token: u64,
    resident: &RuntimeResidentReadClient,
    lexical_hits: &[ResidentSearchHit],
) -> Result<Option<ResidentGraphSearchStage>, RuntimeSearchGraphFailure> {
    if operation != "pipe" || lexical_hits.is_empty() {
        return Ok(None);
    }

    let authority = resident.search_generation_authority();
    let source_snapshot = &authority.source_snapshot;
    let workspace_generation = &authority.workspace_generation;
    let graph_generation = resident.graph_generation();
    let request =
        agent_semantic_search::build_resident_graph_search_request(ResidentGraphSearchRequest {
            operation_id: request_id,
            operation,
            query,
            language_id,
            provider_id,
            generation_digest,
            source_snapshot,
            workspace_generation,
            lexical_hits,
            generation_graph: graph_generation,
        })
        .map_err(RuntimeSearchGraphFailure::invalid)?
        .ok_or_else(|| {
            RuntimeSearchGraphFailure::invalid(
                "graph-required search omitted its resident lexical frontier",
            )
        })?;
    let request = GraphTurboEvaluationRequest::from_value(request)
        .map_err(|error| {
            RuntimeSearchGraphFailure::invalid(format!(
                "invalid resident graph search request: {error}"
            ))
        })?
        .into_value();

    let started = tokio::time::Instant::now();
    let result = runtime_search_service
        .graphs_evaluate(
            project_root.to_path_buf(),
            workspace_identity.to_owned(),
            generation_digest.to_owned(),
            source_snapshot.root_digest.clone(),
            generation_token,
            graph_generation.digest().to_owned(),
            graph_generation.shared_open_payload(),
            request_id.to_owned(),
            request,
        )
        .await
        .map_err(|error| {
            RuntimeSearchGraphFailure::service(
                error,
                generation_digest,
                &source_snapshot.root_digest,
            )
        })?;
    let result = GraphTurboResultPacketV1::from_value(result).map_err(|error| {
        RuntimeSearchGraphFailure::invalid(format!("invalid resident graph search result: {error}"))
    })?;
    agent_semantic_search::project_resident_graph_search_result(
        generation_digest,
        result,
        started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64,
    )
    .map(Some)
    .map_err(RuntimeSearchGraphFailure::invalid)
}
