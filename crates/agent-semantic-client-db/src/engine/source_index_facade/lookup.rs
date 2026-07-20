//! Read-only source-index and graph-owner lookup methods.

use std::path::{Path, PathBuf};

use agent_semantic_client_core::project_client_cache_dir_read_only;
use agent_semantic_content_identity::SourceSnapshotEvidence;

use agent_semantic_client_core::{LanguageId, state_core::TURSO_BACKEND};

use crate::engine::facade::{ClientDbEngine, block_on_db_engine_async};
use crate::engine::source_index_candidate_selection::{
    query_turso_source_index_candidates_with_connection,
    query_turso_source_index_snapshot_candidates_for_scope_with_connection,
    resolve_turso_source_index_lookup_scope,
};
use crate::engine::source_index_candidate_types::{
    TursoSourceIndexCandidateScope, TursoSourceIndexLookupRequestScope, TursoSourceIndexLookupScope,
};
use crate::engine::source_index_query_scoring::source_index_read_model_terms;
use crate::engine::turso::{connect_turso_client_db_read_only, turso_table_exists};
use crate::engine::turso_lock_policy::is_turso_lock_error;
use crate::engine::turso_statement::run_turso_operation_with_lock_retry;
use crate::source_index::{
    ClientDbSourceIndexClientDirLookupRequest, ClientDbSourceIndexLookupResult,
    ClientDbSourceIndexLookupState, ClientDbSourceIndexProjectLookupRequest,
};

