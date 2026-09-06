// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Turso cache-generation read model adapter.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_client_core::{
    CacheArtifactId, CacheExportMethod, CacheGenerationId, ClientCacheFileHash,
    ClientCacheManifest, LanguageId, ProviderId,
};

use crate::types::{ClientDbGenerationHit, normalized_project_root};

use super::turso::connect_turso_client_db;
use super::turso_cache_key::{active_cache_generation_key, active_cache_lookup_key};
use super::turso_statement::{
    execute_turso_operation, execute_turso_prepared_statement_with_lock_retry,
    execute_turso_statement, run_turso_operation,
};

/// Bootstrap Turso cache-generation tables used by DB Engine replay lookup.
pub async fn bootstrap_turso_client_cache_schema(
    connection: &turso::Connection,
) -> Result<(), String> {
    for statement in [
        "CREATE TABLE IF NOT EXISTS asp_cache_generation (
            generation_id TEXT NOT NULL,
            language_id TEXT NOT NULL,
            provider_id TEXT NOT NULL,
            provider_version TEXT,
            export_method TEXT NOT NULL,
            project_root TEXT NOT NULL,
            package_root TEXT,
            schema_ids_json TEXT NOT NULL,
            cache_status TEXT NOT NULL,
            raw_source_stored INTEGER NOT NULL DEFAULT 0,
            request_fingerprint TEXT,
            artifact_ids_json TEXT NOT NULL DEFAULT '[]',
            file_hashes_json TEXT NOT NULL DEFAULT '[]',
            updated_at_ms INTEGER NOT NULL,
            PRIMARY KEY (
                project_root,
                language_id,
                provider_id,
                export_method,
                generation_id
            )
        )",
        "CREATE TABLE IF NOT EXISTS asp_cache_active_generation_v1 (
            lookup_key TEXT NOT NULL PRIMARY KEY,
            generation_key TEXT NOT NULL,
            project_root TEXT NOT NULL,
            language_id TEXT NOT NULL,
            provider_id TEXT NOT NULL,
            export_method TEXT NOT NULL,
            request_fingerprint TEXT NOT NULL,
            generation_id TEXT NOT NULL,
            schema_ids_json TEXT NOT NULL,
            artifact_ids_json TEXT NOT NULL DEFAULT '[]',
            file_hashes_json TEXT NOT NULL DEFAULT '[]',
            updated_at_ms INTEGER NOT NULL
        )",
        "CREATE UNIQUE INDEX IF NOT EXISTS asp_cache_active_generation_v1_generation_idx
            ON asp_cache_active_generation_v1(generation_key)",
        "CREATE INDEX IF NOT EXISTS asp_cache_generation_lookup_idx
            ON asp_cache_generation(language_id, provider_id, project_root, export_method, request_fingerprint, updated_at_ms)",
    ] {
        execute_turso_statement(
            connection,
            statement,
            "failed to bootstrap Turso cache schema",
        )
        .await?;
    }
    Ok(())
}

/// Insert or update cache generations in the active Turso read model.
pub async fn upsert_turso_cache_generations(
    db_path: &Path,
    manifest: &ClientCacheManifest,
) -> Result<usize, String> {
    if manifest.generations.is_empty() {
        return Ok(0);
    }
    let connection = connect_turso_client_db(db_path).await?;
    bootstrap_turso_client_cache_schema(&connection).await?;
    upsert_turso_cache_generations_with_connection(&connection, manifest).await
}

