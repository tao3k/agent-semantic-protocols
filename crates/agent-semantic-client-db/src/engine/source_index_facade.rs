//! Source-index and structural-index DB Engine facade methods.

use std::path::{Path, PathBuf};

use agent_semantic_client_core::{
    LanguageId, ProviderId, project_client_cache_dir_read_only, state_core::TURSO_BACKEND,
};

use crate::evidence_graph::{source_index_evidence_graph, structural_index_evidence_graph};
use crate::source_index::{
    ClientDbSourceIndexCandidate, ClientDbSourceIndexClientDirLookupRequest,
    ClientDbSourceIndexImport, ClientDbSourceIndexLookupResult, ClientDbSourceIndexLookupState,
    ClientDbSourceIndexProjectLookupRequest, ClientDbSourceIndexRefreshRequest,
    ClientDbSourceIndexSelectorPayloadProof, ClientDbSourceIndexSourceKind,
};
use crate::structural_index::ClientDbStructuralIndexImport;

use super::facade::{ClientDbEngine, block_on_db_engine_async};
use super::turso::{connect_turso_client_db, turso_table_exists};
use super::turso_evidence_graph::{
    TursoClientDbEvidenceGraphPersistReport, persist_turso_evidence_graph,
};
use super::turso_lock_policy::is_turso_lock_error;
use super::turso_search::upsert_turso_search_documents;
use super::turso_source_index::refresh_turso_source_index_import;
use super::turso_statement::run_turso_operation_with_lock_retry;
use super::{
    ClientDbEngineSourceIndexReadModelReport, ClientDbEngineStructuralIndexReadModelReport,
};

impl ClientDbEngine {
    /// Lookup source-index candidates from one project's resolved DB Engine state.
    pub fn lookup_source_index_from_project(
        request: ClientDbSourceIndexProjectLookupRequest<'_>,
    ) -> Result<ClientDbSourceIndexLookupResult, String> {
        let client_dir = project_client_cache_dir_read_only(request.cache_project_root)?;
        Self::lookup_source_index_from_client_dir(ClientDbSourceIndexClientDirLookupRequest {
            client_dir: &client_dir,
            indexed_project_root: request.indexed_project_root,
            language_id: request.language_id,
            query_keys: request.query_keys,
            limit: request.limit,
        })
    }

