//! Turso source-index durable adapter.

use std::path::{Path, PathBuf};

use agent_semantic_client_core::{
    ClientCacheFileHash, LanguageId, ProviderId, SemanticSchemaId, SemanticSchemaVersion,
};

use crate::source_index::{
    ClientDbSourceIndexImport, ClientDbSourceIndexRefreshReport, ClientDbSourceIndexRefreshRequest,
    ClientDbSourceIndexScopeFile, ClientDbSourceIndexStats,
};
use crate::types::normalized_project_root;

use crate::engine::{
    turso::connect_turso_client_db,
    turso_statement::{execute_turso_statement, run_turso_operation},
};

use super::contract::TURSO_SOURCE_INDEX_TERM_PROJECTION_VERSION;
use super::selector_identity::turso_source_index_selector_fingerprint;

pub(super) async fn ensure_turso_source_index_schema(
    connection: &turso::Connection,
) -> Result<bool, String> {
    match validate_turso_source_index_schema(connection).await {
        Ok(()) => Ok(false),
        Err(error) if error.contains("no such table") || error.contains("no such column") => {
            reset_turso_source_index_schema(connection).await?;
            super::schema::bootstrap_turso_source_index_schema(connection).await?;
            validate_turso_source_index_schema(connection).await?;
            Ok(true)
        }
        Err(error) => Err(error),
    }
}

async fn validate_turso_source_index_schema(connection: &turso::Connection) -> Result<(), String> {
    run_turso_operation(
        || async {
            connection
                .query(
                    "SELECT project_root, schema_id, schema_version,
                            term_projection_version, token_projection_generation_id
                     FROM asp_source_index_layout_v1
                     LIMIT 1",
                    (),
                )
                .await
                .map_err(|error| error.to_string())?;
            connection
                .query(
                    "SELECT project_root, schema_id, schema_version, generation_id,
                            file_hash, owner_path, language_id, provider_id, source_kind,
                            line_count, query_keys_json, selector_facts_json,
                            term_tokens_json, selector_count
                     FROM asp_source_index_owner_v1
                     LIMIT 1",
                    (),
                )
                .await
                .map_err(|error| error.to_string())?;
            connection
                .query(
                    "SELECT project_root, schema_id, schema_version, generation_id,
                            file_hashes_json, source_snapshot_json, selector_fingerprint,
                            owner_count, selector_count, updated_at_ms
                     FROM asp_source_index_scope_v1
                     LIMIT 1",
                    (),
                )
                .await
                .map_err(|error| error.to_string())?;
            connection
                .query(
                    "SELECT project_root, schema_id, schema_version, generation_id,
                            token, owner_path
                     FROM asp_source_index_token_owner_v1
                     LIMIT 1",
                    (),
                )
                .await
                .map_err(|error| error.to_string())?;
            connection
                .query(
                    "SELECT project_root, schema_id, schema_version, generation_id,
                            owner_path, owner_content_digest, language_id, item_kind,
                            parser_identity_digest, query_pack_digest, item_symbol,
                            scopes_json, structural_selector
                     FROM asp_source_index_selector_v1
                     LIMIT 1",
                    (),
                )
                .await
                .map_err(|error| error.to_string())?;
            Ok(())
        },
        "failed to inspect Turso source-index schema layout",
    )
    .await?;
    Ok(())
}

async fn reset_turso_source_index_schema(connection: &turso::Connection) -> Result<(), String> {
    for table in [
        "asp_source_index_token_owner_v1",
        "asp_source_index_selector_v1",
        "asp_source_index_owner_v1",
        "asp_source_index_layout_v1",
        "asp_source_index_scope_v1",
    ] {
        execute_turso_statement(
            connection,
            &format!("DROP TABLE IF EXISTS {table}"),
            "failed to reset noncanonical Turso source-index schema",
        )
        .await?;
    }
    Ok(())
}