pub(super) async fn upsert_turso_cache_generations_with_connection(
    connection: &turso::Connection,
    manifest: &ClientCacheManifest,
) -> Result<usize, String> {
    use super::turso_batch::{
        TURSO_BATCH_ROW_COUNT, append_parameter_rows, optional_text_value, text_value,
    };

    async fn execute_value_batch(
        connection: &turso::Connection,
        sql: String,
        values: Vec<turso::Value>,
        context: &'static str,
    ) -> Result<(), String> {
        let mut statement = connection
            .prepare_cached(sql)
            .await
            .map_err(|error| format!("failed to prepare Turso value batch: {error}"))?;
        execute_turso_prepared_statement_with_lock_retry!(
            statement,
            turso::params_from_iter(values.clone()),
            context,
        )
        .map(|_| ())
    }

    if manifest
        .generations
        .iter()
        .any(|generation| generation.raw_source_stored)
    {
        return Err("Turso cache read model refuses rawSourceStored=true".to_string());
    }
    if manifest.generations.is_empty() {
        return Ok(0);
    }
    let updated_at_ms = current_timestamp_ms();
    let mut generation_rows = Vec::with_capacity(manifest.generations.len());
    let mut pointer_rows = Vec::with_capacity(manifest.generations.len());
    let mut pointer_delete_rows = Vec::new();
    for generation in &manifest.generations {
        let project_root = normalized_project_root(Path::new(&generation.project_root))?;
        let export_method = generation.export_method.as_deref().ok_or_else(|| {
            format!(
                "Turso cache generation `{}` has no export method",
                generation.generation_id
            )
        })?;
        let schema_ids_json = serde_json::to_string(&generation.schema_ids)
            .map_err(|error| format!("failed to serialize Turso cache schema ids: {error}"))?;
        let artifact_ids_json = serde_json::to_string(
            generation.artifact_ids.as_deref().unwrap_or(&[]),
        )
        .map_err(|error| format!("failed to serialize Turso cache artifact ids: {error}"))?;
        let file_hashes_json =
            serde_json::to_string(generation.file_hashes.as_deref().unwrap_or(&[]))
                .map_err(|error| format!("failed to serialize Turso cache file hashes: {error}"))?;
        generation_rows.push(vec![
            text_value(generation.generation_id.as_str()),
            text_value(generation.language_id.as_str()),
            text_value(generation.provider_id.as_str()),
            optional_text_value(generation.provider_version.as_deref()),
            text_value(export_method),
            text_value(project_root.as_str()),
            optional_text_value(generation.package_root.as_deref()),
            text_value(schema_ids_json.as_str()),
            text_value(generation.cache_status.as_str()),
            turso::Value::Integer(0),
            optional_text_value(generation.request_fingerprint.as_deref()),
            text_value(artifact_ids_json.as_str()),
            text_value(file_hashes_json.as_str()),
            turso::Value::Integer(updated_at_ms),
        ]);
        let generation_key = active_cache_generation_key(
            project_root.as_str(),
            generation.language_id.as_str(),
            generation.provider_id.as_str(),
            export_method,
            &generation.generation_id,
        );
        if let Some(request_fingerprint) = generation.request_fingerprint.as_deref() {
            let lookup_key = active_cache_lookup_key(
                project_root.as_str(),
                generation.language_id.as_str(),
                generation.provider_id.as_str(),
                export_method,
                request_fingerprint,
            );
            pointer_rows.push(vec![
                text_value(lookup_key.as_str()),
                text_value(generation_key.as_str()),
                text_value(project_root.as_str()),
                text_value(generation.language_id.as_str()),
                text_value(generation.provider_id.as_str()),
                text_value(export_method),
                text_value(request_fingerprint),
                text_value(generation.generation_id.as_str()),
                text_value(schema_ids_json.as_str()),
                text_value(artifact_ids_json.as_str()),
                text_value(file_hashes_json.as_str()),
                turso::Value::Integer(updated_at_ms),
            ]);
        } else {
            pointer_delete_rows.push(vec![text_value(generation_key.as_str())]);
        }
    }
    execute_turso_statement(
        connection,
        "BEGIN TRANSACTION",
        "failed to begin Turso cache generation transaction",
    )
    .await?;
    for rows in generation_rows.chunks(TURSO_BATCH_ROW_COUNT) {
        let mut sql = String::from(
            "INSERT INTO asp_cache_generation (
                generation_id,
                language_id,
                provider_id,
                provider_version,
                export_method,
                project_root,
                package_root,
                schema_ids_json,
                cache_status,
                raw_source_stored,
                request_fingerprint,
                artifact_ids_json,
                file_hashes_json,
                updated_at_ms
            ) VALUES ",
        );
        append_parameter_rows(&mut sql, rows.len(), 14);
        sql.push_str(
            " ON CONFLICT(
                project_root,
                language_id,
                provider_id,
                export_method,
                generation_id
            ) DO UPDATE SET
                provider_version = excluded.provider_version,
                package_root = excluded.package_root,
                schema_ids_json = excluded.schema_ids_json,
                cache_status = excluded.cache_status,
                raw_source_stored = excluded.raw_source_stored,
                request_fingerprint = excluded.request_fingerprint,
                artifact_ids_json = excluded.artifact_ids_json,
                file_hashes_json = excluded.file_hashes_json,
                updated_at_ms = excluded.updated_at_ms",
        );
        let values = rows.iter().flatten().cloned().collect();
        if let Err(error) = execute_value_batch(
            connection,
            sql,
            values,
            "failed to batch upsert Turso cache generations",
        )
        .await
        {
            let _ = execute_turso_statement(
                connection,
                "ROLLBACK",
                "failed to rollback Turso cache generation transaction after batch upsert",
            )
            .await;
            return Err(error);
        }
    }
    for rows in pointer_rows.chunks(TURSO_BATCH_ROW_COUNT) {
        let mut sql = String::from(
            "INSERT OR REPLACE INTO asp_cache_active_generation_v1 (
                lookup_key,
                generation_key,
                project_root,
                language_id,
                provider_id,
                export_method,
                request_fingerprint,
                generation_id,
                schema_ids_json,
                artifact_ids_json,
                file_hashes_json,
                updated_at_ms
             ) VALUES ",
        );
        append_parameter_rows(&mut sql, rows.len(), 12);
        let values = rows.iter().flatten().cloned().collect();
        if let Err(error) = execute_value_batch(
            connection,
            sql,
            values,
            "failed to batch upsert Turso active-generation pointers",
        )
        .await
        {
            let _ = execute_turso_statement(
                connection,
                "ROLLBACK",
                "failed to rollback Turso cache generation transaction after pointer batch upsert",
            )
            .await;
            return Err(error);
        }
    }
    for rows in pointer_delete_rows.chunks(TURSO_BATCH_ROW_COUNT) {
        let mut sql = String::from(
            "DELETE FROM asp_cache_active_generation_v1
             WHERE generation_key IN (",
        );
        for index in 0..rows.len() {
            if index > 0 {
                sql.push_str(", ");
            }
            sql.push('?');
            sql.push_str(&(index + 1).to_string());
        }
        sql.push(')');
        let values = rows.iter().flatten().cloned().collect();
        if let Err(error) = execute_value_batch(
            connection,
            sql,
            values,
            "failed to batch delete Turso active-generation pointers without fingerprints",
        )
        .await
        {
            let _ = execute_turso_statement(
                connection,
                "ROLLBACK",
                "failed to rollback Turso cache generation transaction after pointer batch delete",
            )
            .await;
            return Err(error);
        }
    }
    execute_turso_statement(
        connection,
        "COMMIT",
        "failed to commit Turso cache generation transaction",
    )
    .await?;
    Ok(manifest.generations.len())
}