    /// Lookup source-index candidates through the active Turso read model.
    pub fn lookup_source_index_from_client_dir(
        request: ClientDbSourceIndexClientDirLookupRequest<'_>,
    ) -> Result<ClientDbSourceIndexLookupResult, String> {
        let query = request
            .query_keys
            .iter()
            .map(|key| key.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let db_path = Self::turso_path_for_client_dir(request.client_dir);
        let language_id = request.language_id.cloned();
        let limit = request.limit;
        block_on_db_engine_async(async move {
            lookup_source_index_read_model_at_path(
                db_path,
                query.as_str(),
                language_id.as_ref(),
                limit,
            )
            .await
        })
    }

    /// Lookup source-index candidates from the active Turso EvidenceGraph read model.
    pub async fn lookup_source_index_read_model(
        &self,
        query: &str,
        language_id: Option<&LanguageId>,
        limit: u32,
    ) -> Result<ClientDbSourceIndexLookupResult, String> {
        if self.backend() != super::ClientDbBackend::Turso {
            return Err(format!(
                "active DB Engine backend is {}, expected {}",
                self.backend().as_str(),
                TURSO_BACKEND
            ));
        }
        lookup_source_index_read_model_at_path(
            self.db_path().to_path_buf(),
            query,
            language_id,
            limit,
        )
        .await
    }

    /// Lookup source-index candidates from a resolved client directory's Turso read model.
    pub async fn lookup_source_index_read_model_from_client_dir(
        client_dir: impl AsRef<Path>,
        query: &str,
        language_id: Option<&LanguageId>,
        limit: u32,
    ) -> Result<ClientDbSourceIndexLookupResult, String> {
        lookup_source_index_read_model_at_path(
            Self::turso_path_for_client_dir(client_dir),
            query,
            language_id,
            limit,
        )
        .await
    }

    /// Persist stable source-index graph and search documents through the active DB Engine backend.
    pub async fn persist_source_index_read_model(
        &self,
        import: &ClientDbSourceIndexImport,
    ) -> Result<ClientDbEngineSourceIndexReadModelReport, String> {
        let trace_started = std::time::Instant::now();
        let refresh = refresh_turso_source_index_import(
            self.db_path(),
            ClientDbSourceIndexRefreshRequest {
                import: import.clone(),
                file_count: import.file_hashes.len().min(u32::MAX as usize) as u32,
            },
        )
        .await?;
        db_engine_trace("source-index-refresh-read-model", trace_started);
        let graph = source_index_evidence_graph(import);
        db_engine_trace("source-index-graph-built", trace_started);
        let graph_report = TursoClientDbEvidenceGraphPersistReport {
            entity_count: graph.nodes.len(),
            edge_count: graph.edges.len(),
        };
        let search_document_count = refresh.owner_count as usize;
        Ok(source_index_read_model_report(
            graph_report,
            search_document_count,
        ))
    }

    /// Persist stable structural-index graph facts through the active DB Engine backend.
    pub async fn persist_structural_index_read_model(
        &self,
        import: &ClientDbStructuralIndexImport,
    ) -> Result<ClientDbEngineStructuralIndexReadModelReport, String> {
        persist_structural_index_read_model_at_path(self.db_path(), import).await
    }
}

fn db_engine_trace(stage: &str, started: std::time::Instant) {
    if std::env::var_os("ASP_SOURCE_INDEX_TRACE").is_some() {
        eprintln!(
            "[db-engine-trace] stage={} elapsedMs={}",
            stage,
            started.elapsed().as_millis()
        );
    }
}

fn source_index_lookup_result(
    db_path: PathBuf,
    state: ClientDbSourceIndexLookupState,
    candidates: Vec<crate::ClientDbSourceIndexCandidate>,
) -> ClientDbSourceIndexLookupResult {
    ClientDbSourceIndexLookupResult {
        db_path,
        state,
        candidates,
    }
}

fn source_index_busy_lookup_result(db_path: PathBuf) -> ClientDbSourceIndexLookupResult {
    source_index_lookup_result(db_path, ClientDbSourceIndexLookupState::Busy, Vec::new())
}

fn source_index_read_model_report(
    graph_report: TursoClientDbEvidenceGraphPersistReport,
    search_document_count: usize,
) -> ClientDbEngineSourceIndexReadModelReport {
    ClientDbEngineSourceIndexReadModelReport {
        graph_entity_count: graph_report.entity_count,
        graph_edge_count: graph_report.edge_count,
        search_document_count,
    }
}

async fn persist_structural_index_search_documents_at_path(
    db_path: &Path,
    generation_id: &str,
    graph: &crate::ClientDbEvidenceGraph,
) -> Result<usize, String> {
    let mut documents = Vec::new();
    for node in graph
        .nodes
        .iter()
        .filter(|node| matches!(node.kind, "symbol" | "dependency-usage"))
    {
        let mut terms = vec![node.kind.to_string(), node.label.clone()];
        if let Some(path) = &node.path {
            terms.push(path.clone());
        }
        if let Some(selector) = &node.selector {
            terms.push(selector.clone());
        }
        if let Some(language_id) = &node.language_id {
            terms.push(language_id.clone());
        }
        if let Some(provider_id) = &node.provider_id {
            terms.push(provider_id.clone());
        }
        terms.extend(node.query_keys.iter().cloned());
        let document = crate::TursoClientDbSearchDocument {
            namespace: "structural-index".to_string(),
            document_id: format!("structural-index:{generation_id}:{}", node.id),
            entity_id: node.id.clone(),
            selector: node.selector.clone(),
            document: terms.join(" "),
        };
        documents.push(document);
    }
    upsert_turso_search_documents(db_path, &documents).await
}

pub(super) async fn persist_structural_index_read_model_at_path(
    db_path: &Path,
    import: &ClientDbStructuralIndexImport,
) -> Result<ClientDbEngineStructuralIndexReadModelReport, String> {
    let _refresh_write_guard = structural_index_refresh_write_lock()
        .clone()
        .acquire_owned()
        .await
        .map_err(|error| format!("failed to acquire structural index refresh lock: {error}"))?;
    let trace_started = std::time::Instant::now();
    super::turso_bootstrap::bootstrap_turso_client_db(db_path).await?;
    db_engine_trace("structural-index-bootstrap", trace_started);
    let graph = structural_index_evidence_graph(import);
    db_engine_trace("structural-index-graph-built", trace_started);
    let graph_report = persist_turso_evidence_graph(db_path, &graph).await?;
    db_engine_trace("structural-index-graph-persisted", trace_started);
    let search_document_count = persist_structural_index_search_documents_at_path(
        db_path,
        import.generation_id.as_str(),
        &graph,
    )
    .await?;
    db_engine_trace("structural-index-search-documents-persisted", trace_started);
    Ok(structural_index_read_model_report(
        graph_report,
        search_document_count,
    ))
}

fn structural_index_refresh_write_lock() -> &'static std::sync::Arc<tokio::sync::Semaphore> {
    static LOCK: std::sync::OnceLock<std::sync::Arc<tokio::sync::Semaphore>> =
        std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Arc::new(tokio::sync::Semaphore::new(1)))
}

