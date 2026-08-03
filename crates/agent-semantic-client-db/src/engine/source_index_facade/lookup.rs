//! Read-only source-index and graph-owner lookup methods.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use agent_semantic_client_core::project_client_cache_dir_read_only;

use agent_semantic_client_core::{LanguageId, state_core::TURSO_BACKEND};

use crate::engine::facade::{
    ClientDbEngine, ClientDbEngineReadSession, ClientDbEngineSourceIndexQueryCacheKey,
    block_on_db_engine_async,
};
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
use crate::engine::turso_statement::run_turso_operation;
use crate::source_index::{
    ClientDbSourceIndexClientDirLookupRequest, ClientDbSourceIndexLookupResult,
    ClientDbSourceIndexLookupState, ClientDbSourceIndexProjectLookupRequest,
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
            expected_snapshot_root: request.expected_snapshot_root,
            expected_index_artifact_digest: request.expected_index_artifact_digest,
            live_facts: request.live_facts,
        })
    }

    /// Lookup source-index candidates through the active Turso read model.
    pub fn lookup_source_index_from_client_dir(
        request: ClientDbSourceIndexClientDirLookupRequest<'_>,
    ) -> Result<ClientDbSourceIndexLookupResult, String> {
        let ClientDbSourceIndexClientDirLookupRequest {
            client_dir,
            indexed_project_root,
            language_id,
            query_keys,
            limit,
            expected_snapshot_root,
            expected_index_artifact_digest,
            live_facts,
        } = request;
        let query = query_keys
            .iter()
            .map(|key| key.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let db_path = Self::turso_path_for_client_dir(client_dir);
        let lookup_scope = TursoSourceIndexLookupRequestScope {
            project_root: indexed_project_root
                .canonicalize()
                .unwrap_or_else(|_| indexed_project_root.to_path_buf())
                .display()
                .to_string(),
            schema_id: crate::CLIENT_DB_SOURCE_INDEX_SCHEMA_ID.to_string(),
            schema_version: crate::CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION.to_string(),
        };
        let language_id = language_id.cloned();
        let expected_snapshot_root = expected_snapshot_root.to_string();
        let expected_index_artifact_digest = expected_index_artifact_digest.to_string();
        if let Some(result) =
            lookup_live_source_index_read_model(LiveSourceIndexReadModelRequest {
                db_path: db_path.as_path(),
                requested_scope: Some(&lookup_scope),
                live_facts,
                query: query.as_str(),
                language_id: language_id.as_ref(),
                limit,
                expected_snapshot_root: expected_snapshot_root.as_str(),
                expected_index_artifact_digest: expected_index_artifact_digest.as_str(),
            })?
        {
            return Ok(result);
        }
        block_on_db_engine_async(async move {
            lookup_source_index_read_model_at_path(
                db_path,
                Some(lookup_scope),
                query.as_str(),
                language_id.as_ref(),
                limit,
                expected_snapshot_root.as_str(),
                expected_index_artifact_digest.as_str(),
                None,
                None,
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
            None,
            None,
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
            None,
            None,
        )
        .await
    }
}

impl ClientDbEngineReadSession {
    /// Lookup source-index candidates through this already-open Turso 0.7 read session.
    pub async fn lookup_source_index_read_model(
        &self,
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
        let cache_key = ClientDbEngineSourceIndexQueryCacheKey {
            snapshot_root: source_snapshot.root_digest.clone(),
            artifact_digest: expected_index_artifact_digest.clone(),
            query: query.to_string(),
            language_id: language_id.map(|language_id| language_id.as_str().to_string()),
            limit,
        };
        let mut cache_hasher = std::collections::hash_map::DefaultHasher::new();
        std::hash::Hash::hash(&cache_key, &mut cache_hasher);
        let cache_shard_index =
            std::hash::Hasher::finish(&cache_hasher) as usize % self.source_index_query_cache.len();
        if let Some(result) = self.source_index_query_cache[cache_shard_index]
            .read()
            .get(&cache_key)
            .cloned()
        {
            return Ok(result);
        }
        let result = lookup_source_index_read_model_at_path(
            self.turso_db_path.clone(),
            None,
            query,
            language_id,
            limit,
            &source_snapshot.root_digest,
            &expected_index_artifact_digest,
            Some(self.turso_connection.clone()),
            Some(self.source_index_scope_cache.as_ref()),
        )
        .await?;
        if matches!(
            result.state,
            ClientDbSourceIndexLookupState::Hit
                | ClientDbSourceIndexLookupState::Miss
                | ClientDbSourceIndexLookupState::EmptyIndex
        ) {
            let mut shard = self.source_index_query_cache[cache_shard_index].write();
            if shard.len() >= 64 {
                shard.clear();
            }
            shard.insert(cache_key, result.clone());
        }
        Ok(result)
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

fn is_turso_source_index_schema_missing_error(error: &str) -> bool {
    let normalized = error.to_ascii_lowercase();
    normalized.contains("no such table") || normalized.contains("no such column")
}

struct LiveSourceIndexReadModelRequest<'a> {
    db_path: &'a std::path::Path,
    requested_scope: Option<&'a TursoSourceIndexLookupRequestScope>,
    live_facts: Option<crate::source_index::ClientDbLiveSourceIndexFacts<'a>>,
    query: &'a str,
    language_id: Option<&'a LanguageId>,
    limit: u32,
    expected_snapshot_root: &'a str,
    expected_index_artifact_digest: &'a str,
}

fn lookup_live_source_index_read_model(
    request: LiveSourceIndexReadModelRequest<'_>,
) -> Result<Option<ClientDbSourceIndexLookupResult>, String> {
    let LiveSourceIndexReadModelRequest {
        db_path,
        requested_scope,
        live_facts,
        query,
        language_id,
        limit,
        expected_snapshot_root,
        expected_index_artifact_digest,
    } = request;
    let Some(live_facts) = live_facts else {
        return Ok(None);
    };
    if limit == 0 {
        return Ok(Some(source_index_lookup_result(
            db_path.to_path_buf(),
            ClientDbSourceIndexLookupState::Miss,
            Vec::new(),
        )));
    }
    let terms = source_index_read_model_terms(query)?;
    let live_artifact_digest =
        crate::client_db_source_index_artifact_digest(live_facts.source_snapshot);
    let live_generation =
        crate::client_db_source_index_generation_id_for_snapshot(live_facts.source_snapshot);
    let live_project_root = live_facts
        .import
        .project_root
        .canonicalize()
        .unwrap_or_else(|_| live_facts.import.project_root.clone())
        .display()
        .to_string();
    let live_scope_matches = requested_scope.is_none_or(|scope| {
        scope.project_root == live_project_root
            && scope.schema_id == live_facts.import.schema_id.as_str()
            && scope.schema_version == live_facts.import.schema_version.as_str()
    });
    if live_facts.source_snapshot.root_digest.as_str() != expected_snapshot_root
        || live_artifact_digest != expected_index_artifact_digest
        || live_generation != live_facts.import.generation_id
        || !live_scope_matches
    {
        return Ok(None);
    }
    let candidates =
        crate::engine::source_index_candidate_projection::rank_live_source_index_candidates(
            live_facts.import,
            &terms,
            language_id.map(LanguageId::as_str),
            limit,
        );
    let state = if !candidates.is_empty() {
        ClientDbSourceIndexLookupState::Hit
    } else if live_facts.import.owners.is_empty() {
        ClientDbSourceIndexLookupState::EmptyIndex
    } else {
        ClientDbSourceIndexLookupState::Miss
    };
    Ok(Some(source_index_lookup_result(
        db_path.to_path_buf(),
        state,
        candidates,
    )))
}

async fn lookup_source_index_read_model_at_path(
    db_path: PathBuf,
    requested_scope: Option<TursoSourceIndexLookupRequestScope>,
    query: &str,
    language_id: Option<&LanguageId>,
    limit: u32,
    expected_snapshot_root: &str,
    expected_index_artifact_digest: &str,
    resident_connection: Option<Arc<turso::Connection>>,
    resident_scope_cache: Option<
        &tokio::sync::RwLock<
            Option<(
                String,
                String,
                TursoSourceIndexLookupScope,
                agent_semantic_content_identity::SourceSnapshotEvidence,
            )>,
        >,
    >,
) -> Result<ClientDbSourceIndexLookupResult, String> {
    if limit == 0 {
        return Ok(source_index_lookup_result(
            db_path,
            ClientDbSourceIndexLookupState::Miss,
            Vec::new(),
        ));
    }
    let terms = source_index_read_model_terms(query)?;
    if !crate::engine::turso::turso_client_db_exists(&db_path) {
        return Ok(source_index_lookup_result(
            db_path,
            ClientDbSourceIndexLookupState::MissingDb,
            Vec::new(),
        ));
    }
    let _source_index_read_guard =
        crate::engine::turso_source_index::turso_source_index_access_lock(&db_path)
            .read_owned()
            .await;
    let connection = match resident_connection {
        Some(connection) => connection,
        None => match connect_turso_client_db_read_only(&db_path).await {
            Ok(connection) => Arc::new(connection),
            Err(error) if error.to_ascii_lowercase().contains("entity not found") => {
                return Ok(source_index_lookup_result(
                    db_path,
                    ClientDbSourceIndexLookupState::MissingDb,
                    Vec::new(),
                ));
            }
            Err(error) => return Err(error),
        },
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
                Err(error) => return Err(error),
            };
        Some(candidates)
    } else {
        None
    };
    let cached_scope = match resident_scope_cache {
        Some(cache) => cache
            .read()
            .await
            .as_ref()
            .filter(|(snapshot_root, artifact_digest, _, _)| {
                snapshot_root == expected_snapshot_root
                    && artifact_digest == expected_index_artifact_digest
            })
            .map(|(_, _, scope, snapshot)| (scope.clone(), snapshot.clone())),
        None => None,
    };
    let scope_was_cached = cached_scope.is_some();
    let (scope, cached_snapshot) = match cached_scope {
        Some((scope, snapshot)) => (scope, Some(snapshot)),
        None => match resolve_turso_source_index_lookup_scope(&connection, requested_scope.clone())
            .await
        {
            Ok(Some(scope)) => (scope, None),
            Ok(None) => {
                let state = match turso_source_index_lookup_schema_current(
                    &connection,
                    requested_scope.as_ref(),
                )
                .await
                {
                    Ok(true) => ClientDbSourceIndexLookupState::EmptyIndex,
                    Ok(false) => ClientDbSourceIndexLookupState::ColdRequired,
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
            Err(error) => return Err(error),
        },
    };
    let persisted_snapshot = match cached_snapshot {
        Some(snapshot) => snapshot,
        None => {
            let snapshot = match serde_json::from_str::<
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
                        snapshot_root: &snapshot.root_digest,
                        provider_digest: &snapshot.provider_digest,
                        parameters: &[],
                    },
                )
                .value;
            if snapshot.root_digest != expected_snapshot_root
                || persisted_index_artifact_digest != expected_index_artifact_digest
            {
                return Ok(source_index_lookup_result(
                    db_path,
                    ClientDbSourceIndexLookupState::ColdRequired,
                    Vec::new(),
                ));
            }
            snapshot
        }
    };
    if !scope_was_cached && let Some(cache) = resident_scope_cache {
        *cache.write().await = Some((
            expected_snapshot_root.to_string(),
            expected_index_artifact_digest.to_string(),
            scope.clone(),
            persisted_snapshot.clone(),
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
            Err(error) => return Err(error),
        },
    };
    if candidates.is_empty() {
        let owner_rows_exist = match turso_source_index_owner_rows_exist(&connection, &scope).await
        {
            Ok(owner_rows_exist) => owner_rows_exist,
            Err(error) => return Err(error),
        };
        if !owner_rows_exist {
            return Ok(source_index_lookup_result_for_snapshot(
                db_path,
                ClientDbSourceIndexLookupState::EmptyIndex,
                Vec::new(),
                persisted_snapshot,
            ));
        }
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

pub(crate) async fn lookup_source_index_read_model_in_resident_connection(
    db_path: PathBuf,
    connection: Arc<turso::Connection>,
    indexed_project_root: &Path,
    source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
    query: &str,
    language_id: Option<&LanguageId>,
    limit: u32,
) -> Result<ClientDbSourceIndexLookupResult, String> {
    let lookup_scope = TursoSourceIndexLookupRequestScope {
        project_root: indexed_project_root
            .canonicalize()
            .unwrap_or_else(|_| indexed_project_root.to_path_buf())
            .display()
            .to_string(),
        schema_id: crate::CLIENT_DB_SOURCE_INDEX_SCHEMA_ID.to_string(),
        schema_version: crate::CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION.to_string(),
    };
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
        db_path,
        Some(lookup_scope),
        query,
        language_id,
        limit,
        &source_snapshot.root_digest,
        &expected_index_artifact_digest,
        Some(connection),
        None,
    )
    .await
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
    run_turso_operation(
        || async {
            let mut statement = connection
                .prepare_cached(
                    "SELECT owner_path
                     FROM asp_source_index_owner_v1
                     WHERE project_root = ?1
                       AND schema_id = ?2
                       AND schema_version = ?3
                       AND generation_id = ?4
                     LIMIT 1",
                )
                .await
                .map_err(|error| error.to_string())?;
            let mut rows = statement
                .query((
                    scope.project_root.as_str(),
                    scope.schema_id.as_str(),
                    scope.schema_version.as_str(),
                    scope.generation_id.as_str(),
                ))
                .await
                .map_err(|error| error.to_string())?;
            rows.next()
                .await
                .map(|row| row.is_some())
                .map_err(|error| error.to_string())
        },
        "failed to inspect Turso source-index owner rows",
    )
    .await
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