fn source_index_db_trace(stage: &str, started: std::time::Instant) {
    if std::env::var_os("ASP_SOURCE_INDEX_TRACE").is_some() {
        eprintln!(
            "[source-index-db-trace] stage={stage} elapsedMs={}",
            started.elapsed().as_millis()
        );
    }
}

use super::facts::write_turso_source_index_rows;
use super::readiness::turso_source_index_projection_ready;

struct PreparedTursoSourceIndexRefresh {
    file_count: u32,
    import: ClientDbSourceIndexImport,
    writer_import: ClientDbSourceIndexImport,
    project_root: String,
    file_hashes_json: String,
    source_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence,
    source_snapshot_json: String,
    membership_change_set: crate::ClientDbSourceIndexMembershipChangeSet,
}

pub async fn refresh_turso_source_index_import_on_connection(
    connection: &mut turso::Connection,
    request: ClientDbSourceIndexRefreshRequest,
    materialization: &mut crate::runtime_server_workspace::WorkspaceCanonicalMaterialization,
) -> Result<ClientDbSourceIndexRefreshReport, String> {
    let trace_started = std::time::Instant::now();
    let prepared = prepare_turso_source_index_refresh(connection, request, materialization).await?;
    source_index_db_trace("source-index-refresh-prepared", trace_started);
    if let Some(refresh) = reusable_prepared_source_index_generation(
        connection,
        &prepared,
        materialization,
        trace_started,
    )
    .await?
    {
        return Ok(refresh);
    }
    persist_prepared_source_index_refresh(connection, prepared, materialization, trace_started)
        .await
}

async fn prepare_turso_source_index_refresh(
    connection: &turso::Connection,
    request: ClientDbSourceIndexRefreshRequest,
    materialization: &mut crate::runtime_server_workspace::WorkspaceCanonicalMaterialization,
) -> Result<PreparedTursoSourceIndexRefresh, String> {
    let requested_source_snapshot = request.source_snapshot;
    let import = request.import;
    let workspace_snapshot = materialization.workspace_snapshot.clone();
    workspace_snapshot.validate()?;
    if workspace_snapshot.root_digest() != requested_source_snapshot.root_digest {
        return Err(format!(
            "source-index requested snapshot is not the materialized source-byte Merkle root: requested={} materialized={}",
            requested_source_snapshot.root_digest,
            workspace_snapshot.root_digest(),
        ));
    }
    if import.file_hashes.is_empty() {
        return Err("source index import requires file hash evidence".to_owned());
    }
    ensure_turso_source_index_schema(connection).await?;
    let file_hashes_json = serde_json::to_string(&import.file_hashes)
        .map_err(|error| format!("failed to serialize Turso source-index file hashes: {error}"))?;
    let project_root = normalized_project_root(&import.project_root)?;
    let (canonical_import, writer_import, source_snapshot, membership_change_set) =
        prepare_turso_source_index_membership(
            connection,
            &project_root,
            &import,
            &workspace_snapshot,
            &requested_source_snapshot,
            materialization,
        )
        .await?;
    super::membership::validate_source_index_membership_change_set(
        &canonical_import,
        &source_snapshot,
        &membership_change_set,
    )?;
    materialization.finalize_generation_evidence(workspace_snapshot, source_snapshot.clone())?;
    materialization.validate_persisted(materialization.workspace_identity.as_str())?;
    materialization.validate_incremental_source_index_proofs(&import)?;
    let source_snapshot_json = serde_json::to_string(&source_snapshot).map_err(|error| {
        format!("failed to serialize Turso source-index source snapshot evidence: {error}")
    })?;
    Ok(PreparedTursoSourceIndexRefresh {
        file_count: request.file_count,
        import,
        writer_import,
        project_root,
        file_hashes_json,
        source_snapshot,
        source_snapshot_json,
        membership_change_set,
    })
}