fn structural_index_read_model_report(
    graph_report: TursoClientDbEvidenceGraphPersistReport,
    search_document_count: usize,
) -> ClientDbEngineStructuralIndexReadModelReport {
    ClientDbEngineStructuralIndexReadModelReport {
        graph_entity_count: graph_report.entity_count,
        graph_edge_count: graph_report.edge_count,
        search_document_count,
    }
}

async fn lookup_source_index_read_model_at_path(
    db_path: PathBuf,
    query: &str,
    language_id: Option<&LanguageId>,
    limit: u32,
) -> Result<ClientDbSourceIndexLookupResult, String> {
    if !db_path.exists() {
        return Ok(source_index_lookup_result(
            db_path,
            ClientDbSourceIndexLookupState::MissingDb,
            Vec::new(),
        ));
    }
    if limit == 0 {
        return Ok(source_index_lookup_result(
            db_path,
            ClientDbSourceIndexLookupState::Miss,
            Vec::new(),
        ));
    }
    let terms = source_index_read_model_terms(query);
    let connection = match connect_turso_client_db(&db_path).await {
        Ok(connection) => connection,
        Err(error) if is_turso_lock_error(&error) => {
            return Ok(source_index_busy_lookup_result(db_path));
        }
        Err(error) => return Err(error),
    };
    let tables_exist = match turso_source_index_lookup_tables_exist(&connection).await {
        Ok(tables_exist) => tables_exist,
        Err(error) if is_turso_lock_error(&error) => {
            return Ok(source_index_busy_lookup_result(db_path));
        }
        Err(error) => return Err(error),
    };
    if !tables_exist {
        return Ok(source_index_lookup_result(
            db_path,
            ClientDbSourceIndexLookupState::EmptyIndex,
            Vec::new(),
        ));
    }
    match super::turso_source_index::ensure_turso_source_index_selector_columns(&connection).await {
        Ok(()) => {}
        Err(error) if is_turso_lock_error(&error) => {
            return Ok(source_index_busy_lookup_result(db_path));
        }
        Err(error) => return Err(error),
    }
    match super::turso_source_index::ensure_turso_source_index_owner_columns(&connection).await {
        Ok(()) => {}
        Err(error) if is_turso_lock_error(&error) => {
            return Ok(source_index_busy_lookup_result(db_path));
        }
        Err(error) => return Err(error),
    }
    let candidates = match query_turso_source_index_candidates_with_connection(
        &connection,
        query,
        language_id,
        limit,
        &terms,
    )
    .await
    {
        Ok(candidates) => candidates,
        Err(error) if is_turso_lock_error(&error) => {
            return Ok(source_index_busy_lookup_result(db_path));
        }
        Err(error) => return Err(error),
    };
    let owner_rows_exist = match turso_source_index_owner_rows_exist(&connection).await {
        Ok(owner_rows_exist) => owner_rows_exist,
        Err(error) if is_turso_lock_error(&error) => {
            return Ok(source_index_busy_lookup_result(db_path));
        }
        Err(error) => return Err(error),
    };
    if candidates.is_empty() && !owner_rows_exist {
        return Ok(source_index_lookup_result(
            db_path,
            ClientDbSourceIndexLookupState::EmptyIndex,
            Vec::new(),
        ));
    }
    let state = if candidates.is_empty() {
        ClientDbSourceIndexLookupState::Miss
    } else {
        ClientDbSourceIndexLookupState::Hit
    };
    Ok(source_index_lookup_result(db_path, state, candidates))
}

