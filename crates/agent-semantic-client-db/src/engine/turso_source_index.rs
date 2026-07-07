//! Turso source-index durable adapter.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_client_core::{
    ClientCacheFileHash, LanguageId, ProviderId, SemanticSchemaId, SemanticSchemaVersion,
};

use crate::source_index::{
    ClientDbSourceIndexImport, ClientDbSourceIndexRefreshReport, ClientDbSourceIndexRefreshRequest,
    ClientDbSourceIndexScopeFile, ClientDbSourceIndexStats,
};
use crate::types::normalized_project_root;

use super::turso::{connect_turso_client_db, turso_table_column_exists};
use super::turso_bootstrap::bootstrap_turso_client_db;
use super::turso_operation_lock::acquire_turso_operation_lock;
use super::turso_statement::{
    execute_turso_operation_with_lock_retry, execute_turso_statement_with_lock_retry,
    run_turso_operation_with_lock_retry,
};

pub(super) async fn bootstrap_turso_source_index_schema(
    connection: &turso::Connection,
) -> Result<(), String> {
    for statement in [
        "CREATE TABLE IF NOT EXISTS asp_source_index_generation (
            generation_id TEXT PRIMARY KEY,
            project_root TEXT NOT NULL,
            schema_id TEXT NOT NULL,
            schema_version TEXT NOT NULL,
            file_hashes_json TEXT NOT NULL,
            owner_count INTEGER NOT NULL,
            selector_count INTEGER NOT NULL,
            updated_at_ms INTEGER NOT NULL
        )",
        "CREATE TABLE IF NOT EXISTS asp_source_index_owner (
            generation_id TEXT NOT NULL,
            owner_path TEXT NOT NULL,
            language_id TEXT,
            provider_id TEXT,
            source_kind TEXT NOT NULL,
            line_count INTEGER,
            query_keys_json TEXT NOT NULL,
            PRIMARY KEY (generation_id, owner_path)
        )",
        "CREATE TABLE IF NOT EXISTS asp_source_index_selector (
            generation_id TEXT NOT NULL,
            owner_path TEXT NOT NULL,
            selector_id TEXT NOT NULL,
            symbol TEXT,
            kind TEXT,
            start_line INTEGER NOT NULL,
            end_line INTEGER NOT NULL,
            source TEXT NOT NULL,
            payload_kind TEXT,
            payload_bounded INTEGER NOT NULL DEFAULT 0,
            query_keys_json TEXT NOT NULL,
            PRIMARY KEY (generation_id, selector_id)
        )",
    ] {
        execute_turso_statement_with_lock_retry(
            connection,
            statement,
            "failed to bootstrap Turso source-index schema",
        )
        .await?;
    }

    ensure_turso_source_index_owner_columns(connection).await?;
    ensure_turso_source_index_selector_columns(connection).await?;

    for statement in [
        "CREATE INDEX IF NOT EXISTS asp_source_index_generation_reuse_idx
            ON asp_source_index_generation(project_root, schema_id, schema_version, file_hashes_json)",
        "CREATE INDEX IF NOT EXISTS asp_source_index_owner_lookup_idx
            ON asp_source_index_owner(owner_path, language_id, provider_id)",
        "CREATE INDEX IF NOT EXISTS asp_source_index_selector_owner_idx
            ON asp_source_index_selector(generation_id, owner_path)",
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

pub(super) async fn ensure_turso_source_index_owner_columns(
    connection: &turso::Connection,
) -> Result<(), String> {
    for (column, definition) in [
        ("language_id", "TEXT"),
        ("provider_id", "TEXT"),
        ("source_kind", "TEXT NOT NULL DEFAULT 'provider'"),
        ("line_count", "INTEGER"),
        ("query_keys_json", "TEXT NOT NULL DEFAULT '[]'"),
    ] {
        if !turso_table_column_exists(connection, "asp_source_index_owner", column).await? {
            let statement =
                format!("ALTER TABLE asp_source_index_owner ADD COLUMN {column} {definition}");
            execute_turso_statement_with_lock_retry(
                connection,
                statement.as_str(),
                "failed to migrate Turso source-index owner column",
            )
            .await?;
        }
    }
    Ok(())
}

pub(super) async fn ensure_turso_source_index_selector_columns(
    connection: &turso::Connection,
) -> Result<(), String> {
    for (column, definition) in [
        ("payload_kind", "TEXT"),
        ("payload_bounded", "INTEGER NOT NULL DEFAULT 0"),
    ] {
        if !turso_table_column_exists(connection, "asp_source_index_selector", column).await? {
            let statement =
                format!("ALTER TABLE asp_source_index_selector ADD COLUMN {column} {definition}");
            execute_turso_statement_with_lock_retry(
                connection,
                statement.as_str(),
                "failed to migrate Turso source-index selector column",
            )
            .await?;
        }
    }
    Ok(())
}

pub async fn refresh_turso_source_index_import(
    db_path: &Path,
    request: ClientDbSourceIndexRefreshRequest,
) -> Result<ClientDbSourceIndexRefreshReport, String> {
    let import = request.import;
    if import.file_hashes.is_empty() {
        return Err("source index import requires file hash evidence".to_string());
    }
    let _operation_lock = acquire_turso_operation_lock(db_path, "source-index-refresh")?;
    bootstrap_turso_client_db(db_path).await?;
    let connection = connect_turso_client_db(db_path).await?;
    bootstrap_turso_source_index_schema(&connection).await?;
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
        upsert_turso_source_index_search_documents(&connection, &import).await?;
        return Ok(refresh);
    }
    write_turso_source_index_rows(&connection, &import, &project_root, &file_hashes_json).await?;
    upsert_turso_source_index_search_documents(&connection, &import).await?;
    let (owner_count, selector_count) =
        turso_source_index_generation_row_counts(&connection, import.generation_id.as_str())
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

pub async fn latest_turso_source_index_scope_files(
    db_path: &Path,
    project_root: &Path,
    schema_id: &SemanticSchemaId,
    schema_version: &SemanticSchemaVersion,
) -> Result<Option<Vec<ClientDbSourceIndexScopeFile>>, String> {
    let Some((generation_id, _, _, _)) =
        latest_turso_source_index_generation(db_path, project_root, schema_id, schema_version)
            .await?
    else {
        return Ok(None);
    };
    let connection = connect_turso_client_db(db_path).await?;
    bootstrap_turso_source_index_schema(&connection).await?;
    let mut rows = run_turso_operation_with_lock_retry(
        || async {
            connection
                .query(
                    "SELECT owner_path, language_id, provider_id
                     FROM asp_source_index_owner
                     WHERE generation_id = ?1
                     ORDER BY owner_path",
                    [generation_id.as_str()],
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
        files.push(ClientDbSourceIndexScopeFile {
            path: PathBuf::from(row.get::<String>(0).map_err(|error| {
                format!("failed to read Turso source-index owner path: {error}")
            })?),
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
    bootstrap_turso_source_index_schema(&connection).await?;
    let mut rows = run_turso_operation_with_lock_retry(
        || async {
            connection
                .query(
                    "SELECT generation_id, owner_count, selector_count
                     FROM asp_source_index_generation
                     WHERE project_root = ?1
                       AND schema_id = ?2
                       AND schema_version = ?3
                       AND file_hashes_json = ?4
                     ORDER BY updated_at_ms DESC
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
    bootstrap_turso_source_index_schema(&connection).await?;
    let mut rows = run_turso_operation_with_lock_retry(
        || async {
            connection
                .query(
                    "SELECT generation_id, file_hashes_json, owner_count, selector_count
                     FROM asp_source_index_generation
                     WHERE project_root = ?1
                       AND schema_id = ?2
                       AND schema_version = ?3
                     ORDER BY updated_at_ms DESC
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
                     FROM asp_source_index_generation
                     WHERE project_root = ?1
                       AND schema_id = ?2
                       AND schema_version = ?3
                       AND file_hashes_json = ?4
                     ORDER BY updated_at_ms DESC
                     LIMIT 1",
                    (
                        project_root,
                        import.schema_id.as_str(),
                        import.schema_version.as_str(),
                        file_hashes_json,
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
    let (owner_count, selector_count) =
        turso_source_index_generation_row_counts(connection, &generation_id).await?;
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
    }))
}

async fn turso_source_index_generation_row_counts(
    connection: &turso::Connection,
    generation_id: &str,
) -> Result<(u32, u32), String> {
    Ok((
        count_turso_source_index_generation_rows(
            connection,
            "asp_source_index_owner",
            generation_id,
        )
        .await?,
        count_turso_source_index_generation_rows(
            connection,
            "asp_source_index_selector",
            generation_id,
        )
        .await?,
    ))
}

async fn count_turso_source_index_generation_rows(
    connection: &turso::Connection,
    table: &str,
    generation_id: &str,
) -> Result<u32, String> {
    let sql = format!("SELECT COUNT(*) FROM {table} WHERE generation_id = ?1");
    let mut rows = run_turso_operation_with_lock_retry(
        || async {
            connection
                .query(sql.as_str(), [generation_id])
                .await
                .map_err(|error| error.to_string())
        },
        &format!("failed to count Turso source-index generation rows in {table}"),
    )
    .await?;
    let Some(row) = rows.next().await.map_err(|error| {
        format!("failed to read Turso source-index generation row count in {table}: {error}")
    })?
    else {
        return Ok(0);
    };
    Ok(row
        .get::<i64>(0)
        .map_err(|error| {
            format!("failed to decode Turso source-index generation row count in {table}: {error}")
        })?
        .max(0)
        .min(i64::from(u32::MAX)) as u32)
}

async fn upsert_turso_source_index_search_documents(
    connection: &turso::Connection,
    import: &ClientDbSourceIndexImport,
) -> Result<usize, String> {
    let generation_id = import.generation_id.as_str();
    let documents = import
        .owners
        .iter()
        .map(|owner| {
            let owner_path = owner.owner_path.as_str();
            let selector = owner
                .language_id
                .as_ref()
                .map(|language_id| format!("{}://{}#file", language_id.as_str(), owner_path));
            let mut document_terms = vec![
                owner_path.to_string(),
                owner.source_kind.as_str().to_string(),
            ];
            if let Some(language_id) = &owner.language_id {
                document_terms.push(language_id.as_str().to_string());
            }
            if let Some(provider_id) = &owner.provider_id {
                document_terms.push(provider_id.as_str().to_string());
            }
            document_terms.extend(
                owner
                    .query_keys
                    .iter()
                    .map(|query_key| query_key.as_str().to_string()),
            );
            crate::TursoClientDbSearchDocument {
                namespace: "stable".to_string(),
                document_id: format!("source-index:{generation_id}:{owner_path}"),
                entity_id: owner_path.to_string(),
                selector,
                document: document_terms.join(" "),
            }
        })
        .collect::<Vec<_>>();
    crate::engine::turso_search::upsert_turso_search_documents_with_connection(
        connection, &documents,
    )
    .await
}

async fn write_turso_source_index_rows(
    connection: &turso::Connection,
    import: &ClientDbSourceIndexImport,
    project_root: &str,
    file_hashes_json: &str,
) -> Result<(), String> {
    execute_turso_statement_with_lock_retry(
        connection,
        "BEGIN IMMEDIATE",
        "failed to begin Turso source-index transaction",
    )
    .await?;
    let generation_id = import.generation_id.as_str();
    execute_turso_operation_with_lock_retry(
        || async {
            connection
                .execute(
                    "INSERT INTO asp_source_index_generation (
                        generation_id,
                        project_root,
                        schema_id,
                        schema_version,
                        file_hashes_json,
                        owner_count,
                        selector_count,
                        updated_at_ms
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                    ON CONFLICT(generation_id) DO UPDATE SET
                        project_root = excluded.project_root,
                        schema_id = excluded.schema_id,
                        schema_version = excluded.schema_version,
                        file_hashes_json = excluded.file_hashes_json,
                        owner_count = excluded.owner_count,
                        selector_count = excluded.selector_count,
                        updated_at_ms = excluded.updated_at_ms",
                    (
                        generation_id,
                        project_root,
                        import.schema_id.as_str(),
                        import.schema_version.as_str(),
                        file_hashes_json,
                        import.owners.len().min(i64::MAX as usize) as i64,
                        import.selectors.len().min(i64::MAX as usize) as i64,
                        unix_time_ms(),
                    ),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to write Turso source-index generation",
    )
    .await?;
    execute_turso_operation_with_lock_retry(
        || async {
            connection
                .execute(
                    "DELETE FROM asp_source_index_owner WHERE generation_id = ?1",
                    [generation_id],
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to clear Turso source-index owners",
    )
    .await?;
    execute_turso_operation_with_lock_retry(
        || async {
            connection
                .execute(
                    "DELETE FROM asp_source_index_selector WHERE generation_id = ?1",
                    [generation_id],
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to clear Turso source-index selectors",
    )
    .await?;
    for owner in &import.owners {
        let query_keys_json = serde_json::to_string(
            &owner
                .query_keys
                .iter()
                .map(|key| key.as_str())
                .collect::<Vec<_>>(),
        )
        .map_err(|error| format!("failed to encode Turso source-index owner keys: {error}"))?;
        execute_turso_operation_with_lock_retry(
            || async {
                connection
                    .execute(
                        "INSERT INTO asp_source_index_owner (
                            generation_id,
                            owner_path,
                            language_id,
                            provider_id,
                            source_kind,
                            line_count,
                            query_keys_json
                        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                        (
                            generation_id,
                            owner.owner_path.as_str(),
                            owner.language_id.as_ref().map(|value| value.as_str()),
                            owner.provider_id.as_ref().map(|value| value.as_str()),
                            owner.source_kind.as_str(),
                            owner.line_count.map(i64::from),
                            query_keys_json.as_str(),
                        ),
                    )
                    .await
                    .map_err(|error| error.to_string())
            },
            "failed to write Turso source-index owner",
        )
        .await?;
    }
    for selector in &import.selectors {
        if let Some(proof) = &selector.payload_proof
            && proof.structural_selector != selector.selector_id
        {
            return Err(format!(
                "source-index selector payload proof selector mismatch: selector_id={} proof={}",
                selector.selector_id, proof.structural_selector
            ));
        }
        let query_keys_json = serde_json::to_string(
            &selector
                .query_keys
                .iter()
                .map(|key| key.as_str())
                .collect::<Vec<_>>(),
        )
        .map_err(|error| format!("failed to encode Turso source-index selector keys: {error}"))?;
        execute_turso_operation_with_lock_retry(
            || async {
                connection
                    .execute(
                        "INSERT INTO asp_source_index_selector (
                            generation_id,
                            owner_path,
                            selector_id,
                            symbol,
                            kind,
                            start_line,
                            end_line,
                            source,
                            payload_kind,
                            payload_bounded,
                            query_keys_json
                        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                        (
                            generation_id,
                            selector.owner_path.as_str(),
                            selector.selector_id.as_str(),
                            selector.symbol.as_deref(),
                            selector.kind.as_deref(),
                            i64::from(selector.start_line),
                            i64::from(selector.end_line),
                            selector.source.as_str(),
                            selector
                                .payload_proof
                                .as_ref()
                                .map(|proof| proof.payload_kind.as_str()),
                            selector
                                .payload_proof
                                .as_ref()
                                .is_some_and(|proof| proof.bounded),
                            query_keys_json.as_str(),
                        ),
                    )
                    .await
                    .map_err(|error| error.to_string())
            },
            "failed to write Turso source-index selector",
        )
        .await?;
    }
    execute_turso_statement_with_lock_retry(
        connection,
        "COMMIT",
        "failed to commit Turso source-index transaction",
    )
    .await?;
    Ok(())
}

fn unix_time_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}
