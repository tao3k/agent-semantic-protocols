//! Source-index and structural-index DB Engine facade methods.

use std::collections::BTreeSet;
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
use super::turso::{
    connect_turso_client_db_read_only, turso_table_column_exists, turso_table_exists,
};
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
    /// Read one owner's projection readiness and selector nodes from read-only Turso state.
    pub fn lookup_graph_owner_read_model_from_project(
        project_root: &Path,
        owner_path: &str,
        language_id: Option<&LanguageId>,
        limit: u32,
    ) -> Result<crate::engine::turso_evidence_graph::TursoClientDbGraphOwnerReadModel, String> {
        let client_dir = project_client_cache_dir_read_only(project_root)?;
        let db_path = Self::turso_path_for_client_dir(client_dir);
        let owner_path = owner_path.to_string();
        let language_id = language_id.cloned();
        block_on_db_engine_async(async move {
            crate::engine::turso_evidence_graph::lookup_turso_graph_owner_read_model(
                &db_path,
                &owner_path,
                language_id.as_ref().map(LanguageId::as_str),
                limit,
            )
            .await
        })
    }

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

    /// Read parser-owned selector nodes for one owner from a project's read-only DB state.
    pub fn lookup_graph_owner_selectors_from_project(
        project_root: &Path,
        owner_path: &str,
        language_id: Option<&LanguageId>,
        limit: u32,
    ) -> Result<Vec<super::TursoClientDbGraphEntity>, String> {
        let client_dir = project_client_cache_dir_read_only(project_root)?;
        let db_path = Self::turso_path_for_client_dir(client_dir);
        let owner_path = owner_path.to_string();
        let language_id = language_id.cloned();
        block_on_db_engine_async(async move {
            crate::engine::turso_evidence_graph::lookup_turso_graph_owner_selectors(
                &db_path,
                &owner_path,
                language_id.as_ref().map(LanguageId::as_str),
                limit,
            )
            .await
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
        let lookup_scope = TursoSourceIndexLookupScope {
            project_root: request
                .indexed_project_root
                .canonicalize()
                .unwrap_or_else(|_| request.indexed_project_root.to_path_buf())
                .display()
                .to_string(),
            schema_id: crate::CLIENT_DB_SOURCE_INDEX_SCHEMA_ID.to_string(),
            schema_version: crate::CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION.to_string(),
        };
        let language_id = request.language_id.cloned();
        let limit = request.limit;
        block_on_db_engine_async(async move {
            lookup_source_index_read_model_at_path(
                db_path,
                Some(lookup_scope),
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
            None,
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
            None,
            query,
            language_id,
            limit,
        )
        .await
    }

    /// Read parser-owned selector nodes for one owner from an isolated client directory.
    pub async fn lookup_graph_owner_selectors_from_client_dir(
        client_dir: impl AsRef<Path>,
        owner_path: &str,
        language_id: Option<&LanguageId>,
        limit: u32,
    ) -> Result<Vec<super::TursoClientDbGraphEntity>, String> {
        crate::engine::turso_evidence_graph::lookup_turso_graph_owner_selectors(
            &Self::turso_path_for_client_dir(client_dir),
            owner_path,
            language_id.map(LanguageId::as_str),
            limit,
        )
        .await
    }

    /// Read one owner's projection readiness and selector nodes from an isolated client directory.
    pub async fn lookup_graph_owner_read_model_from_client_dir(
        client_dir: impl AsRef<Path>,
        owner_path: &str,
        language_id: Option<&LanguageId>,
        limit: u32,
    ) -> Result<crate::engine::turso_evidence_graph::TursoClientDbGraphOwnerReadModel, String> {
        crate::engine::turso_evidence_graph::lookup_turso_graph_owner_read_model(
            &Self::turso_path_for_client_dir(client_dir),
            owner_path,
            language_id.map(LanguageId::as_str),
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
        let graph_report = super::persist_turso_evidence_graph(self.db_path(), &graph).await?;
        let search_document_count = refresh.owner_count as usize;
        Ok(source_index_read_model_report(
            graph_report,
            search_document_count,
        ))
    }

    /// Persist one parser-owned language projection through the Turso read model.
    pub async fn persist_language_projection_read_model(
        &self,
        import: &ClientDbSourceIndexImport,
        projection: &crate::ClientDbLanguageProjection,
    ) -> Result<ClientDbEngineSourceIndexReadModelReport, String> {
        persist_language_projection_read_model_at_path(self.db_path(), import, projection).await
    }

    /// Persist one parser-owned language projection through an isolated client directory.
    pub fn persist_language_projection_read_model_from_client_dir(
        client_dir: impl AsRef<Path>,
        import: &ClientDbSourceIndexImport,
        projection: &crate::ClientDbLanguageProjection,
    ) -> Result<ClientDbEngineSourceIndexReadModelReport, String> {
        let db_path = Self::turso_path_for_client_dir(client_dir);
        let import = import.clone();
        let projection = projection.clone();
        block_on_db_engine_async(async move {
            persist_language_projection_read_model_at_path(&db_path, &import, &projection).await
        })
    }

    /// Persist stable structural-index graph facts through the active DB Engine backend.
    pub async fn persist_structural_index_read_model(
        &self,
        import: &ClientDbStructuralIndexImport,
    ) -> Result<ClientDbEngineStructuralIndexReadModelReport, String> {
        persist_structural_index_read_model_at_path(self.db_path(), import).await
    }
}

async fn persist_language_projection_read_model_at_path(
    db_path: &Path,
    import: &ClientDbSourceIndexImport,
    projection: &crate::ClientDbLanguageProjection,
) -> Result<ClientDbEngineSourceIndexReadModelReport, String> {
    let trace_started = std::time::Instant::now();
    crate::engine::turso_bootstrap::bootstrap_turso_client_db(db_path).await?;
    let refresh = refresh_turso_source_index_import(
        db_path,
        ClientDbSourceIndexRefreshRequest {
            import: import.clone(),
            file_count: import.file_hashes.len().min(u32::MAX as usize) as u32,
        },
    )
    .await?;
    db_engine_trace("language-projection-source-index-refreshed", trace_started);
    let graph = crate::source_index::language_projection::language_projection_evidence_graph(
        import, projection,
    )?;
    let graph_report = super::persist_turso_evidence_graph(db_path, &graph).await?;
    db_engine_trace(
        "language-projection-evidence-graph-persisted",
        trace_started,
    );
    Ok(source_index_read_model_report(
        graph_report,
        refresh.owner_count as usize,
    ))
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
    requested_scope: Option<TursoSourceIndexLookupScope>,
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
    let _source_index_read_guard = match super::turso_source_index::turso_source_index_access_lock()
        .clone()
        .try_read_owned()
    {
        Ok(guard) => guard,
        Err(_) => return Ok(source_index_busy_lookup_result(db_path)),
    };
    let terms = source_index_read_model_terms(query);
    let connection = match connect_turso_client_db_read_only(&db_path).await {
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
        if turso_source_index_precanonical_storage_exists(&connection).await? {
            return Ok(source_index_lookup_result(
                db_path,
                ClientDbSourceIndexLookupState::ColdRequired,
                Vec::new(),
            ));
        }
        return Ok(source_index_lookup_result(
            db_path,
            ClientDbSourceIndexLookupState::EmptyIndex,
            Vec::new(),
        ));
    }
    match turso_source_index_lookup_schema_current(&connection).await {
        Ok(true) => {}
        Ok(false) => {
            return Ok(source_index_lookup_result(
                db_path,
                ClientDbSourceIndexLookupState::ColdRequired,
                Vec::new(),
            ));
        }
        Err(error) if is_turso_lock_error(&error) => {
            return Ok(source_index_busy_lookup_result(db_path));
        }
        Err(error) => return Err(error),
    }
    let scope = match resolve_turso_source_index_lookup_scope(&connection, requested_scope).await {
        Ok(Some(scope)) => scope,
        Ok(None) => {
            return Ok(source_index_lookup_result(
                db_path,
                ClientDbSourceIndexLookupState::EmptyIndex,
                Vec::new(),
            ));
        }
        Err(error) if is_turso_lock_error(&error) => {
            return Ok(source_index_busy_lookup_result(db_path));
        }
        Err(error) => return Err(error),
    };
    let candidates = match query_turso_source_index_candidates_with_connection(
        &connection,
        &scope,
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
    let owner_rows_exist = match turso_source_index_owner_rows_exist(&connection, &scope).await {
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
        "asp_source_index_scope_v1",
        "asp_source_index_owner_v1",
        "asp_source_index_layout_v1",
    ] {
        if !turso_table_exists(connection, table_name).await? {
            return Ok(false);
        }
    }
    Ok(true)
}

async fn turso_source_index_precanonical_storage_exists(
    connection: &turso::Connection,
) -> Result<bool, String> {
    turso_table_exists(connection, "asp_source_index_generation").await
}

async fn turso_source_index_lookup_schema_current(
    connection: &turso::Connection,
) -> Result<bool, String> {
    if !turso_table_exists(connection, "asp_source_index_token_owner_v1").await? {
        return Ok(false);
    }
    for column in [
        "file_hash",
        "language_id",
        "provider_id",
        "source_kind",
        "line_count",
        "query_keys_json",
        "selector_facts_json",
        "selector_count",
    ] {
        if !turso_table_column_exists(connection, "asp_source_index_owner_v1", column).await? {
            return Ok(false);
        }
    }
    Ok(true)
}

async fn turso_source_index_owner_rows_exist(
    connection: &turso::Connection,
    scope: &TursoSourceIndexLookupScope,
) -> Result<bool, String> {
    let mut rows = run_turso_operation_with_lock_retry(
        || async {
            connection
                .query(
                    "SELECT owner_path
                     FROM asp_source_index_owner_v1
                     WHERE project_root = ?1
                       AND schema_id = ?2
                       AND schema_version = ?3
                     LIMIT 1",
                    (
                        scope.project_root.as_str(),
                        scope.schema_id.as_str(),
                        scope.schema_version.as_str(),
                    ),
                )
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

#[derive(Clone)]
struct TursoSourceIndexLookupScope {
    project_root: String,
    schema_id: String,
    schema_version: String,
}

#[derive(serde::Deserialize)]
struct TursoSourceIndexCanonicalSelectorFact {
    selector_id: String,
    symbol: Option<String>,
    kind: Option<String>,
    source: String,
    payload_kind: Option<String>,
    payload_bounded: bool,
    query_keys: Vec<String>,
}

fn decode_turso_source_index_canonical_selectors(
    selector_facts_json: &str,
) -> Result<
    (
        String,
        Option<String>,
        Option<String>,
        Option<ClientDbSourceIndexSelectorPayloadProof>,
    ),
    String,
> {
    let selector_facts =
        serde_json::from_str::<Vec<TursoSourceIndexCanonicalSelectorFact>>(selector_facts_json)
            .map_err(|error| {
                format!("failed to decode Turso source-index canonical selectors: {error}")
            })?;
    let mut haystack = String::new();
    let mut selector_symbol = None;
    let mut selector_kind = None;
    let mut selector_proof = None;
    for selector in selector_facts {
        if selector_proof.is_none()
            && let Some(payload_kind) = selector
                .payload_kind
                .filter(|value| !value.trim().is_empty())
        {
            selector_symbol = selector.symbol.clone();
            selector_kind = selector.kind.clone();
            selector_proof = Some(ClientDbSourceIndexSelectorPayloadProof {
                structural_selector: selector.selector_id.clone(),
                payload_kind,
                bounded: selector.payload_bounded,
            });
        }
        haystack.push(' ');
        haystack.push_str(&selector.selector_id);
        haystack.push(' ');
        haystack.push_str(selector.symbol.as_deref().unwrap_or_default());
        haystack.push(' ');
        haystack.push_str(selector.kind.as_deref().unwrap_or_default());
        haystack.push(' ');
        haystack.push_str(&selector.source);
        haystack.push(' ');
        haystack.push_str(
            &serde_json::to_string(&selector.query_keys).map_err(|error| {
                format!("failed to encode Turso source-index canonical selector keys: {error}")
            })?,
        );
    }
    Ok((haystack, selector_symbol, selector_kind, selector_proof))
}

async fn resolve_turso_source_index_lookup_scope(
    connection: &turso::Connection,
    requested_scope: Option<TursoSourceIndexLookupScope>,
) -> Result<Option<TursoSourceIndexLookupScope>, String> {
    let mut rows = match requested_scope {
        Some(scope) => {
            run_turso_operation_with_lock_retry(
                || async {
                    connection
                        .query(
                            "SELECT project_root, schema_id, schema_version
                         FROM asp_source_index_scope_v1
                         WHERE project_root = ?1
                           AND schema_id = ?2
                           AND schema_version = ?3
                         LIMIT 1",
                            (
                                scope.project_root.as_str(),
                                scope.schema_id.as_str(),
                                scope.schema_version.as_str(),
                            ),
                        )
                        .await
                        .map_err(|error| error.to_string())
                },
                "failed to resolve Turso source-index scope",
            )
            .await?
        }
        None => {
            run_turso_operation_with_lock_retry(
                || async {
                    connection
                        .query(
                            "SELECT project_root, schema_id, schema_version
                         FROM asp_source_index_scope_v1
                         ORDER BY updated_at_ms DESC
                         LIMIT 2",
                            (),
                        )
                        .await
                        .map_err(|error| error.to_string())
                },
                "failed to resolve unscoped Turso source-index scope",
            )
            .await?
        }
    };
    let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso source-index scope: {error}"))?
    else {
        return Ok(None);
    };
    let scope = TursoSourceIndexLookupScope {
        project_root: row
            .get::<String>(0)
            .map_err(|error| format!("failed to read Turso source-index project root: {error}"))?,
        schema_id: row
            .get::<String>(1)
            .map_err(|error| format!("failed to read Turso source-index schema id: {error}"))?,
        schema_version: row.get::<String>(2).map_err(|error| {
            format!("failed to read Turso source-index schema version: {error}")
        })?,
    };
    if rows
        .next()
        .await
        .map_err(|error| format!("failed to verify Turso source-index scope: {error}"))?
        .is_some()
    {
        return Err(
            "unscoped Turso source-index lookup is ambiguous; provide the indexed project root"
                .to_string(),
        );
    }
    Ok(Some(scope))
}

async fn query_turso_source_index_snapshot_candidates_with_connection(
    connection: &turso::Connection,
    scope: &TursoSourceIndexLookupScope,
    query: &str,
    language_id: Option<&LanguageId>,
    limit: u32,
    terms: &[String],
) -> Result<Vec<ClientDbSourceIndexCandidate>, String> {
    if limit == 0 || query.trim().is_empty() {
        return Ok(Vec::new());
    }
    let term_tokens_json = serde_json::to_string(terms)
        .map_err(|error| format!("failed to encode Turso source-index query terms: {error}"))?;
    if terms.is_empty() {
        return Ok(Vec::new());
    }
    // Each requested token probes the `(scope, token, owner_path)` primary key
    // directly. Avoid a Turso group/order aggregate over high-fanout postings;
    // structured scoring below fuses the bounded per-token owner windows.
    let candidate_limit = i64::from(limit);
    if std::env::var_os("ASP_SOURCE_INDEX_TRACE").is_some() {
        trace_turso_source_index_posting_projection(
            connection,
            scope,
            term_tokens_json.as_str(),
            terms.len(),
        )
        .await?;
    }
    let posting_lookup_started_at = std::time::Instant::now();
    let mut fetched_owner_count = 0;
    let mut seen_owner_paths = BTreeSet::new();
    let mut candidates = Vec::<(usize, ClientDbSourceIndexCandidate)>::new();
    for term in terms {
        let mut rows = run_turso_operation_with_lock_retry(
            || async {
                connection
                    .query(
                        "SELECT owner.owner_path,
                                owner.language_id,
                                owner.provider_id,
                                owner.source_kind,
                                owner.line_count,
                                owner.query_keys_json,
                                owner.selector_facts_json
                         FROM asp_source_index_token_owner_v1 AS indexed
                         JOIN asp_source_index_owner_v1 AS owner
                           ON owner.project_root = indexed.project_root
                          AND owner.schema_id = indexed.schema_id
                          AND owner.schema_version = indexed.schema_version
                          AND owner.owner_path = indexed.owner_path
                         WHERE indexed.project_root = ?1
                           AND indexed.schema_id = ?2
                           AND indexed.schema_version = ?3
                           AND indexed.token = ?4
                           AND (?5 IS NULL OR owner.language_id = ?5)
                         ORDER BY indexed.owner_path
                         LIMIT ?6",
                        (
                            scope.project_root.as_str(),
                            scope.schema_id.as_str(),
                            scope.schema_version.as_str(),
                            term.as_str(),
                            language_id.map(|value| value.as_str()),
                            candidate_limit,
                        ),
                    )
                    .await
                    .map_err(|error| error.to_string())
            },
            "failed to query Turso source-index token postings",
        )
        .await?;
        while let Some(row) = rows
            .next()
            .await
            .map_err(|error| format!("failed to read Turso source-index snapshot owner: {error}"))?
        {
            fetched_owner_count += 1;
            let path = row.get::<String>(0).map_err(|error| {
                format!("failed to read Turso source-index owner path: {error}")
            })?;
            if !seen_owner_paths.insert(path.clone()) {
                continue;
            }
            let row_language_id = row.get::<Option<String>>(1).map_err(|error| {
                format!("failed to read Turso source-index owner language id: {error}")
            })?;
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
            let query_keys_json = row.get::<String>(5).map_err(|error| {
                format!("failed to read Turso source-index query keys: {error}")
            })?;
            let selector_facts_json = row.get::<String>(6).map_err(|error| {
                format!("failed to read Turso source-index canonical selectors: {error}")
            })?;
            let query_keys =
                serde_json::from_str::<Vec<String>>(&query_keys_json).map_err(|error| {
                    format!("failed to decode Turso source-index query keys: {error}")
                })?;
            let (selector_haystack, selector_symbol, selector_kind, selector_proof) =
                decode_turso_source_index_canonical_selectors(&selector_facts_json)?;
            let match_score = source_index_structured_candidate_score(
                &path,
                row_language_id.as_deref(),
                provider_id.as_deref(),
                &source_kind,
                &query_keys,
                &selector_haystack,
                terms,
            );
            if match_score == 0 {
                continue;
            }
            candidates.push((
                match_score,
                ClientDbSourceIndexCandidate {
                    path,
                    language_id: row_language_id.map(LanguageId::from),
                    provider_id: provider_id.map(ProviderId::from),
                    source_kind: ClientDbSourceIndexSourceKind::Other(
                        "turso-source-index".to_string(),
                    ),
                    line_count,
                    query_keys,
                    selector_symbol,
                    selector_kind,
                    selector_proof,
                },
            ));
        }
    }
    candidates.sort_by(|(left_score, left), (right_score, right)| {
        right_score
            .cmp(left_score)
            .then_with(|| left.path.cmp(&right.path))
    });
    if std::env::var_os("ASP_SOURCE_INDEX_TRACE").is_some() {
        eprintln!(
            "[source-index-read-trace] stage=token-candidates fetchedOwners={fetched_owner_count} rankedOwners={} lookupMs={}",
            candidates.len(),
            posting_lookup_started_at.elapsed().as_millis(),
        );
    }
    Ok(candidates
        .into_iter()
        .map(|(_, candidate)| candidate)
        .take(limit as usize)
        .collect())
}

async fn trace_turso_source_index_posting_projection(
    connection: &turso::Connection,
    scope: &TursoSourceIndexLookupScope,
    term_tokens_json: &str,
    requested_term_count: usize,
) -> Result<(), String> {
    let mut rows = connection
        .query(
            "WITH requested_terms AS (
                SELECT DISTINCT lower(value) AS token
                FROM json_each(?4)
             )
             SELECT COUNT(*),
                    COUNT(*)
             FROM asp_source_index_token_owner_v1 AS indexed
             JOIN requested_terms
               ON requested_terms.token = indexed.token
             WHERE indexed.project_root = ?1
               AND indexed.schema_id = ?2
               AND indexed.schema_version = ?3",
            (
                scope.project_root.as_str(),
                scope.schema_id.as_str(),
                scope.schema_version.as_str(),
                term_tokens_json,
            ),
        )
        .await
        .map_err(|error| format!("failed to trace Turso source-index token projection: {error}"))?;
    let Some(row) = rows.next().await.map_err(|error| {
        format!("failed to read Turso source-index token projection trace: {error}")
    })?
    else {
        return Ok(());
    };
    let token_count = row
        .get::<i64>(0)
        .map_err(|error| format!("failed to decode Turso source-index token trace: {error}"))?;
    let owner_count = row
        .get::<i64>(1)
        .map_err(|error| format!("failed to decode Turso source-index owner trace: {error}"))?;
    eprintln!(
        "[source-index-read-trace] stage=posting-lookup requestedTerms={requested_term_count} matchedTokens={token_count} matchedPostings={owner_count}"
    );
    Ok(())
}

async fn query_turso_source_index_candidates_with_connection(
    connection: &turso::Connection,
    scope: &TursoSourceIndexLookupScope,
    query: &str,
    language_id: Option<&LanguageId>,
    limit: u32,
    terms: &[String],
) -> Result<Vec<ClientDbSourceIndexCandidate>, String> {
    query_turso_source_index_snapshot_candidates_with_connection(
        connection,
        scope,
        query,
        language_id,
        limit,
        terms,
    )
    .await
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