async fn prepare_turso_source_index_membership(
    connection: &turso::Connection,
    project_root: &str,
    import: &ClientDbSourceIndexImport,
    workspace_snapshot: &agent_semantic_content_identity::WorkspaceSnapshot,
    requested_source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
    materialization: &crate::runtime_server_workspace::WorkspaceCanonicalMaterialization,
) -> Result<
    (
        ClientDbSourceIndexImport,
        ClientDbSourceIndexImport,
        agent_semantic_content_identity::SourceSnapshotEvidence,
        crate::ClientDbSourceIndexMembershipChangeSet,
    ),
    String,
> {
    let Some(previous) = super::generation_snapshot::load_turso_source_index_generation_snapshot(
        connection,
        project_root,
        import.schema_id.as_str(),
        import.schema_version.as_str(),
    )
    .await?
    else {
        return Ok((
            import.clone(),
            import.clone(),
            requested_source_snapshot.clone(),
            crate::ClientDbSourceIndexMembershipChangeSet::FullSnapshot,
        ));
    };
    let current_file_hashes = import
        .file_hashes
        .iter()
        .map(|file| (file.path.clone(), file.sha256.clone()))
        .collect::<std::collections::BTreeMap<_, _>>();
    let current_owner_paths = materialization
        .owners
        .iter()
        .map(|owner| owner.owner_path.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let changed_owner_paths = current_owner_paths
        .iter()
        .filter(|path| {
            previous.file_hashes.get(path.as_str()) != current_file_hashes.get(path.as_str())
        })
        .cloned()
        .collect::<Vec<_>>();
    let removed_owner_paths = previous
        .owners
        .iter()
        .map(|owner| owner.owner_path.clone())
        .filter(|path| !current_owner_paths.contains(path))
        .collect::<Vec<_>>();
    if changed_owner_paths.is_empty() && removed_owner_paths.is_empty() {
        return Ok((
            import.clone(),
            import.clone(),
            requested_source_snapshot.clone(),
            crate::ClientDbSourceIndexMembershipChangeSet::FullSnapshot,
        ));
    }
    prepare_turso_source_index_overlay(
        connection,
        project_root,
        import,
        workspace_snapshot,
        requested_source_snapshot,
        previous,
        changed_owner_paths,
        removed_owner_paths,
    )
    .await
}

async fn prepare_turso_source_index_overlay(
    connection: &turso::Connection,
    project_root: &str,
    import: &ClientDbSourceIndexImport,
    workspace_snapshot: &agent_semantic_content_identity::WorkspaceSnapshot,
    requested_source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
    previous: crate::ClientDbSourceIndexGenerationSnapshot,
    changed_owner_paths: Vec<String>,
    removed_owner_paths: Vec<String>,
) -> Result<
    (
        ClientDbSourceIndexImport,
        ClientDbSourceIndexImport,
        agent_semantic_content_identity::SourceSnapshotEvidence,
        crate::ClientDbSourceIndexMembershipChangeSet,
    ),
    String,
> {
    let source_snapshot = workspace_snapshot.overlay_evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        requested_source_snapshot.provider_digest.clone(),
        previous.source_snapshot.root_digest.clone(),
        agent_semantic_content_identity::WorkspaceOverlayPaths::new(
            changed_owner_paths.iter().cloned(),
            removed_owner_paths.iter().cloned(),
        ),
    )?;
    let active_blobs =
        super::generation_snapshot::load_turso_source_index_generation_blobs_on_connection(
            connection,
            project_root,
            import.schema_id.as_str(),
            import.schema_version.as_str(),
            &previous,
        )
        .await?;
    let changed_owner_set = changed_owner_paths.iter().cloned().collect();
    let removed_owner_set = removed_owner_paths.iter().cloned().collect();
    let writer_import = crate::source_index::partial_source_index_import(
        import,
        &changed_owner_set,
        &removed_owner_set,
    )?;
    let canonical_import = crate::overlay_active_source_index_import(
        &previous,
        &active_blobs,
        &writer_import,
        &changed_owner_set,
        &removed_owner_set,
    )?;
    let membership_change_set = crate::ClientDbSourceIndexMembershipChangeSet::MerkleOverlay {
        base_generation_id: previous.generation_id,
        changed_owner_paths: changed_owner_paths
            .into_iter()
            .map(crate::ClientDbSourceIndexPath::new)
            .collect(),
        removed_owner_paths: removed_owner_paths
            .into_iter()
            .map(crate::ClientDbSourceIndexPath::new)
            .collect(),
    };
    Ok((
        canonical_import,
        writer_import,
        source_snapshot,
        membership_change_set,
    ))
}