/// Clear cache-generation metadata from the active Turso read model.
pub async fn clear_turso_cache_generations(db_path: &Path) -> Result<(), String> {
    let connection = connect_turso_client_db(db_path).await?;
    bootstrap_turso_client_cache_schema(&connection).await?;
    execute_turso_statement(
        &connection,
        "BEGIN TRANSACTION",
        "failed to begin Turso cache clear transaction",
    )
    .await?;
    if let Err(error) = execute_turso_statement(
        &connection,
        "DELETE FROM asp_cache_active_generation_v1",
        "failed to clear Turso active-generation pointers",
    )
    .await
    {
        let _ = execute_turso_statement(
            &connection,
            "ROLLBACK",
            "failed to rollback Turso cache clear after pointer delete",
        )
        .await;
        return Err(error);
    }
    if let Err(error) = execute_turso_statement(
        &connection,
        "DELETE FROM asp_cache_generation",
        "failed to clear Turso cache generations",
    )
    .await
    {
        let _ = execute_turso_statement(
            &connection,
            "ROLLBACK",
            "failed to rollback Turso cache clear after generation delete",
        )
        .await;
        return Err(error);
    }
    execute_turso_statement(
        &connection,
        "COMMIT",
        "failed to commit Turso cache clear transaction",
    )
    .await?;
    Ok(())
}

