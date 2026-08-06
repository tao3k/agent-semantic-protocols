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

pub(in crate::engine) const TURSO_SOURCE_INDEX_TERM_PROJECTION_VERSION: i64 = 3;

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
            connection
                .query(
                    "SELECT language_id, workspace_root_digest, owner_path,
                            owner_subtree_digest, source_blob_digest,
                            parser_identity_digest, query_pack_digest,
                            structural_selector, projection_mode, record_json
                     FROM asp_exact_selector_projection_v1
                     LIMIT 1",
                    (),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to inspect Turso source-index schema layout",
    )
    .await?;
    Ok(())
}

async fn reset_turso_source_index_schema(connection: &turso::Connection) -> Result<(), String> {
    for table in [
        "asp_exact_selector_projection_v1",
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

pub async fn refresh_turso_source_index_import_on_connection(
    connection: &mut turso::Connection,
    request: ClientDbSourceIndexRefreshRequest,
    materialization: &mut crate::runtime_server_workspace::WorkspaceCanonicalMaterialization,
) -> Result<ClientDbSourceIndexRefreshReport, String> {
    let trace_started = std::time::Instant::now();
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
    let mut source_snapshot = requested_source_snapshot.clone();
    if import.file_hashes.is_empty() {
        return Err("source index import requires file hash evidence".to_string());
    }
    source_index_db_trace("resident-writer-admitted", trace_started);
    ensure_turso_source_index_schema(connection).await?;
    source_index_db_trace("source-index-schema-verified", trace_started);
    let file_hashes_json = serde_json::to_string(&import.file_hashes)
        .map_err(|error| format!("failed to serialize Turso source-index file hashes: {error}"))?;
    let project_root = normalized_project_root(&import.project_root)?;
    let current_file_hashes = import
        .file_hashes
        .iter()
        .map(|file| {
            (
                file.path.as_str().to_owned(),
                file.sha256.as_str().to_owned(),
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    let current_owner_paths = import
        .owners
        .iter()
        .map(|owner| owner.owner_path.as_str().to_owned())
        .collect::<std::collections::BTreeSet<_>>();
    let membership_change_set = if let Some(previous) =
        super::generation_snapshot::load_turso_source_index_generation_snapshot(
            connection,
            &project_root,
            import.schema_id.as_str(),
            import.schema_version.as_str(),
        )
        .await?
    {
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
            crate::ClientDbSourceIndexMembershipChangeSet::FullSnapshot
        } else {
            source_snapshot = workspace_snapshot.overlay_evidence(
                agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
                requested_source_snapshot.provider_digest.clone(),
                previous.source_snapshot.root_digest,
                changed_owner_paths.iter().cloned(),
                removed_owner_paths.iter().cloned(),
            )?;
            crate::ClientDbSourceIndexMembershipChangeSet::MerkleOverlay {
                changed_owner_paths: changed_owner_paths
                    .into_iter()
                    .map(crate::ClientDbSourceIndexPath::new)
                    .collect(),
                removed_owner_paths: removed_owner_paths
                    .into_iter()
                    .map(crate::ClientDbSourceIndexPath::new)
                    .collect(),
            }
        }
    } else {
        crate::ClientDbSourceIndexMembershipChangeSet::FullSnapshot
    };
    super::membership::validate_source_index_membership_change_set(
        &import,
        &source_snapshot,
        &membership_change_set,
    )?;
    materialization.finalize_generation_evidence(workspace_snapshot, source_snapshot.clone())?;
    materialization.validate_against(
        materialization.workspace_identity.as_str(),
        &source_snapshot,
        &import,
        import.owners.len().min(u32::MAX as usize) as u32,
    )?;
    let source_snapshot_json = serde_json::to_string(&source_snapshot).map_err(|error| {
        format!("failed to serialize Turso source-index source snapshot evidence: {error}")
    })?;
    if let Some(refresh) = reusable_turso_source_index_generation(
        connection,
        &import,
        &project_root,
        &file_hashes_json,
        &source_snapshot,
        request.file_count,
    )
    .await?
    {
        match super::materialization::load_workspace_generation_materialization(
            connection,
            materialization.workspace_identity.as_str(),
            project_root.as_str(),
            import.schema_id.as_str(),
            import.schema_version.as_str(),
            refresh.generation_id.as_str(),
        )
        .await?
        {
            Some(existing) if existing.has_same_generation_identity(&materialization) => {
                source_index_db_trace("generation-reused", trace_started);
                return Ok(refresh);
            }
            Some(_) => {
                source_index_db_trace("generation-materialization-superseded", trace_started);
            }
            None => {
                source_index_db_trace("generation-materialization-missing", trace_started);
            }
        }
    }
    source_index_db_trace("reuse-probe-missed", trace_started);
    let write_stats = write_turso_source_index_rows(
        connection,
        &import,
        &materialization,
        &membership_change_set,
        &project_root,
        &file_hashes_json,
        &source_snapshot_json,
    )
    .await?;
    source_index_db_trace("rows-written", trace_started);
    let (owner_count, selector_count) = turso_source_index_scope_row_counts(
        connection,
        &project_root,
        import.schema_id.as_str(),
        import.schema_version.as_str(),
        write_stats.physical_generation_id.as_str(),
    )
    .await?;
    let expected_owner_count = import.owners.len().min(u32::MAX as usize) as u32;
    let expected_selector_count = import.selectors.len().min(u32::MAX as usize) as u32;
    if owner_count != expected_owner_count || selector_count < expected_selector_count {
        return Err(format!(
            "Turso source-index refresh did not persist generation rows: generation={} expectedOwners={} persistedOwners={} expectedSelectors={} persistedSelectors={}",
            write_stats.physical_generation_id.as_str(),
            expected_owner_count,
            owner_count,
            expected_selector_count,
            selector_count
        ));
    }
    Ok(ClientDbSourceIndexRefreshReport {
        generation_id: write_stats.physical_generation_id.clone().into(),
        reused_generation: false,
        file_count: request.file_count,
        source_snapshot,
        owner_count,
        selector_count,
        changed_owner_count: write_stats.changed_owner_count,
        removed_owner_count: write_stats.removed_owner_count,
        posting_write_count: write_stats.posting_write_count,
    })
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

pub(super) fn turso_source_index_selector_fingerprint(
    import: &ClientDbSourceIndexImport,
) -> Result<String, String> {
    use sha2::{Digest, Sha256};

    fn update_text(hasher: &mut Sha256, value: &str) {
        hasher.update((value.len() as u64).to_be_bytes());
        hasher.update(value.as_bytes());
    }

    let mut hasher = Sha256::new();
    hasher.update(b"asp.source-index-selector-fingerprint.v1\0");
    hasher.update((import.selectors.len() as u64).to_be_bytes());
    for selector in &import.selectors {
        update_text(&mut hasher, selector.owner_path.as_str());
        update_text(&mut hasher, selector.selector_id.as_str());
        update_text(
            &mut hasher,
            selector.symbol.as_ref().map_or("", |value| value.as_str()),
        );
        update_text(
            &mut hasher,
            selector.kind.as_ref().map_or("", |value| value.as_str()),
        );
        hasher.update((selector.query_keys.len() as u64).to_be_bytes());
        for query_key in &selector.query_keys {
            update_text(&mut hasher, query_key.as_str());
        }
        update_text(&mut hasher, selector.provider_id.as_str());
        let identity =
            serde_json::to_vec(selector.projection_record.proof.canonical_item_selector())
                .map_err(|error| {
                    format!("encode source-index selector canonical identity: {error}")
                })?;
        hasher.update((identity.len() as u64).to_be_bytes());
        hasher.update(identity);
        let projection = serde_json::to_vec(&selector.projection_record)
            .map_err(|error| format!("encode source-index selector projection record: {error}"))?;
        hasher.update((projection.len() as u64).to_be_bytes());
        hasher.update(projection);
    }
    Ok(format!("{:x}", hasher.finalize()))
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

pub(in crate::engine) use super::exact_projection_store::{
    lookup_exact_selector_projection_v1, persist_exact_selector_projection_v1,
};
