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
    turso::turso_table_column_exists,
    turso_operation_lock::acquire_turso_operation_lock,
    turso_statement::{
        execute_turso_statement_with_lock_retry, run_turso_operation_with_lock_retry,
    },
};

pub(super) const TURSO_SOURCE_INDEX_TERM_PROJECTION_VERSION: i64 = 3;

pub(in crate::engine) async fn bootstrap_turso_source_index_schema(
    connection: &turso::Connection,
) -> Result<(), String> {
    for statement in [
        "CREATE TABLE IF NOT EXISTS asp_source_index_scope_v1 (
            project_root TEXT NOT NULL,
            schema_id TEXT NOT NULL,
            schema_version TEXT NOT NULL,
            generation_id TEXT NOT NULL,
            file_hashes_json TEXT NOT NULL,
            owner_count INTEGER NOT NULL,
            selector_count INTEGER NOT NULL,
            updated_at_ms INTEGER NOT NULL,
            PRIMARY KEY (project_root, schema_id, schema_version)
        )",
        "CREATE TABLE IF NOT EXISTS asp_source_index_owner_v1 (
            project_root TEXT NOT NULL,
            schema_id TEXT NOT NULL,
            schema_version TEXT NOT NULL,
            file_hash TEXT NOT NULL,
            owner_path TEXT NOT NULL,
            language_id TEXT,
            provider_id TEXT,
            source_kind TEXT NOT NULL,
            line_count INTEGER,
            query_keys_json TEXT NOT NULL,
            selector_facts_json TEXT NOT NULL,
            term_tokens_json TEXT NOT NULL,
            selector_count INTEGER NOT NULL,
            PRIMARY KEY (project_root, schema_id, schema_version, owner_path)
        )",
        "CREATE TABLE IF NOT EXISTS asp_source_index_layout_v1 (
            project_root TEXT NOT NULL,
            schema_id TEXT NOT NULL,
            schema_version TEXT NOT NULL,
            term_projection_version INTEGER NOT NULL,
            token_projection_generation_id TEXT NOT NULL DEFAULT '',
            PRIMARY KEY (project_root, schema_id, schema_version)
        )",
        "CREATE TABLE IF NOT EXISTS asp_source_index_token_owner_v1 (
            project_root TEXT NOT NULL,
            schema_id TEXT NOT NULL,
            schema_version TEXT NOT NULL,
            token TEXT NOT NULL,
            owner_path TEXT NOT NULL,
            PRIMARY KEY (project_root, schema_id, schema_version, token, owner_path)
        )",
    ] {
        execute_turso_statement_with_lock_retry(
            connection,
            statement,
            "failed to bootstrap Turso source-index schema",
        )
        .await?;
    }
    if !turso_table_column_exists(connection, "asp_source_index_owner_v1", "term_tokens_json")
        .await?
    {
        execute_turso_statement_with_lock_retry(
            connection,
            "ALTER TABLE asp_source_index_owner_v1 ADD COLUMN term_tokens_json TEXT NOT NULL DEFAULT '[]'",
            "failed to migrate Turso source-index owner token projection",
        )
        .await?;
    }
    if !turso_table_column_exists(
        connection,
        "asp_source_index_layout_v1",
        "token_projection_generation_id",
    )
    .await?
    {
        execute_turso_statement_with_lock_retry(
            connection,
            "ALTER TABLE asp_source_index_layout_v1 ADD COLUMN token_projection_generation_id TEXT NOT NULL DEFAULT ''",
            "failed to migrate Turso source-index token projection generation",
        )
        .await?;
    }

    for statement in [
        "CREATE INDEX IF NOT EXISTS asp_source_index_owner_v1_lookup_idx
            ON asp_source_index_owner_v1(project_root, schema_id, schema_version, language_id, owner_path)",
        "CREATE INDEX IF NOT EXISTS asp_source_index_token_owner_v1_owner_idx
            ON asp_source_index_token_owner_v1(project_root, schema_id, schema_version, owner_path, token)",
    ] {
        execute_turso_statement_with_lock_retry(
            connection,
            statement,
            "failed to bootstrap Turso source-index schema",
        )
        .await?;
    }
    Ok(())
}