async fn turso_source_index_lookup_tables_exist(
    connection: &turso::Connection,
) -> Result<bool, String> {
    for table_name in [
        "asp_source_index_generation",
        "asp_source_index_owner",
        "asp_source_index_selector",
    ] {
        if !turso_table_exists(connection, table_name).await? {
            return Ok(false);
        }
    }
    Ok(true)
}

async fn turso_source_index_owner_rows_exist(
    connection: &turso::Connection,
) -> Result<bool, String> {
    let mut rows = run_turso_operation_with_lock_retry(
        || async {
            connection
                .query("SELECT owner_path FROM asp_source_index_owner LIMIT 1", ())
                .await
                .map_err(|error| error.to_string())
        },
        "failed to inspect Turso source-index owner rows",
    )
    .await?;
    Ok(rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso source-index owner rows: {error}"))?
        .is_some())
}

async fn query_turso_source_index_candidates_with_connection(
    connection: &turso::Connection,
    query: &str,
    language_id: Option<&LanguageId>,
    limit: u32,
    terms: &[String],
) -> Result<Vec<ClientDbSourceIndexCandidate>, String> {
    if limit == 0 || query.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut generation_rows = run_turso_operation_with_lock_retry(
        || async {
            connection
                .query(
                    "SELECT generation_id
                     FROM asp_source_index_generation
                     ORDER BY updated_at_ms DESC
                     LIMIT 1",
                    (),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to query Turso source-index latest generation",
    )
    .await?;
    let Some(generation_row) = generation_rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso source-index latest generation: {error}"))?
    else {
        return Ok(Vec::new());
    };
    let generation_id = generation_row
        .get::<String>(0)
        .map_err(|error| format!("failed to read Turso source-index generation id: {error}"))?;
    drop(generation_rows);

    let mut selector_rows = run_turso_operation_with_lock_retry(
        || async {
            connection
                .query(
                    "SELECT owner_path, selector_id, COALESCE(symbol, ''), kind, COALESCE(source, ''), payload_kind, COALESCE(payload_bounded, 0), query_keys_json
                     FROM asp_source_index_selector
                     WHERE generation_id = ?1
                     ORDER BY owner_path, selector_id",
                    (generation_id.as_str(),),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to query Turso source-index selectors",
    )
    .await?;
    let mut selector_haystacks = std::collections::BTreeMap::<String, String>::new();
    let mut selector_proofs =
        std::collections::BTreeMap::<String, ClientDbSourceIndexSelectorPayloadProof>::new();
    while let Some(row) = selector_rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso source-index selector row: {error}"))?
    {
        let owner_path = row.get::<String>(0).map_err(|error| {
            format!("failed to read Turso source-index selector owner path: {error}")
        })?;
        let selector_id = row
            .get::<String>(1)
            .map_err(|error| format!("failed to read Turso source-index selector id: {error}"))?;
        let symbol = row.get::<String>(2).map_err(|error| {
            format!("failed to read Turso source-index selector symbol: {error}")
        })?;
        let kind = row
            .get::<String>(3)
            .map_err(|error| format!("failed to read Turso source-index selector kind: {error}"))?;
        let source = row.get::<String>(4).map_err(|error| {
            format!("failed to read Turso source-index selector source: {error}")
        })?;
        let payload_kind = row.get::<Option<String>>(5).map_err(|error| {
            format!("failed to read Turso source-index selector payload kind: {error}")
        })?;
        let payload_bounded = row.get::<i64>(6).map_err(|error| {
            format!("failed to read Turso source-index selector payload bound: {error}")
        })? != 0;
        let query_keys_json = row.get::<String>(7).map_err(|error| {
            format!("failed to read Turso source-index selector query keys: {error}")
        })?;
        if let Some(payload_kind) = payload_kind.filter(|value| !value.trim().is_empty()) {
            selector_proofs.entry(owner_path.clone()).or_insert(
                ClientDbSourceIndexSelectorPayloadProof {
                    structural_selector: selector_id.clone(),
                    payload_kind,
                    bounded: payload_bounded,
                },
            );
        }
        let haystack = selector_haystacks.entry(owner_path).or_default();
        haystack.push(' ');
        haystack.push_str(&selector_id);
        haystack.push(' ');
        haystack.push_str(&symbol);
        haystack.push(' ');
        haystack.push_str(&kind);
        haystack.push(' ');
        haystack.push_str(&source);
        haystack.push(' ');
        haystack.push_str(&query_keys_json);
    }
    drop(selector_rows);

    let mut rows = run_turso_operation_with_lock_retry(
        || async {
            connection
                .query(
                    "SELECT owner_path, language_id, provider_id, source_kind, line_count, query_keys_json
                     FROM asp_source_index_owner
                     WHERE generation_id = ?1
                     ORDER BY owner_path",
                    (generation_id.as_str(),),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to query Turso source-index owners",
    )
    .await?;

    let mut candidates = Vec::<(usize, ClientDbSourceIndexCandidate)>::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso source-index owner row: {error}"))?
    {
        let path = row
            .get::<String>(0)
            .map_err(|error| format!("failed to read Turso source-index owner path: {error}"))?;
        let row_language_id = row.get::<Option<String>>(1).map_err(|error| {
            format!("failed to read Turso source-index owner language id: {error}")
        })?;
        if let Some(language_id) = language_id
            && row_language_id.as_deref() != Some(language_id.as_str())
        {
            continue;
        }
        let provider_id = row.get::<Option<String>>(2).map_err(|error| {
            format!("failed to read Turso source-index owner provider id: {error}")
        })?;
        let source_kind = row.get::<String>(3).map_err(|error| {
            format!("failed to read Turso source-index owner source kind: {error}")
        })?;
        let line_count = row
            .get::<Option<i64>>(4)
            .map_err(|error| format!("failed to read Turso source-index line count: {error}"))?
            .and_then(|value| u32::try_from(value).ok());
        let query_keys_json = row
            .get::<String>(5)
            .map_err(|error| format!("failed to read Turso source-index query keys: {error}"))?;
        let query_keys = serde_json::from_str::<Vec<String>>(&query_keys_json)
            .map_err(|error| format!("failed to decode Turso source-index query keys: {error}"))?;
        let selector_haystack = selector_haystacks
            .get(&path)
            .map(String::as_str)
            .unwrap_or_default();
        let match_score = source_index_structured_candidate_score(
            &path,
            row_language_id.as_deref(),
            provider_id.as_deref(),
            &source_kind,
            &query_keys,
            selector_haystack,
            terms,
        );
        if match_score == 0 {
            continue;
        }
        let language_id = row_language_id.map(LanguageId::from);
        let provider_id = provider_id.map(ProviderId::from);
        if candidates.iter().any(|(_, candidate)| {
            candidate.path == path
                && candidate.language_id == language_id
                && candidate.provider_id == provider_id
        }) {
            continue;
        }
        let selector_proof = selector_proofs.get(&path).cloned();
        candidates.push((
            match_score,
            ClientDbSourceIndexCandidate {
                path,
                language_id,
                provider_id,
                source_kind: ClientDbSourceIndexSourceKind::Other("turso-source-index".to_string()),
                line_count,
                query_keys,
                selector_proof,
            },
        ));
    }
    candidates.sort_by(|(left_score, left), (right_score, right)| {
        right_score
            .cmp(left_score)
            .then_with(|| left.path.cmp(&right.path))
    });
    Ok(candidates
        .into_iter()
        .map(|(_, candidate)| candidate)
        .take(limit as usize)
        .collect())
}

fn source_index_structured_candidate_score(
    path: &str,
    language_id: Option<&str>,
    provider_id: Option<&str>,
    source_kind: &str,
    query_keys: &[String],
    selector_haystack: &str,
    terms: &[String],
) -> usize {
    if terms.is_empty() {
        return 1;
    }
    let mut haystack = String::new();
    haystack.push_str(path);
    haystack.push(' ');
    if let Some(language_id) = language_id {
        haystack.push_str(language_id);
        haystack.push(' ');
    }
    if let Some(provider_id) = provider_id {
        haystack.push_str(provider_id);
        haystack.push(' ');
    }
    haystack.push_str(source_kind);
    haystack.push(' ');
    for query_key in query_keys {
        haystack.push_str(query_key);
        haystack.push(' ');
    }
    haystack.push_str(selector_haystack);
    let haystack = haystack.to_lowercase();
    terms.iter().filter(|term| haystack.contains(*term)).count()
}

fn source_index_read_model_terms(query: &str) -> Vec<String> {
    query
        .split(|character: char| {
            !character.is_alphanumeric() && character != '_' && character != '-'
        })
        .filter(|term| !term.is_empty())
        .map(|term| term.to_ascii_lowercase())
        .collect()
}