/// Delete Turso cache-generation rows absent from the current manifest.
pub async fn prune_turso_cache_generations_to_manifest(
    db_path: &Path,
    manifest: &ClientCacheManifest,
) -> Result<(), String> {
    if manifest.generations.is_empty() {
        return clear_turso_cache_generations(db_path).await;
    }
    let connection = connect_turso_client_db(db_path).await?;
    bootstrap_turso_client_cache_schema(&connection).await?;
    let keep_keys = manifest_cache_generation_keys(manifest)?;
    let delete_keys = cache_generation_delete_keys(&connection, &keep_keys).await?;
    if delete_keys.is_empty() {
        return Ok(());
    }
    delete_cache_generation_keys(&connection, delete_keys).await
}

type CacheGenerationPruneKey = (String, String, String, String, CacheGenerationId);

fn manifest_cache_generation_keys(
    manifest: &ClientCacheManifest,
) -> Result<std::collections::HashSet<CacheGenerationPruneKey>, String> {
    manifest
        .generations
        .iter()
        .filter_map(|generation| {
            generation
                .export_method
                .as_deref()
                .map(|export_method| (generation, export_method))
        })
        .map(|(generation, export_method)| {
            Ok((
                normalized_project_root(Path::new(&generation.project_root))?,
                generation.language_id.as_str().to_owned(),
                generation.provider_id.as_str().to_owned(),
                export_method.to_owned(),
                generation.generation_id.clone(),
            ))
        })
        .collect()
}