async fn ensure_turso_source_index_schema(connection: &turso::Connection) -> Result<bool, String> {
    match run_turso_operation_with_lock_retry(
        || async {
            connection
                .query("SELECT 1 FROM asp_source_index_layout_v1 LIMIT 1", ())
                .await
                .map_err(|error| error.to_string())?;
            connection
                .query(
                    "SELECT token_projection_generation_id
                     FROM asp_source_index_layout_v1
                     LIMIT 1",
                    (),
                )
                .await
                .map_err(|error| error.to_string())?;
            connection
                .query("SELECT 1 FROM asp_source_index_token_v1 LIMIT 1", ())
                .await
                .map_err(|error| error.to_string())?;
            connection
                .query("SELECT 1 FROM asp_source_index_token_owner_v1 LIMIT 1", ())
                .await
                .map_err(|error| error.to_string())
        },
        "failed to inspect Turso source-index schema layout",
    )
    .await
    {
        Ok(_) => Ok(false),
        Err(error) if error.contains("no such table") || error.contains("no such column") => {
            bootstrap_turso_source_index_schema(connection).await?;
            Ok(true)
        }
        Err(error) => Err(error),
    }
}

fn source_index_db_trace(stage: &str, started: std::time::Instant) {
    if std::env::var_os("ASP_SOURCE_INDEX_TRACE").is_some() {
        eprintln!(
            "[source-index-db-trace] stage={stage} elapsedMs={}",
            started.elapsed().as_millis()
        );
    }
}

use super::facts::{turso_source_index_projection_ready, write_turso_source_index_rows};

pub async fn refresh_turso_source_index_import(
    db_path: &Path,
    request: ClientDbSourceIndexRefreshRequest,
) -> Result<ClientDbSourceIndexRefreshReport, String> {
    let _source_index_write_guard = turso_source_index_access_lock(db_path).write_owned().await;
    let trace_started = std::time::Instant::now();
    let import = request.import;
    if import.file_hashes.is_empty() {
        return Err("source index import requires file hash evidence".to_string());
    }
    let _operation_lock = acquire_turso_operation_lock(db_path, "source-index-refresh")?;
    source_index_db_trace("operation-lock-acquired", trace_started);
    crate::engine::turso_bootstrap::bootstrap_turso_source_index_db(db_path).await?;
    source_index_db_trace("base-bootstrap-complete", trace_started);
    let connection = connect_turso_client_db(db_path).await?;
    source_index_db_trace("write-connection-open", trace_started);
    let source_index_schema_migrated = ensure_turso_source_index_schema(&connection).await?;
    source_index_db_trace(
        if source_index_schema_migrated {
            "source-index-schema-migrated"
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
        request.file_count,
    )
    .await?
    {
        source_index_db_trace("generation-reused", trace_started);
        return Ok(refresh);
    }
    source_index_db_trace("reuse-probe-missed", trace_started);
    let write_stats =
        write_turso_source_index_rows(&connection, &import, &project_root, &file_hashes_json)
            .await?;
    source_index_db_trace("rows-written", trace_started);
    let (owner_count, selector_count) = turso_source_index_scope_row_counts(
        &connection,
        &project_root,
        import.schema_id.as_str(),
        import.schema_version.as_str(),
    )
    .await?;
    let expected_owner_count = import.owners.len().min(u32::MAX as usize) as u32;
    let expected_selector_count = import.selectors.len().min(u32::MAX as usize) as u32;
    if owner_count != expected_owner_count || selector_count != expected_selector_count {
        return Err(format!(
            "Turso source-index refresh did not persist generation rows: generation={} expectedOwners={} persistedOwners={} expectedSelectors={} persistedSelectors={}",
            import.generation_id.as_str(),
            expected_owner_count,
            owner_count,
            expected_selector_count,
            selector_count
        ));
    }
    Ok(ClientDbSourceIndexRefreshReport {
        generation_id: import.generation_id,
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
    let Some((_, file_hashes_json, _, _)) =
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
    let Some((generation_id, _, owner_count, selector_count)) =
        latest_turso_source_index_generation(db_path, project_root, schema_id, schema_version)
            .await?
    else {
        return Ok(None);
    };
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
    }))
}