async fn reusable_prepared_source_index_generation(
    connection: &turso::Connection,
    prepared: &PreparedTursoSourceIndexRefresh,
    materialization: &crate::runtime_server_workspace::WorkspaceCanonicalMaterialization,
    trace_started: std::time::Instant,
) -> Result<Option<ClientDbSourceIndexRefreshReport>, String> {
    let Some(refresh) = reusable_turso_source_index_generation(
        connection,
        &prepared.import,
        &prepared.project_root,
        &prepared.file_hashes_json,
        &prepared.source_snapshot,
        prepared.file_count,
    )
    .await?
    else {
        source_index_db_trace("reuse-probe-missed", trace_started);
        return Ok(None);
    };
    let existing = super::materialization::load_workspace_generation_materialization(
        connection,
        materialization.workspace_identity.as_str(),
        prepared.project_root.as_str(),
        prepared.import.schema_id.as_str(),
        prepared.import.schema_version.as_str(),
        refresh.generation_id.as_str(),
    )
    .await?;
    if existing
        .as_ref()
        .is_some_and(|value| value.has_same_generation_identity(materialization))
    {
        source_index_db_trace("generation-reused", trace_started);
        return Ok(Some(refresh));
    }
    source_index_db_trace("generation-materialization-not-reusable", trace_started);
    Ok(None)
}

async fn persist_prepared_source_index_refresh(
    connection: &mut turso::Connection,
    prepared: PreparedTursoSourceIndexRefresh,
    materialization: &mut crate::runtime_server_workspace::WorkspaceCanonicalMaterialization,
    trace_started: std::time::Instant,
) -> Result<ClientDbSourceIndexRefreshReport, String> {
    let (write_stats, effective_materialization) = write_turso_source_index_rows(
        connection,
        &prepared.writer_import,
        materialization,
        &prepared.membership_change_set,
        &prepared.project_root,
        &prepared.file_hashes_json,
        &prepared.source_snapshot_json,
    )
    .await?;
    *materialization = effective_materialization;
    source_index_db_trace("rows-written", trace_started);
    let (owner_count, selector_count) = turso_source_index_scope_row_counts(
        connection,
        &prepared.project_root,
        prepared.import.schema_id.as_str(),
        prepared.import.schema_version.as_str(),
        write_stats.physical_generation_id.as_str(),
    )
    .await?;
    validate_persisted_source_index_counts(
        materialization,
        &write_stats.physical_generation_id,
        owner_count,
        selector_count,
    )?;
    Ok(ClientDbSourceIndexRefreshReport {
        generation_id: write_stats.physical_generation_id.into(),
        reused_generation: false,
        file_count: prepared.file_count,
        source_snapshot: materialization.source_snapshot.clone(),
        owner_count,
        selector_count,
        changed_owner_count: write_stats.changed_owner_count,
        removed_owner_count: write_stats.removed_owner_count,
        posting_write_count: write_stats.posting_write_count,
    })
}

fn validate_persisted_source_index_counts(
    materialization: &crate::runtime_server_workspace::WorkspaceCanonicalMaterialization,
    generation_id: &str,
    owner_count: u32,
    selector_count: u32,
) -> Result<(), String> {
    let expected_owner_count = materialization.owners.len().min(u32::MAX as usize) as u32;
    let expected_selector_count = materialization
        .owners
        .iter()
        .map(|owner| owner.selectors.len())
        .sum::<usize>()
        .min(u32::MAX as usize) as u32;
    if owner_count == expected_owner_count && selector_count >= expected_selector_count {
        return Ok(());
    }
    Err(format!(
        "Turso source-index refresh did not persist generation rows: generation={generation_id} expectedOwners={expected_owner_count} persistedOwners={owner_count} expectedSelectors={expected_selector_count} persistedSelectors={selector_count}"
    ))
}