async fn cache_generation_delete_keys(
    connection: &turso::Connection,
    keep_keys: &std::collections::HashSet<CacheGenerationPruneKey>,
) -> Result<Vec<CacheGenerationPruneKey>, String> {
    let mut rows = run_turso_operation(
        || async {
            connection
                .query(
                    "SELECT project_root,
                            language_id,
                            provider_id,
                            export_method,
                            generation_id
                     FROM asp_cache_generation",
                    (),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to list Turso cache generations for prune",
    )
    .await?;
    let mut delete_keys = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso cache generation for prune: {error}"))?
    {
        let key = (
            row.get::<String>(0).map_err(|error| {
                format!("failed to decode Turso cache project root for prune: {error}")
            })?,
            row.get::<String>(1).map_err(|error| {
                format!("failed to decode Turso cache language id for prune: {error}")
            })?,
            row.get::<String>(2).map_err(|error| {
                format!("failed to decode Turso cache provider id for prune: {error}")
            })?,
            row.get::<String>(3).map_err(|error| {
                format!("failed to decode Turso cache export method for prune: {error}")
            })?,
            CacheGenerationId::from(row.get::<String>(4).map_err(|error| {
                format!("failed to decode Turso cache generation id for prune: {error}")
            })?),
        );
        if !keep_keys.contains(&key) {
            delete_keys.push(key);
        }
    }
    Ok(delete_keys)
}

async fn delete_cache_generation_keys(
    connection: &turso::Connection,
    delete_keys: Vec<CacheGenerationPruneKey>,
) -> Result<(), String> {
    execute_turso_statement(
        connection,
        "BEGIN TRANSACTION",
        "failed to begin Turso cache prune transaction",
    )
    .await?;
    let mut pointer_statement = match connection
        .prepare_cached(
            "DELETE FROM asp_cache_active_generation_v1
             WHERE generation_key = ?1",
        )
        .await
    {
        Ok(statement) => statement,
        Err(error) => {
            let _ = execute_turso_statement(
                connection,
                "ROLLBACK",
                "failed to rollback Turso cache prune transaction after prepare",
            )
            .await;
            return Err(format!(
                "failed to prepare Turso active-generation pointer prune: {error}"
            ));
        }
    };
    let mut generation_statement = match connection
        .prepare_cached(
            "DELETE FROM asp_cache_generation
             WHERE project_root = ?1
               AND language_id = ?2
               AND provider_id = ?3
               AND export_method = ?4
               AND generation_id = ?5",
        )
        .await
    {
        Ok(statement) => statement,
        Err(error) => {
            let _ = execute_turso_statement(
                connection,
                "ROLLBACK",
                "failed to rollback Turso cache prune transaction after generation prepare",
            )
            .await;
            return Err(format!(
                "failed to prepare Turso cache generation prune: {error}"
            ));
        }
    };
    for (project_root, language_id, provider_id, export_method, generation_id) in delete_keys {
        let generation_key = active_cache_generation_key(
            &project_root,
            &language_id,
            &provider_id,
            &export_method,
            &generation_id,
        );
        if let Err(error) = execute_turso_prepared_statement_with_lock_retry!(
            pointer_statement,
            [generation_key.as_str()],
            "failed to prune Turso active-generation pointer",
        ) {
            let _ = execute_turso_statement(
                connection,
                "ROLLBACK",
                "failed to rollback Turso cache prune transaction after pointer delete",
            )
            .await;
            return Err(error);
        }
        if let Err(error) = execute_turso_prepared_statement_with_lock_retry!(
            generation_statement,
            (
                project_root.as_str(),
                language_id.as_str(),
                provider_id.as_str(),
                export_method.as_str(),
                generation_id.as_str(),
            ),
            "failed to prune Turso cache generation",
        ) {
            let _ = execute_turso_statement(
                connection,
                "ROLLBACK",
                "failed to rollback Turso cache prune transaction after delete",
            )
            .await;
            return Err(error);
        }
    }
    execute_turso_statement(
        connection,
        "COMMIT",
        "failed to commit Turso cache prune transaction",
    )
    .await?;
    Ok(())
}

/// Delete Turso cache-generation rows for one normalized project root.
pub async fn invalidate_turso_cache_generations_for_project(
    db_path: &Path,
    project_root: &Path,
) -> Result<u32, String> {
    let initial_connection = async {
        let connection = connect_turso_client_db(db_path).await?;
        bootstrap_turso_client_cache_schema(&connection).await?;
        Ok::<_, String>(connection)
    }
    .await;
    let connection = match initial_connection {
        Ok(connection) => connection,
        Err(open_or_bootstrap_error) => {
            reset_corrupt_turso_cache_files(db_path).map_err(|reset_error| {
                format!(
                    "failed to reset derived Turso cache after open/bootstrap error `{open_or_bootstrap_error}`: {reset_error}"
                )
            })?;
            let connection = connect_turso_client_db(db_path).await.map_err(|retry_error| {
                format!(
                    "failed to recreate derived Turso cache after open/bootstrap error `{open_or_bootstrap_error}`: {retry_error}"
                )
            })?;
            bootstrap_turso_client_cache_schema(&connection)
                .await
                .map_err(|retry_error| {
                    format!(
                        "failed to bootstrap recreated Turso cache after open/bootstrap error `{open_or_bootstrap_error}`: {retry_error}"
                    )
                })?;
            connection
        }
    };
    invalidate_turso_cache_generations_for_project_with_connection(&connection, project_root).await
}

pub(super) async fn invalidate_turso_cache_generations_for_project_with_connection(
    connection: &turso::Connection,
    project_root: &Path,
) -> Result<u32, String> {
    let project_root = normalized_project_root(project_root)?;
    execute_turso_statement(
        connection,
        "BEGIN TRANSACTION",
        "failed to begin Turso project cache invalidation transaction",
    )
    .await?;
    if let Err(error) = execute_turso_operation(
        || async {
            connection
                .execute(
                    "DELETE FROM asp_cache_active_generation_v1 WHERE project_root = ?1",
                    [project_root.as_str()],
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to invalidate Turso active-generation pointers",
    )
    .await
    {
        let _ = execute_turso_statement(
            connection,
            "ROLLBACK",
            "failed to rollback Turso project cache invalidation after pointer delete",
        )
        .await;
        return Err(error);
    }
    let count = match execute_turso_operation(
        || async {
            connection
                .execute(
                    "DELETE FROM asp_cache_generation WHERE project_root = ?1",
                    [project_root.as_str()],
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to invalidate Turso cache generations",
    )
    .await
    {
        Ok(count) => count,
        Err(error) => {
            let _ = execute_turso_statement(
                connection,
                "ROLLBACK",
                "failed to rollback Turso project cache invalidation after generation delete",
            )
            .await;
            return Err(error);
        }
    };
    execute_turso_statement(
        connection,
        "COMMIT",
        "failed to commit Turso project cache invalidation transaction",
    )
    .await?;
    Ok(count.min(u64::from(u32::MAX)) as u32)
}

fn reset_corrupt_turso_cache_files(db_path: &Path) -> Result<(), String> {
    let mut paths = vec![db_path.to_path_buf()];
    for suffix in ["-wal", "-shm", "-tshm"] {
        let mut sidecar = db_path.as_os_str().to_os_string();
        sidecar.push(suffix);
        paths.push(std::path::PathBuf::from(sidecar));
    }
    for path in paths {
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!("failed to remove `{}`: {error}", path.display()));
            }
        }
    }
    Ok(())
}

/// Return recent matching cache generations from the active Turso read model.
pub async fn lookup_recent_turso_cache_generations(
    db_path: &Path,
    language_id: &LanguageId,
    provider_id: &ProviderId,
    project_root: &Path,
    export_method: &CacheExportMethod,
    request_fingerprint: Option<&str>,
    limit: u32,
) -> Result<Vec<ClientDbGenerationHit>, String> {
    if limit == 0 || !db_path.exists() {
        return Ok(Vec::new());
    }
    let connection = connect_turso_client_db(db_path).await?;
    lookup_recent_turso_cache_generations_with_connection(
        &connection,
        language_id,
        provider_id,
        project_root,
        export_method,
        request_fingerprint,
        limit,
    )
    .await
}

pub(super) async fn lookup_recent_turso_cache_generations_with_connection(
    connection: &turso::Connection,
    language_id: &LanguageId,
    provider_id: &ProviderId,
    project_root: &Path,
    export_method: &CacheExportMethod,
    request_fingerprint: Option<&str>,
    limit: u32,
) -> Result<Vec<ClientDbGenerationHit>, String> {
    if limit == 0 {
        return Ok(Vec::new());
    }
    let project_root = normalized_project_root(project_root)?;
    if let Some(request_fingerprint) = request_fingerprint {
        maybe_report_turso_cache_generation_query_plan(
            connection,
            language_id,
            provider_id,
            &project_root,
            export_method,
            request_fingerprint,
            limit,
        )
        .await?;
    }
    let lookup_key = request_fingerprint.map(|request_fingerprint| {
        active_cache_lookup_key(
            &project_root,
            language_id.as_str(),
            provider_id.as_str(),
            export_method.as_str(),
            request_fingerprint,
        )
    });
    let sql = if request_fingerprint.is_some() {
        "SELECT language_id,
                provider_id,
                project_root,
                export_method,
                schema_ids_json,
                request_fingerprint,
                artifact_ids_json,
                file_hashes_json
         FROM asp_cache_active_generation_v1
         WHERE lookup_key = ?1
         LIMIT ?2"
    } else {
        "SELECT language_id,
                provider_id,
                project_root,
                export_method,
                schema_ids_json,
                request_fingerprint,
                artifact_ids_json,
                file_hashes_json
         FROM asp_cache_generation
         WHERE language_id = ?1
           AND provider_id = ?2
           AND project_root = ?3
           AND export_method = ?4
           AND raw_source_stored = 0
         ORDER BY updated_at_ms DESC
         LIMIT ?5"
    };
    let mut statement = connection
        .prepare_cached(sql)
        .await
        .map_err(|error| format!("failed to prepare Turso cache generation lookup: {error}"))?;
    let mut rows = match request_fingerprint {
        Some(_) => {
            statement
                .query((
                    lookup_key
                        .as_deref()
                        .ok_or_else(|| "missing Turso active lookup key".to_string())?,
                    i64::from(limit),
                ))
                .await
        }
        None => {
            statement
                .query((
                    language_id.as_str(),
                    provider_id.as_str(),
                    project_root.as_str(),
                    export_method.as_str(),
                    i64::from(limit),
                ))
                .await
        }
    }
    .map_err(|error| format!("failed to query Turso cache generations: {error}"))?;
    let mut hits = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso cache generation row: {error}"))?
    {
        hits.push(turso_cache_generation_hit_from_row(
            TursoCacheGenerationHitColumns {
                language_id: row
                    .get::<String>(0)
                    .map_err(|error| format!("failed to read Turso cache language id: {error}"))?,
                provider_id: row
                    .get::<String>(1)
                    .map_err(|error| format!("failed to read Turso cache provider id: {error}"))?,
                project_root: row
                    .get::<String>(2)
                    .map_err(|error| format!("failed to read Turso cache project root: {error}"))?,
                export_method: row.get::<String>(3).map_err(|error| {
                    format!("failed to read Turso cache export method: {error}")
                })?,
                schema_ids_json: row
                    .get::<String>(4)
                    .map_err(|error| format!("failed to read Turso cache schema ids: {error}"))?,
                request_fingerprint: row
                    .get::<Option<String>>(5)
                    .map_err(|error| format!("failed to read Turso cache fingerprint: {error}"))?,
                artifact_ids_json: row
                    .get::<String>(6)
                    .map_err(|error| format!("failed to read Turso cache artifact ids: {error}"))?,
                file_hashes_json: row
                    .get::<String>(7)
                    .map_err(|error| format!("failed to read Turso cache file hashes: {error}"))?,
            },
        )?);
    }
    Ok(hits)
}

async fn maybe_report_turso_cache_generation_query_plan(
    connection: &turso::Connection,
    language_id: &LanguageId,
    provider_id: &ProviderId,
    project_root: &str,
    export_method: &CacheExportMethod,
    request_fingerprint: &str,
    limit: u32,
) -> Result<(), String> {
    static REPORTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if std::env::var_os("ASP_TURSO_EXPLAIN_CACHE_LOOKUP").is_none()
        || REPORTED.swap(true, std::sync::atomic::Ordering::AcqRel)
    {
        return Ok(());
    }
    let lookup_key = active_cache_lookup_key(
        project_root,
        language_id.as_str(),
        provider_id.as_str(),
        export_method.as_str(),
        request_fingerprint,
    );
    let mut rows = connection
        .query(
            "EXPLAIN QUERY PLAN
             SELECT language_id,
                    provider_id,
                    project_root,
                    export_method,
                    schema_ids_json,
                    request_fingerprint,
                    artifact_ids_json,
                    file_hashes_json
             FROM asp_cache_active_generation_v1
             WHERE lookup_key = ?1
             LIMIT ?2",
            (lookup_key.as_str(), i64::from(limit)),
        )
        .await
        .map_err(|error| format!("failed to explain Turso cache lookup: {error}"))?;
    let mut details = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso cache query plan: {error}"))?
    {
        details.push(
            row.get::<String>(3)
                .map_err(|error| format!("failed to decode Turso cache query plan: {error}"))?,
        );
    }
    eprintln!(
        "[turso-cache-query-plan] {}",
        serde_json::json!({
            "schemaId": "agent.semantic-protocols.turso-cache-query-plan",
            "schemaVersion": "1",
            "details": details,
        })
    );
    Ok(())
}

struct TursoCacheGenerationHitColumns {
    language_id: String,
    provider_id: String,
    project_root: String,
    export_method: String,
    schema_ids_json: String,
    request_fingerprint: Option<String>,
    artifact_ids_json: String,
    file_hashes_json: String,
}

fn turso_cache_generation_hit_from_row(
    columns: TursoCacheGenerationHitColumns,
) -> Result<ClientDbGenerationHit, String> {
    let schema_ids = serde_json::from_str(&columns.schema_ids_json)
        .map_err(|error| format!("failed to parse Turso cache schema ids: {error}"))?;
    let artifact_ids = serde_json::from_str::<Vec<CacheArtifactId>>(&columns.artifact_ids_json)
        .map_err(|error| format!("failed to parse Turso cache artifact ids: {error}"))?;
    let file_hashes =
        serde_json::from_str::<Vec<ClientCacheFileHash>>(&columns.file_hashes_json)
            .map_err(|error| format!("failed to parse Turso cache file hashes: {error}"))?;
    Ok(ClientDbGenerationHit {
        language_id: LanguageId::from(columns.language_id),
        provider_id: ProviderId::from(columns.provider_id),
        project_root: PathBuf::from(columns.project_root),
        export_method: CacheExportMethod::from(columns.export_method),
        schema_ids,
        request_fingerprint: columns.request_fingerprint,
        file_hashes,
        artifact_ids,
    })
}

fn current_timestamp_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}