pub async fn latest_turso_source_index_scope_files(
    db_path: &Path,
    project_root: &Path,
    schema_id: &SemanticSchemaId,
    schema_version: &SemanticSchemaVersion,
) -> Result<Option<Vec<ClientDbSourceIndexScopeFile>>, String> {
    let Some((_generation_id, _, _, _)) =
        latest_turso_source_index_generation(db_path, project_root, schema_id, schema_version)
            .await?
    else {
        return Ok(None);
    };
    let connection = connect_turso_client_db(db_path).await?;
    ensure_turso_source_index_schema(&connection).await?;
    let normalized_project_root = normalized_project_root(project_root);
    let mut rows = run_turso_operation_with_lock_retry(
        || async {
            connection
                .query(
                    "SELECT owner_path, language_id, provider_id
                     FROM asp_source_index_owner_v1
                     WHERE project_root = ?1
                       AND schema_id = ?2
                       AND schema_version = ?3
                     ORDER BY owner_path",
                    (
                        normalized_project_root.as_str(),
                        schema_id.as_str(),
                        schema_version.as_str(),
                    ),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to query Turso source-index scope files",
    )
    .await?;
    let mut files = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso source-index scope file: {error}"))?
    {
        let Some(language_id) = row
            .get::<Option<String>>(1)
            .map_err(|error| format!("failed to read Turso source-index language id: {error}"))?
        else {
            continue;
        };
        let Some(provider_id) = row
            .get::<Option<String>>(2)
            .map_err(|error| format!("failed to read Turso source-index provider id: {error}"))?
        else {
            continue;
        };
        let owner_path =
            PathBuf::from(row.get::<String>(0).map_err(|error| {
                format!("failed to read Turso source-index owner path: {error}")
            })?);
        let path = if owner_path.is_absolute() {
            owner_path
        } else {
            project_root.join(owner_path)
        };
        files.push(ClientDbSourceIndexScopeFile {
            path,
            language_id: LanguageId::from(language_id),
            provider_id: ProviderId::from(provider_id),
            selector_receipts: Vec::new(),
        });
    }
    Ok(Some(files))
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
    let mut rows = run_turso_operation_with_lock_retry(
        || async {
            connection
                .query(
                    "SELECT generation_id, owner_count, selector_count
                     FROM asp_source_index_scope_v1
                     WHERE project_root = ?1
                       AND schema_id = ?2
                       AND schema_version = ?3
                       AND file_hashes_json = ?4
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
    }))
}

async fn latest_turso_source_index_generation(
    db_path: &Path,
    project_root: &Path,
    schema_id: &SemanticSchemaId,
    schema_version: &SemanticSchemaVersion,
) -> Result<Option<(String, String, u32, u32)>, String> {
    if !db_path.exists() {
        return Ok(None);
    }
    let project_root = normalized_project_root(project_root);
    let connection = connect_turso_client_db(db_path).await?;
    ensure_turso_source_index_schema(&connection).await?;
    let mut rows = run_turso_operation_with_lock_retry(
        || async {
            connection
                .query(
                    "SELECT generation_id, file_hashes_json, owner_count, selector_count
                     FROM asp_source_index_scope_v1
                     WHERE project_root = ?1
                       AND schema_id = ?2
                       AND schema_version = ?3
                     LIMIT 1",
                    (
                        project_root.as_str(),
                        schema_id.as_str(),
                        schema_version.as_str(),
                    ),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to query latest Turso source-index generation",
    )
    .await?;
    let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read latest Turso source-index generation: {error}"))?
    else {
        return Ok(None);
    };
    Ok(Some((
        row.get::<String>(0)
            .map_err(|error| format!("failed to read Turso source-index generation id: {error}"))?,
        row.get::<String>(1)
            .map_err(|error| format!("failed to read Turso source-index file hashes: {error}"))?,
        row.get::<i64>(2)
            .map_err(|error| format!("failed to read Turso source-index owner count: {error}"))?
            .max(0)
            .min(i64::from(u32::MAX)) as u32,
        row.get::<i64>(3)
            .map_err(|error| format!("failed to read Turso source-index selector count: {error}"))?
            .max(0)
            .min(i64::from(u32::MAX)) as u32,
    )))
}

async fn reusable_turso_source_index_generation(
    connection: &turso::Connection,
    import: &ClientDbSourceIndexImport,
    project_root: &str,
    file_hashes_json: &str,
    file_count: u32,
) -> Result<Option<ClientDbSourceIndexRefreshReport>, String> {
    let mut rows = run_turso_operation_with_lock_retry(
        || async {
            connection
                .query(
                    "SELECT generation_id, owner_count, selector_count
                     FROM asp_source_index_scope_v1
                     WHERE project_root = ?1
                       AND schema_id = ?2
                       AND schema_version = ?3
                       AND file_hashes_json = ?4
                       AND EXISTS (
                           SELECT 1
                           FROM asp_source_index_layout_v1 AS layout
                           WHERE layout.project_root = asp_source_index_scope_v1.project_root
                             AND layout.schema_id = asp_source_index_scope_v1.schema_id
                             AND layout.schema_version = asp_source_index_scope_v1.schema_version
                             AND layout.term_projection_version = ?5
                             AND layout.token_projection_generation_id = asp_source_index_scope_v1.generation_id
                       )
                       AND EXISTS (
                           SELECT 1
                           FROM asp_source_index_token_owner_v1 AS token_owner
                           WHERE token_owner.project_root = asp_source_index_scope_v1.project_root
                             AND token_owner.schema_id = asp_source_index_scope_v1.schema_id
                             AND token_owner.schema_version = asp_source_index_scope_v1.schema_version
                       )
                     LIMIT 1",
                    (
                        project_root,
                        import.schema_id.as_str(),
                        import.schema_version.as_str(),
                        file_hashes_json,
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

async fn turso_source_index_scope_row_counts(
    connection: &turso::Connection,
    project_root: &str,
    schema_id: &str,
    schema_version: &str,
) -> Result<(u32, u32), String> {
    let mut rows = run_turso_operation_with_lock_retry(
        || async {
            connection
                .query(
                    "SELECT COUNT(*), COALESCE(SUM(selector_count), 0)
                     FROM asp_source_index_owner_v1
                     WHERE project_root = ?1
                       AND schema_id = ?2
                       AND schema_version = ?3",
                    (project_root, schema_id, schema_version),
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
    let selector_count = row
        .get::<i64>(1)
        .map_err(|error| {
            format!("failed to read Turso source-index snapshot selector count: {error}")
        })?
        .max(0)
        .min(i64::from(u32::MAX)) as u32;
    Ok((owner_count, selector_count))
}

pub(in crate::engine) fn turso_source_index_access_lock(
    db_path: &std::path::Path,
) -> std::sync::Arc<tokio::sync::RwLock<()>> {
    type LockRegistry =
        std::collections::HashMap<std::path::PathBuf, std::sync::Weak<tokio::sync::RwLock<()>>>;
    static LOCKS: std::sync::OnceLock<std::sync::Mutex<LockRegistry>> = std::sync::OnceLock::new();
    let lock_path = std::fs::canonicalize(db_path).unwrap_or_else(|_| {
        db_path
            .parent()
            .and_then(|parent| std::fs::canonicalize(parent).ok())
            .and_then(|parent| db_path.file_name().map(|name| parent.join(name)))
            .unwrap_or_else(|| db_path.to_path_buf())
    });
    let locks = LOCKS.get_or_init(|| std::sync::Mutex::new(LockRegistry::new()));
    let mut locks = locks
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    locks.retain(|_, lock| lock.strong_count() > 0);
    if let Some(lock) = locks.get(&lock_path).and_then(std::sync::Weak::upgrade) {
        return lock;
    }
    let lock = std::sync::Arc::new(tokio::sync::RwLock::new(()));
    locks.insert(lock_path, std::sync::Arc::downgrade(&lock));
    lock
}