pub async fn latest_turso_source_index_file_hashes(
    db_path: &Path,
    project_root: &Path,
    schema_id: &SemanticSchemaId,
    schema_version: &SemanticSchemaVersion,
) -> Result<Option<Vec<ClientCacheFileHash>>, String> {
    let Some((_, file_hashes_json, _, _, _)) =
        latest_turso_source_index_generation(db_path, project_root, schema_id, schema_version)
            .await?
    else {
        return Ok(None);
    };
    serde_json::from_str::<Vec<ClientCacheFileHash>>(&file_hashes_json)
        .map(Some)
        .map_err(|error| format!("failed to decode Turso source-index file hashes: {error}"))
}

pub async fn latest_turso_source_index_stats(
    db_path: &Path,
    project_root: &Path,
    schema_id: &SemanticSchemaId,
    schema_version: &SemanticSchemaVersion,
) -> Result<Option<ClientDbSourceIndexStats>, String> {
    let Some((generation_id, _, source_snapshot_json, owner_count, selector_count)) =
        latest_turso_source_index_generation(db_path, project_root, schema_id, schema_version)
            .await?
    else {
        return Ok(None);
    };
    let source_snapshot = serde_json::from_str(&source_snapshot_json).map_err(|error| {
        format!("failed to decode latest Turso source-index source snapshot evidence: {error}")
    })?;
    let connection = connect_turso_client_db(db_path).await?;
    ensure_turso_source_index_schema(&connection).await?;
    let normalized_project_root = normalized_project_root(project_root)?;
    if !turso_source_index_projection_ready(
        &connection,
        normalized_project_root.as_str(),
        schema_id.as_str(),
        schema_version.as_str(),
    )
    .await?
    {
        return Ok(None);
    }
    Ok(Some(ClientDbSourceIndexStats {
        generation_id: generation_id.into(),
        owner_count,
        selector_count,
        source_snapshot,
    }))
}

pub async fn latest_turso_source_index_scope_files(
    db_path: &Path,
    project_root: &Path,
    schema_id: &SemanticSchemaId,
    schema_version: &SemanticSchemaVersion,
) -> Result<Option<Vec<ClientDbSourceIndexScopeFile>>, String> {
    let Some(snapshot) = super::generation_snapshot::latest_turso_source_index_generation_snapshot(
        db_path,
        project_root,
        schema_id,
        schema_version,
    )
    .await?
    else {
        return Ok(None);
    };
    Ok(Some(
        snapshot
            .owners
            .into_iter()
            .filter_map(|owner| {
                let language_id = owner.language_id?;
                let provider_id = owner.provider_id?;
                let owner_path = PathBuf::from(owner.owner_path);
                let path = if owner_path.is_absolute() {
                    owner_path
                } else {
                    project_root.join(owner_path)
                };
                Some(ClientDbSourceIndexScopeFile {
                    path,
                    language_id: LanguageId::from(language_id),
                    provider_id: ProviderId::from(provider_id),
                    projection_coverage: crate::ClientDbSourceIndexProjectionCoverage::NotDeclared,
                    projection_diagnostic: None,
                    selector_receipts: Vec::new(),
                    relations: Vec::new(),
                })
            })
            .collect(),
    ))
}

