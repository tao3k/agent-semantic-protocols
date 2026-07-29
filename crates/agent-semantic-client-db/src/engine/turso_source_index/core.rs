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

pub async fn refresh_turso_source_index_import(
    db_path: &Path,
    request: ClientDbSourceIndexRefreshRequest,
) -> Result<ClientDbSourceIndexRefreshReport, String> {
    let _source_index_write_guard = turso_source_index_access_lock(db_path).write_owned().await;
    let trace_started = std::time::Instant::now();
    let source_snapshot_json =
        serde_json::to_string(&request.source_snapshot).map_err(|error| {
            format!("failed to serialize Turso source-index source snapshot evidence: {error}")
        })?;
    super::membership::validate_source_index_membership_change_set(&request)?;
    let membership_change_set = request.membership_change_set;
    let import = request.import;
    if import.file_hashes.is_empty() {
        return Err("source index import requires file hash evidence".to_string());
    }
    source_index_db_trace("operation-lock-acquired", trace_started);
    crate::engine::turso_bootstrap::bootstrap_turso_source_index_db(db_path).await?;
    source_index_db_trace("base-bootstrap-complete", trace_started);
    let mut connection = connect_turso_client_db(db_path).await?;
    source_index_db_trace("write-connection-open", trace_started);
    let source_index_schema_rebuilt = ensure_turso_source_index_schema(&connection).await?;
    source_index_db_trace(
        if source_index_schema_rebuilt {
            "source-index-schema-rebuilt"
        } else {
            "source-index-schema-verified"
        },
        trace_started,
    );
    let file_hashes_json = serde_json::to_string(&import.file_hashes)
        .map_err(|error| format!("failed to serialize Turso source-index file hashes: {error}"))?;
    let project_root = normalized_project_root(&import.project_root);
    if let Some(refresh) = reusable_turso_source_index_generation(
        &connection,
        &import,
        &project_root,
        &file_hashes_json,
        &source_snapshot_json,
        request.file_count,
    )
    .await?
    {
        source_index_db_trace("generation-reused", trace_started);
        return Ok(refresh);
    }
    source_index_db_trace("reuse-probe-missed", trace_started);
    let write_stats = write_turso_source_index_rows(
        &mut connection,
        &import,
        &membership_change_set,
        &project_root,
        &file_hashes_json,
        &source_snapshot_json,
    )
    .await?;
    source_index_db_trace("rows-written", trace_started);
    let (owner_count, selector_count) = turso_source_index_scope_row_counts(
        &connection,
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
    let normalized_project_root = normalized_project_root(project_root);
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
                    selector_receipts: Vec::new(),
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
    let project_root = normalized_project_root(project_root);
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
    let project_root = normalized_project_root(project_root);
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
        update_text(&mut hasher, selector.source.as_str());
        hasher.update((selector.query_keys.len() as u64).to_be_bytes());
        for query_key in &selector.query_keys {
            update_text(&mut hasher, query_key.as_str());
        }
        let proof = &selector.materialization_proof;
        update_text(&mut hasher, &proof.language_id);
        update_text(&mut hasher, &proof.provider_id);
        update_text(&mut hasher, &proof.structural_selector);
        update_text(&mut hasher, &proof.owner_path);
        hasher.update(proof.parser_identity_digest);
        hasher.update(proof.query_pack_digest);
        hasher.update(proof.workspace_root_digest);
        hasher.update(proof.owner_subtree_digest);
        hasher.update(proof.source_blob_digest);
        hasher.update(proof.normalized_parser_facts_digest);
        hasher.update(proof.source_byte_start.to_be_bytes());
        hasher.update(proof.source_byte_end.to_be_bytes());
        hasher.update([proof.projection_mode as u8]);
        hasher.update(proof.projection_digest);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

async fn reusable_turso_source_index_generation(
    connection: &turso::Connection,
    import: &ClientDbSourceIndexImport,
    project_root: &str,
    file_hashes_json: &str,
    source_snapshot_json: &str,
    file_count: u32,
) -> Result<Option<ClientDbSourceIndexRefreshReport>, String> {
    let selector_fingerprint = turso_source_index_selector_fingerprint(import)?;
    let mut rows = run_turso_operation(
        || async {
            connection
                .query(
                    "SELECT generation_id, owner_count, selector_count
                     FROM asp_source_index_scope_v1
                     WHERE project_root = ?1
                       AND schema_id = ?2
                       AND schema_version = ?3
                       AND file_hashes_json = ?4
                       AND source_snapshot_json = ?5
                       AND selector_fingerprint = ?6
                       AND EXISTS (
                           SELECT 1
                           FROM asp_source_index_layout_v1 AS layout
                           WHERE layout.project_root = asp_source_index_scope_v1.project_root
                             AND layout.schema_id = asp_source_index_scope_v1.schema_id
                             AND layout.schema_version = asp_source_index_scope_v1.schema_version
                             AND layout.term_projection_version = ?7
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
                     LIMIT 1",
                    (
                        project_root,
                        import.schema_id.as_str(),
                        import.schema_version.as_str(),
                        file_hashes_json,
                        source_snapshot_json,
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
    let Some(row) = rows.next().await.map_err(|error| {
        format!("failed to read Turso reusable source-index generation: {error}")
    })?
    else {
        return Ok(None);
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

pub(in crate::engine) async fn lookup_exact_selector_projection_v1(
    db_path: &Path,
    key: &agent_semantic_content_identity::exact_selector_cache::ExactSelectorMerkleLookupKeyV1<'_>,
) -> Result<
    Option<
        agent_semantic_content_identity::exact_selector_cache::ValidatedExactSelectorProjectionV1,
    >,
    String,
> {
    if !db_path.exists() {
        return Ok(None);
    }
    let connection = match crate::engine::turso::connect_turso_client_db_read_only(db_path).await {
        Ok(connection) => connection,
        Err(error) if crate::engine::turso_lock_policy::is_turso_lock_error(&error) => {
            return Ok(None);
        }
        Err(error) => return Err(error),
    };
    let table_exists = match crate::engine::turso::turso_table_exists(
        &connection,
        "asp_exact_selector_projection_v1",
    )
    .await
    {
        Ok(table_exists) => table_exists,
        Err(error) if crate::engine::turso_lock_policy::is_turso_lock_error(&error) => {
            return Ok(None);
        }
        Err(error) => return Err(error),
    };
    if !table_exists {
        return Ok(None);
    }
    let projection_mode = exact_projection_mode_v1_name(key.projection_mode);
    let mut rows = run_turso_operation(
        || async {
            connection
                .query(
                    "SELECT record_json
                     FROM asp_exact_selector_projection_v1
                     WHERE language_id = ?1
                       AND workspace_root_digest = ?2
                       AND owner_path = ?3
                       AND owner_subtree_digest = ?4
                       AND source_blob_digest = ?5
                       AND parser_identity_digest = ?6
                       AND query_pack_digest = ?7
                       AND structural_selector = ?8
                       AND projection_mode = ?9
                     LIMIT 1",
                    (
                        key.language_id,
                        key.workspace_root_digest.as_str(),
                        key.owner_path,
                        key.owner_subtree_digest.as_str(),
                        key.source_blob_digest.as_str(),
                        key.parser_identity_digest.as_str(),
                        key.query_pack_digest.as_str(),
                        key.structural_selector,
                        projection_mode,
                    ),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to query Turso exact-selector projection",
    )
    .await?;
    let next_row = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso exact-selector projection: {error}"));
    let Some(row) = (match next_row {
        Ok(row) => row,
        Err(error) if crate::engine::turso_lock_policy::is_turso_lock_error(&error) => {
            return Ok(None);
        }
        Err(error) => return Err(error),
    }) else {
        return Ok(None);
    };
    let record_json = row
        .get::<String>(0)
        .map_err(|error| format!("failed to read Turso exact-selector record JSON: {error}"))?;
    let record = match serde_json::from_str(&record_json) {
        Ok(record) => record,
        Err(_) => return Ok(None),
    };
    let validated = match agent_semantic_content_identity::exact_selector_cache::ValidatedExactSelectorProjectionV1::hydrate(
        record,
        key,
    ) {
        Ok(validated) => validated,
        Err(_) => return Ok(None),
    };
    Ok(Some(validated))
}

pub(in crate::engine) async fn persist_exact_selector_projection_v1(
    db_path: &Path,
    key: &agent_semantic_content_identity::exact_selector_cache::ExactSelectorMerkleLookupKeyV1<'_>,
    record: &agent_semantic_content_identity::exact_selector_cache::ExactSelectorProjectionRecordV1,
) -> Result<(), String> {
    agent_semantic_content_identity::exact_selector_cache::ValidatedExactSelectorProjectionV1::hydrate(
        record.clone(),
        key,
    )
    .map_err(|miss| format!("refusing invalid exact-selector projection write: {miss:?}"))?;
    let record_json = serde_json::to_string(record)
        .map_err(|error| format!("failed to encode Turso exact-selector record: {error}"))?;
    let connection = match connect_turso_client_db(db_path).await {
        Ok(connection) => connection,
        Err(error) if crate::engine::turso_lock_policy::is_turso_lock_error(&error) => {
            return Ok(());
        }
        Err(error) => return Err(error),
    };
    if let Err(error) = ensure_turso_source_index_schema(&connection).await {
        if crate::engine::turso_lock_policy::is_turso_lock_error(&error) {
            return Ok(());
        }
        return Err(error);
    }
    let projection_mode = exact_projection_mode_v1_name(key.projection_mode);
    if let Err(error) = run_turso_operation(
        || async {
            connection
                .execute(
                    "INSERT OR REPLACE INTO asp_exact_selector_projection_v1 (
                        language_id,
                        workspace_root_digest,
                        owner_path,
                        owner_subtree_digest,
                        source_blob_digest,
                        parser_identity_digest,
                        query_pack_digest,
                        structural_selector,
                        projection_mode,
                        record_json
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    (
                        key.language_id,
                        key.workspace_root_digest.as_str(),
                        key.owner_path,
                        key.owner_subtree_digest.as_str(),
                        key.source_blob_digest.as_str(),
                        key.parser_identity_digest.as_str(),
                        key.query_pack_digest.as_str(),
                        key.structural_selector,
                        projection_mode,
                        record_json.as_str(),
                    ),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to persist Turso exact-selector projection",
    )
    .await
    {
        if crate::engine::turso_lock_policy::is_turso_lock_error(&error) {
            return Ok(());
        }
        return Err(error);
    }
    Ok(())
}

fn exact_projection_mode_v1_name(
    mode: agent_semantic_content_identity::exact_selector_merkle::ExactProjectionModeV1,
) -> &'static str {
    match mode {
        agent_semantic_content_identity::exact_selector_merkle::ExactProjectionModeV1::Code => {
            "code"
        }
        agent_semantic_content_identity::exact_selector_merkle::ExactProjectionModeV1::Skeleton => {
            "skeleton"
        }
        agent_semantic_content_identity::exact_selector_merkle::ExactProjectionModeV1::Names => {
            "names"
        }
        agent_semantic_content_identity::exact_selector_merkle::ExactProjectionModeV1::Verbatim => {
            "verbatim"
        }
    }
}