impl ClientDbEngine {
    /// Read one owner's projection readiness and selector nodes from read-only Turso state.
    pub fn lookup_graph_owner_read_model_from_project(
        project_root: &Path,
        source_snapshot: &SourceSnapshotEvidence,
        owner_path: &str,
        language_id: Option<&LanguageId>,
        limit: u32,
    ) -> Result<crate::engine::turso_evidence_graph::TursoClientDbGraphOwnerReadModel, String> {
        let client_dir = project_client_cache_dir_read_only(project_root)?;
        let db_path = Self::turso_path_for_client_dir(client_dir);
        let source_snapshot = source_snapshot.clone();
        let owner_path = owner_path.to_string();
        let language_id = language_id.cloned();
        block_on_db_engine_async(async move {
            crate::engine::turso_evidence_graph::lookup_turso_graph_owner_read_model(
                &db_path,
                &source_snapshot,
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
            expected_snapshot_root: request.expected_snapshot_root,
            expected_index_artifact_digest: request.expected_index_artifact_digest,
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
        let lookup_scope = TursoSourceIndexLookupRequestScope {
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
        let expected_snapshot_root = request.expected_snapshot_root.to_string();
        let expected_index_artifact_digest = request.expected_index_artifact_digest.to_string();
        block_on_db_engine_async(async move {
            lookup_source_index_read_model_at_path(
                db_path,
                Some(lookup_scope),
                query.as_str(),
                language_id.as_ref(),
                limit,
                expected_snapshot_root.as_str(),
                expected_index_artifact_digest.as_str(),
            )
            .await
        })
    }

    /// Lookup source-index candidates from the active Turso EvidenceGraph read model.
    pub async fn lookup_source_index_read_model(
        &self,
        source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
        query: &str,
        language_id: Option<&LanguageId>,
        limit: u32,
    ) -> Result<ClientDbSourceIndexLookupResult, String> {
        if self.backend() != crate::engine::ClientDbBackend::Turso {
            return Err(format!(
                "active DB Engine backend is {}, expected {}",
                self.backend().as_str(),
                TURSO_BACKEND
            ));
        }
        let expected_index_artifact_digest =
            agent_semantic_content_identity::hash_derived_artifact_key(
                agent_semantic_content_identity::DerivedArtifactKeyInput {
                    artifact_kind: "source-index",
                    schema_id: "asp.source-index-artifact.v1",
                    snapshot_root: &source_snapshot.root_digest,
                    provider_digest: &source_snapshot.provider_digest,
                    parameters: &[],
                },
            )
            .value;
        lookup_source_index_read_model_at_path(
            self.db_path().to_path_buf(),
            None,
            query,
            language_id,
            limit,
            &source_snapshot.root_digest,
            &expected_index_artifact_digest,
        )
        .await
    }

    /// Lookup source-index candidates from a resolved client directory's Turso read model.
    pub async fn lookup_source_index_read_model_from_client_dir(
        client_dir: impl AsRef<Path>,
        source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
        query: &str,
        language_id: Option<&LanguageId>,
        limit: u32,
    ) -> Result<ClientDbSourceIndexLookupResult, String> {
        let expected_index_artifact_digest =
            agent_semantic_content_identity::hash_derived_artifact_key(
                agent_semantic_content_identity::DerivedArtifactKeyInput {
                    artifact_kind: "source-index",
                    schema_id: "asp.source-index-artifact.v1",
                    snapshot_root: &source_snapshot.root_digest,
                    provider_digest: &source_snapshot.provider_digest,
                    parameters: &[],
                },
            )
            .value;
        lookup_source_index_read_model_at_path(
            Self::turso_path_for_client_dir(client_dir),
            None,
            query,
            language_id,
            limit,
            &source_snapshot.root_digest,
            &expected_index_artifact_digest,
        )
        .await
    }

    /// Read one owner's projection readiness and selector nodes from an isolated client directory.
    pub async fn lookup_graph_owner_read_model_from_client_dir(
        client_dir: impl AsRef<Path>,
        source_snapshot: &SourceSnapshotEvidence,
        owner_path: &str,
        language_id: Option<&LanguageId>,
        limit: u32,
    ) -> Result<crate::engine::turso_evidence_graph::TursoClientDbGraphOwnerReadModel, String> {
        crate::engine::turso_evidence_graph::lookup_turso_graph_owner_read_model(
            &Self::turso_path_for_client_dir(client_dir),
            source_snapshot,
            owner_path,
            language_id.map(LanguageId::as_str),
            limit,
        )
        .await
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
        source_snapshot: None,
        index_artifact_digest: None,
    }
}

fn source_index_lookup_result_for_snapshot(
    db_path: PathBuf,
    state: ClientDbSourceIndexLookupState,
    candidates: Vec<crate::ClientDbSourceIndexCandidate>,
    source_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence,
) -> ClientDbSourceIndexLookupResult {
    let index_artifact_digest = agent_semantic_content_identity::hash_derived_artifact_key(
        agent_semantic_content_identity::DerivedArtifactKeyInput {
            artifact_kind: "source-index",
            schema_id: "asp.source-index-artifact.v1",
            snapshot_root: &source_snapshot.root_digest,
            provider_digest: &source_snapshot.provider_digest,
            parameters: &[],
        },
    )
    .value;
    ClientDbSourceIndexLookupResult {
        db_path,
        state,
        candidates,
        source_snapshot: Some(source_snapshot),
        index_artifact_digest: Some(index_artifact_digest),
    }
}

fn source_index_busy_lookup_result(db_path: PathBuf) -> ClientDbSourceIndexLookupResult {
    source_index_lookup_result(db_path, ClientDbSourceIndexLookupState::Busy, Vec::new())
}

fn is_turso_source_index_schema_missing_error(error: &str) -> bool {
    let normalized = error.to_ascii_lowercase();
    normalized.contains("no such table") || normalized.contains("no such column")
}

async fn lookup_source_index_read_model_at_path(
    db_path: PathBuf,
    requested_scope: Option<TursoSourceIndexLookupRequestScope>,
    query: &str,
    language_id: Option<&LanguageId>,
    limit: u32,
    expected_snapshot_root: &str,
    expected_index_artifact_digest: &str,
) -> Result<ClientDbSourceIndexLookupResult, String> {
    if !crate::engine::turso::turso_client_db_exists(&db_path) {
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
    let _source_index_read_guard =
        match crate::engine::turso_source_index::turso_source_index_access_lock(&db_path)
            .try_read_owned()
        {
            Ok(guard) => guard,
            Err(_) => return Ok(source_index_busy_lookup_result(db_path)),
        };
    let terms = source_index_read_model_terms(query)?;
    let connection = match connect_turso_client_db_read_only(&db_path).await {
        Ok(connection) => connection,
        Err(error) if is_turso_lock_error(&error) => {
            return Ok(source_index_busy_lookup_result(db_path));
        }
        Err(error) if error.to_ascii_lowercase().contains("entity not found") => {
            return Ok(source_index_lookup_result(
                db_path,
                ClientDbSourceIndexLookupState::MissingDb,
                Vec::new(),
            ));
        }
        Err(error) => return Err(error),
    };
    let requested_scope_candidates = if let Some(requested_scope) = requested_scope.as_ref() {
        let candidates =
            match query_turso_source_index_snapshot_candidates_for_scope_with_connection(
                &connection,
                TursoSourceIndexCandidateScope::Requested(requested_scope),
                query,
                language_id,
                limit,
                &terms,
            )
            .await
            {
                Ok(candidates) => candidates,
                Err(error) if is_turso_source_index_schema_missing_error(&error) => Vec::new(),
                Err(error) if is_turso_lock_error(&error) => {
                    return Ok(source_index_busy_lookup_result(db_path));
                }
                Err(error) => return Err(error),
            };
        Some(candidates)
    } else {
        None
    };
    let scope =
        match resolve_turso_source_index_lookup_scope(&connection, requested_scope.clone()).await {
            Ok(Some(scope)) => scope,
            Ok(None) => {
                let state = match turso_source_index_lookup_schema_current(
                    &connection,
                    requested_scope.as_ref(),
                )
                .await
                {
                    Ok(true) => ClientDbSourceIndexLookupState::EmptyIndex,
                    Ok(false) => ClientDbSourceIndexLookupState::ColdRequired,
                    Err(error) if is_turso_lock_error(&error) => {
                        return Ok(source_index_busy_lookup_result(db_path));
                    }
                    Err(error) => return Err(error),
                };
                return Ok(source_index_lookup_result(db_path, state, Vec::new()));
            }
            Err(error) if is_turso_source_index_schema_missing_error(&error) => {
                let state = if turso_source_index_namespace_exists(&connection).await? {
                    ClientDbSourceIndexLookupState::ColdRequired
                } else {
                    ClientDbSourceIndexLookupState::EmptyIndex
                };
                return Ok(source_index_lookup_result(db_path, state, Vec::new()));
            }
            Err(error) if is_turso_lock_error(&error) => {
                return Ok(source_index_busy_lookup_result(db_path));
            }
            Err(error) => return Err(error),
        };
    let persisted_snapshot = match serde_json::from_str::<
        agent_semantic_content_identity::SourceSnapshotEvidence,
    >(&scope.source_snapshot_json)
    {
        Ok(snapshot) => snapshot,
        Err(_) => {
            return Ok(source_index_lookup_result(
                db_path,
                ClientDbSourceIndexLookupState::ColdRequired,
                Vec::new(),
            ));
        }
    };
    let persisted_index_artifact_digest =
        agent_semantic_content_identity::hash_derived_artifact_key(
            agent_semantic_content_identity::DerivedArtifactKeyInput {
                artifact_kind: "source-index",
                schema_id: "asp.source-index-artifact.v1",
                snapshot_root: &persisted_snapshot.root_digest,
                provider_digest: &persisted_snapshot.provider_digest,
                parameters: &[],
            },
        )
        .value;
    if persisted_snapshot.root_digest != expected_snapshot_root
        || persisted_index_artifact_digest != expected_index_artifact_digest
    {
        return Ok(source_index_lookup_result(
            db_path,
            ClientDbSourceIndexLookupState::ColdRequired,
            Vec::new(),
        ));
    }
    let candidates = match requested_scope_candidates {
        Some(candidates) => candidates,
        None => match query_turso_source_index_candidates_with_connection(
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
        },
    };
    let owner_rows_exist = match turso_source_index_owner_rows_exist(&connection, &scope).await {
        Ok(owner_rows_exist) => owner_rows_exist,
        Err(error) if is_turso_lock_error(&error) => {
            return Ok(source_index_busy_lookup_result(db_path));
        }
        Err(error) => return Err(error),
    };
    if candidates.is_empty() && !owner_rows_exist {
        return Ok(source_index_lookup_result_for_snapshot(
            db_path,
            ClientDbSourceIndexLookupState::EmptyIndex,
            Vec::new(),
            persisted_snapshot,
        ));
    }
    let state = if candidates.is_empty() {
        ClientDbSourceIndexLookupState::Miss
    } else {
        ClientDbSourceIndexLookupState::Hit
    };
    Ok(source_index_lookup_result_for_snapshot(
        db_path,
        state,
        candidates,
        persisted_snapshot,
    ))
}

async fn turso_source_index_lookup_schema_current(
    connection: &turso::Connection,
    requested_scope: Option<&TursoSourceIndexLookupRequestScope>,
) -> Result<bool, String> {
    let mut rows =
        match requested_scope {
            Some(scope) => connection
                .query(
                    "SELECT 1
                     FROM asp_source_index_layout_v1
                     WHERE project_root = ?1
                       AND schema_id = ?2
                       AND schema_version = ?3
                       AND term_projection_version = ?4
                       AND token_projection_generation_id <> ''
                     LIMIT 1",
                    (
                        scope.project_root.as_str(),
                        scope.schema_id.as_str(),
                        scope.schema_version.as_str(),
                        crate::engine::turso_source_index::core::TURSO_SOURCE_INDEX_TERM_PROJECTION_VERSION,
                    ),
                )
                .await,
            None => connection
                .query(
                    "SELECT 1
                     FROM asp_source_index_layout_v1
                     WHERE term_projection_version = ?1
                       AND token_projection_generation_id <> ''
                     LIMIT 1",
                    (crate::engine::turso_source_index::core::TURSO_SOURCE_INDEX_TERM_PROJECTION_VERSION,),
                )
                .await,
        }
        .map_err(|error| format!("failed to inspect Turso source-index layout: {error}"))?;
    Ok(rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso source-index layout: {error}"))?
        .is_some())
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
                       AND generation_id = ?4
                     LIMIT 1",
                    (
                        scope.project_root.as_str(),
                        scope.schema_id.as_str(),
                        scope.schema_version.as_str(),
                        scope.generation_id.as_str(),
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
async fn turso_source_index_namespace_exists(
    connection: &turso::Connection,
) -> Result<bool, String> {
    for table in [
        "asp_source_index_scope_v1",
        "asp_source_index_owner_v1",
        "asp_source_index_layout_v1",
        "asp_source_index_token_owner_v1",
    ] {
        if turso_table_exists(connection, table).await? {
            return Ok(true);
        }
    }
    Ok(false)
}
