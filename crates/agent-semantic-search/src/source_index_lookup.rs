//! DB-backed source-index lookup adapter.

use std::path::Path;

use agent_semantic_client_core::LanguageId;
use agent_semantic_client_db::ClientDbSourceIndexLookupResult;
use agent_semantic_client_db::workspace_db_ipc::WorkspaceDbSourceIndexLookupRequest;

use crate::reorder_source_index_candidates;

/// Request for looking up source-index owners from one project's cache.
#[derive(Clone, Copy, Debug)]
pub struct SourceIndexLookupRequest<'a> {
    pub cache_project_root: &'a Path,
    pub indexed_project_root: &'a Path,
    pub language_id: Option<&'a LanguageId>,
    pub query: &'a str,
    pub limit: u32,
    pub source_snapshot: &'a agent_semantic_content_identity::SourceSnapshotEvidence,
}

fn source_index_artifact_digest(
    source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
) -> String {
    agent_semantic_client_db::client_db_source_index_artifact_digest(source_snapshot)
}

/// Request for source-index lookup with an optional warm search planner.
#[derive(Clone, Copy, Debug)]
pub struct SourceIndexPlannerLookupRequest<'a> {
    /// Existing source-index lookup request.
    pub source_index: SourceIndexLookupRequest<'a>,
    /// Optional warm path index used before falling through to provider work.
    pub file_locator: Option<&'a crate::file_locator::FileLocatorIndex>,
}

/// Lookup source-index owners from the client DB for one project root.
pub fn lookup_source_index(
    project_root: &Path,
    source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
    query: &str,
    limit: u32,
) -> Result<ClientDbSourceIndexLookupResult, String> {
    lookup_source_index_for_language(project_root, source_snapshot, None, query, limit)
}

/// Lookup source-index owners from the client DB for one language scope.
pub fn lookup_source_index_for_language(
    project_root: &Path,
    source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
    language_id: Option<&LanguageId>,
    query: &str,
    limit: u32,
) -> Result<ClientDbSourceIndexLookupResult, String> {
    lookup_source_index_in_cache(SourceIndexLookupRequest {
        cache_project_root: project_root,
        indexed_project_root: project_root,
        language_id,
        query,
        limit,
        source_snapshot,
    })
}

/// Lookup source-index owners from one project's client DB for an explicit
/// indexed project root.
pub fn lookup_source_index_in_cache(
    request: SourceIndexLookupRequest<'_>,
) -> Result<ClientDbSourceIndexLookupResult, String> {
    let lookup = agent_semantic_client_db::workspace_db_ipc::read_source_index_via_runtime_server(
        WorkspaceDbSourceIndexLookupRequest {
            project_root: request.cache_project_root.to_path_buf(),
            indexed_project_root: request.indexed_project_root.to_path_buf(),
            source_snapshot: request.source_snapshot.clone(),
            query: request.query.to_owned(),
            language_id: request.language_id.cloned(),
            limit: request.limit,
        },
    )?;
    let lookup = rank_source_index_lookup_result(lookup, request.query);
    if !lookup.candidates.is_empty() {
        return Ok(lookup);
    }

    // Turso is durable state, not the interactive read path. A miss is returned
    // to the planner so it can choose a bounded backend without a blocking DB scan.
    Ok(lookup)
}

/// Use a warm file locator first, then query the resident workspace owner.
pub fn lookup_source_index_with_planner(
    request: SourceIndexPlannerLookupRequest<'_>,
) -> Result<ClientDbSourceIndexLookupResult, String> {
    if let Some(file_locator) = request.file_locator
        && let Some(file_lookup) =
            source_index_file_locator_lookup(request.source_index, file_locator)
    {
        return Ok(file_lookup);
    }
    lookup_source_index_in_cache(request.source_index)
}

fn source_index_file_locator_lookup(
    request: SourceIndexLookupRequest<'_>,
    file_locator: &crate::file_locator::FileLocatorIndex,
) -> Option<ClientDbSourceIndexLookupResult> {
    let decision =
        crate::search_planner::plan_search_route(crate::search_planner::SearchPlannerRequest {
            query: request.query,
            limit: request.limit as usize,
            file_locator: Some(file_locator),
        });
    if decision.route != crate::search_planner::SearchPlannerRoute::FileLocator {
        return None;
    }
    let candidates = decision
        .file_candidates
        .into_iter()
        .map(|candidate| {
            let path = candidate.workspace_relative_path;
            agent_semantic_client_db::ClientDbSourceIndexCandidate {
                path: path.clone().into(),
                language_id: request.language_id.cloned(),
                provider_id: None,
                source_kind: agent_semantic_client_db::ClientDbSourceIndexSourceKind::File,
                line_count: None,
                query_keys: vec![path.into()],
                selector_symbol: None,
                selector_kind: None,
                selector_proof: None,
            }
        })
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return None;
    }
    Some(agent_semantic_client_db::ClientDbSourceIndexLookupResult {
        db_path: std::path::PathBuf::new(),
        state: agent_semantic_client_db::ClientDbSourceIndexLookupState::Hit,
        candidates,
        source_snapshot: Some(request.source_snapshot.clone()),
        index_artifact_digest: Some(source_index_artifact_digest(request.source_snapshot)),
    })
}

pub fn rank_source_index_lookup_result(
    mut lookup: ClientDbSourceIndexLookupResult,
    query: &str,
) -> ClientDbSourceIndexLookupResult {
    lookup.candidates = reorder_source_index_candidates(
        lookup.candidates,
        query,
        |candidate| (candidate.path.clone()).to_string(),
        |candidate| {
            (candidate.query_keys.clone())
                .into_iter()
                .map(|value| value.to_string())
                .collect()
        },
    );
    lookup
}

pub fn search_pipe_source_index_lookup_from_client_result(
    result: ClientDbSourceIndexLookupResult,
) -> crate::SearchPipeSourceIndexLookup {
    crate::SearchPipeSourceIndexLookup {
        state: result.state.as_str().to_string().into(),
        candidates: result
            .candidates
            .into_iter()
            .map(|candidate| crate::SearchPipeSourceIndexCandidate {
                path: candidate.path.as_str().to_string().into(),
                language_id: candidate
                    .language_id
                    .map(|value| value.as_str().to_string().into()),
                provider_id: candidate
                    .provider_id
                    .map(|value| value.as_str().to_string().into()),
                source_kind: candidate.source_kind.as_str().to_string().into(),
                line_count: candidate.line_count,
                query_keys: candidate
                    .query_keys
                    .into_iter()
                    .map(|key| key.as_str().to_string().into())
                    .collect(),
                selector_proof: candidate.selector_proof,
            })
            .collect(),
        source_snapshot: result.source_snapshot,
        index_artifact_digest: result.index_artifact_digest.map(Into::into),
    }
}