pub async fn lookup_reusable_turso_source_index_generation(
    db_path: &Path,
    project_root: &Path,
    schema_id: &SemanticSchemaId,
    schema_version: &SemanticSchemaVersion,
    file_hashes: &[ClientCacheFileHash],
) -> Result<Option<ClientDbSourceIndexStats>, String> {
    if !db_path.exists() {
        return Ok(None);
    }
    let file_hashes_json = serde_json::to_string(file_hashes)
        .map_err(|error| format!("failed to serialize Turso source-index file hashes: {error}"))?;
    let project_root = normalized_project_root(project_root)?;
    let connection = connect_turso_client_db(db_path).await?;
    ensure_turso_source_index_schema(&connection).await?;
    let mut rows = run_turso_operation(
        || async {
            connection
                .query(
                    "SELECT generation_id, owner_count, selector_count, source_snapshot_json
                     FROM asp_source_index_scope_v1
                     WHERE project_root = ?1
                       AND schema_id = ?2
  AND schema_version = ?3
  AND file_hashes_json = ?4
  AND source_snapshot_json <> ''
ORDER BY updated_at_ms DESC, generation_id DESC
LIMIT 1",
                    (
                        project_root.as_str(),
                        schema_id.as_str(),
                        schema_version.as_str(),
                        file_hashes_json.as_str(),
                    ),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to query Turso reusable source-index stats",
    )
    .await?;
    let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso reusable source-index stats: {error}"))?
    else {
        return Ok(None);
    };
    Ok(Some(ClientDbSourceIndexStats {
        generation_id: row
            .get::<String>(0)
            .map_err(|error| format!("failed to read Turso source-index generation id: {error}"))?
            .into(),
        owner_count: row
            .get::<i64>(1)
            .map_err(|error| format!("failed to read Turso source-index owner count: {error}"))?
            .max(0)
            .min(i64::from(u32::MAX)) as u32,
        selector_count: row
            .get::<i64>(2)
            .map_err(|error| format!("failed to read Turso source-index selector count: {error}"))?
            .max(0)
            .min(i64::from(u32::MAX)) as u32,
        source_snapshot: serde_json::from_str(&row.get::<String>(3).map_err(|error| {
            format!("failed to read Turso source-index source snapshot evidence: {error}")
        })?)
        .map_err(|error| {
            format!("failed to decode Turso source-index source snapshot evidence: {error}")
        })?,
    }))
}

async fn latest_turso_source_index_generation(
    db_path: &Path,
    project_root: &Path,
    schema_id: &SemanticSchemaId,
    schema_version: &SemanticSchemaVersion,
) -> Result<Option<(String, String, String, u32, u32)>, String> {
    if !db_path.exists() {
        return Ok(None);
    }
    let project_root = normalized_project_root(project_root)?;
    let connection = connect_turso_client_db(db_path).await?;
    ensure_turso_source_index_schema(&connection).await?;
    super::generation_snapshot::latest_turso_source_index_generation_on_connection(
        &connection,
        project_root.as_str(),
        schema_id.as_str(),
        schema_version.as_str(),
    )
    .await
}

async fn reusable_turso_source_index_generation(
    connection: &turso::Connection,
    import: &ClientDbSourceIndexImport,
    project_root: &str,
    file_hashes_json: &str,
    source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
    file_count: u32,
) -> Result<Option<ClientDbSourceIndexRefreshReport>, String> {
    let selector_fingerprint = turso_source_index_selector_fingerprint(import)?;
    let mut rows = run_turso_operation(
        || async {
            connection
                .query(
                    "SELECT generation_id, owner_count, selector_count, source_snapshot_json
                     FROM asp_source_index_scope_v1
                     WHERE project_root = ?1
                       AND schema_id = ?2
                       AND schema_version = ?3
                       AND file_hashes_json = ?4
                       AND source_snapshot_json <> ''
                       AND selector_fingerprint = ?5
                       AND EXISTS (
                           SELECT 1
                           FROM asp_source_index_layout_v1 AS layout
                           WHERE layout.project_root = asp_source_index_scope_v1.project_root
                             AND layout.schema_id = asp_source_index_scope_v1.schema_id
                             AND layout.schema_version = asp_source_index_scope_v1.schema_version
                             AND layout.term_projection_version = ?6
                             AND layout.token_projection_generation_id = asp_source_index_scope_v1.generation_id
                       )
                       AND EXISTS (
                           SELECT 1
                           FROM asp_source_index_token_owner_v1 AS token_owner
                           WHERE token_owner.project_root = asp_source_index_scope_v1.project_root
                             AND token_owner.schema_id = asp_source_index_scope_v1.schema_id
                             AND token_owner.schema_version = asp_source_index_scope_v1.schema_version
                             AND token_owner.generation_id = asp_source_index_scope_v1.generation_id
                       )
                       AND EXISTS (
                           SELECT 1
                           FROM asp_source_index_owner_v1 AS owner
                           WHERE owner.project_root = asp_source_index_scope_v1.project_root
                             AND owner.schema_id = asp_source_index_scope_v1.schema_id
                             AND owner.schema_version = asp_source_index_scope_v1.schema_version
                             AND owner.generation_id = asp_source_index_scope_v1.generation_id
                       )
                       AND (
                           asp_source_index_scope_v1.selector_count = 0
                           OR asp_source_index_scope_v1.selector_count = (
                               SELECT COUNT(*)
                               FROM asp_source_index_selector_v1 AS selector
                               WHERE selector.project_root = asp_source_index_scope_v1.project_root
                                 AND selector.schema_id = asp_source_index_scope_v1.schema_id
                                 AND selector.schema_version = asp_source_index_scope_v1.schema_version
                                 AND selector.generation_id = asp_source_index_scope_v1.generation_id
                           )
                       )
                     ORDER BY updated_at_ms DESC, generation_id DESC
                     LIMIT 8",
                    (
                        project_root,
                        import.schema_id.as_str(),
                        import.schema_version.as_str(),
                        file_hashes_json,
                        selector_fingerprint.as_str(),
                        TURSO_SOURCE_INDEX_TERM_PROJECTION_VERSION,
                    ),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to query Turso reusable source-index generation",
    )
    .await?;
    let row = loop {
        let Some(row) = rows.next().await.map_err(|error| {
            format!("failed to read Turso reusable source-index generation: {error}")
        })?
        else {
            return Ok(None);
        };
        let persisted_snapshot_json = row.get::<String>(3).map_err(|error| {
            format!("failed to read reusable source-index snapshot evidence: {error}")
        })?;
        let persisted_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence =
            serde_json::from_str(&persisted_snapshot_json).map_err(|error| {
                format!("failed to decode reusable source-index snapshot evidence: {error}")
            })?;
        if persisted_snapshot.has_same_content_identity(source_snapshot) {
            break row;
        }
    };
    let generation_id = row
        .get::<String>(0)
        .map_err(|error| format!("failed to read Turso source-index generation id: {error}"))?;
    let metadata_owner_count = row
        .get::<i64>(1)
        .map_err(|error| format!("failed to read Turso source-index owner count: {error}"))?
        .max(0)
        .min(i64::from(u32::MAX)) as u32;
    let metadata_selector_count = row
        .get::<i64>(2)
        .map_err(|error| format!("failed to read Turso source-index selector count: {error}"))?
        .max(0)
        .min(i64::from(u32::MAX)) as u32;
    let (owner_count, selector_count) = turso_source_index_scope_row_counts(
        connection,
        project_root,
        import.schema_id.as_str(),
        import.schema_version.as_str(),
        generation_id.as_str(),
    )
    .await?;
    if owner_count != metadata_owner_count || selector_count != metadata_selector_count {
        return Err(format!(
            "Turso source-index reusable generation has stale metadata: generation={generation_id} metadataOwners={metadata_owner_count} persistedOwners={owner_count} metadataSelectors={metadata_selector_count} persistedSelectors={selector_count}"
        ));
    }
    Ok(Some(ClientDbSourceIndexRefreshReport {
        generation_id: generation_id.into(),
        reused_generation: true,
        file_count,
        source_snapshot: source_snapshot.clone(),
        owner_count,
        selector_count,
        changed_owner_count: 0,
        removed_owner_count: 0,
        posting_write_count: 0,
    }))
}

pub(super) async fn turso_source_index_scope_row_counts(
    connection: &turso::Connection,
    project_root: &str,
    schema_id: &str,
    schema_version: &str,
    generation_id: &str,
) -> Result<(u32, u32), String> {
    let mut rows = run_turso_operation(
        || async {
            connection
                .query(
                    "SELECT
                         COUNT(*),
                         COALESCE(SUM(selector_count), 0),
                         (
                             SELECT COUNT(*)
                             FROM asp_source_index_selector_v1
                             WHERE project_root = ?1
                               AND schema_id = ?2
                               AND schema_version = ?3
                               AND generation_id = ?4
                         )
                     FROM asp_source_index_owner_v1
                     WHERE project_root = ?1
                       AND schema_id = ?2
                       AND schema_version = ?3
                       AND generation_id = ?4",
                    (project_root, schema_id, schema_version, generation_id),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to count Turso source-index snapshot rows",
    )
    .await?;
    let Some(row) = rows.next().await.map_err(|error| {
        format!("failed to read Turso source-index snapshot row counts: {error}")
    })?
    else {
        return Ok((0, 0));
    };
    let owner_count = row
        .get::<i64>(0)
        .map_err(|error| {
            format!("failed to read Turso source-index snapshot owner count: {error}")
        })?
        .max(0)
        .min(i64::from(u32::MAX)) as u32;
    let owner_selector_count = row
        .get::<i64>(1)
        .map_err(|error| {
            format!("failed to read Turso source-index snapshot selector count: {error}")
        })?
        .max(0)
        .min(i64::from(u32::MAX)) as u32;
    let selector_count = row
        .get::<i64>(2)
        .map_err(|error| format!("failed to read Turso normalized selector count: {error}"))?
        .max(0)
        .min(i64::from(u32::MAX)) as u32;
    if owner_selector_count != selector_count {
        return Err(format!(
            "Turso source-index selector projection is incomplete: ownerSelectors={owner_selector_count} normalizedSelectors={selector_count}"
        ));
    }
    Ok((owner_count, selector_count))
}

pub(in crate::engine) fn turso_source_index_access_lock(
    db_path: &std::path::Path,
) -> std::sync::Arc<tokio::sync::RwLock<()>> {
    type LockRegistry =
        std::collections::HashMap<std::path::PathBuf, std::sync::Weak<tokio::sync::RwLock<()>>>;
    const LOCK_REGISTRY_SHARDS: usize = 64;
    const STALE_ENTRY_CLEANUP_THRESHOLD: usize = 256;
    static LOCKS: std::sync::OnceLock<Box<[parking_lot::Mutex<LockRegistry>]>> =
        std::sync::OnceLock::new();
    let lock_path = std::fs::canonicalize(db_path).unwrap_or_else(|_| {
        db_path
            .parent()
            .and_then(|parent| std::fs::canonicalize(parent).ok())
            .and_then(|parent| db_path.file_name().map(|name| parent.join(name)))
            .unwrap_or_else(|| db_path.to_path_buf())
    });
    let shards = LOCKS.get_or_init(|| {
        (0..LOCK_REGISTRY_SHARDS)
            .map(|_| parking_lot::Mutex::new(LockRegistry::new()))
            .collect::<Vec<_>>()
            .into_boxed_slice()
    });
    let mut path_hasher = std::collections::hash_map::DefaultHasher::new();
    std::hash::Hash::hash(&lock_path, &mut path_hasher);
    let shard_index = std::hash::Hasher::finish(&path_hasher) as usize % shards.len();
    let mut shard = shards[shard_index].lock();
    if let Some(lock) = shard.get(&lock_path).and_then(std::sync::Weak::upgrade) {
        return lock;
    }
    if shard.len() >= STALE_ENTRY_CLEANUP_THRESHOLD {
        shard.retain(|_, lock| lock.strong_count() > 0);
    }
    let lock = std::sync::Arc::new(tokio::sync::RwLock::new(()));
    shard.insert(lock_path, std::sync::Arc::downgrade(&lock));
    lock
}
