//! Turso cache-generation read model adapter.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_client_core::{
    CacheArtifactId, CacheExportMethod, CacheGenerationId, ClientCacheFileHash,
    ClientCacheManifest, LanguageId, ProviderId,
};

use crate::types::{ClientDbGenerationHit, normalized_project_root};

use super::turso::connect_turso_client_db;
use super::turso_operation_lock::acquire_turso_operation_lock;
use super::turso_statement::{
    execute_turso_operation_with_lock_retry, execute_turso_prepared_statement_with_lock_retry,
    execute_turso_statement_with_lock_retry, run_turso_operation_with_lock_retry,
};

/// Bootstrap Turso cache-generation tables used by DB Engine replay lookup.
pub async fn bootstrap_turso_client_cache_schema(
    connection: &turso::Connection,
) -> Result<(), String> {
    for statement in [
        "CREATE TABLE IF NOT EXISTS asp_cache_generation (
            generation_id TEXT PRIMARY KEY,
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
            updated_at_ms INTEGER NOT NULL
        )",
        "CREATE INDEX IF NOT EXISTS asp_cache_generation_lookup_idx
            ON asp_cache_generation(language_id, provider_id, project_root, export_method, request_fingerprint, updated_at_ms)",
    ] {
        execute_turso_statement_with_lock_retry(
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
    let _operation_lock = acquire_turso_operation_lock(db_path, "cache-generation-upsert")?;
    let connection = connect_turso_client_db(db_path).await?;
    bootstrap_turso_client_cache_schema(&connection).await?;
    execute_turso_statement_with_lock_retry(
        &connection,
        "BEGIN TRANSACTION",
        "failed to begin Turso cache generation transaction",
    )
    .await?;
    let mut statement = match connection
        .prepare_cached(
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
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0, ?10, ?11, ?12, ?13)
            ON CONFLICT(generation_id) DO UPDATE SET
                language_id = excluded.language_id,
                provider_id = excluded.provider_id,
                provider_version = excluded.provider_version,
                export_method = excluded.export_method,
                project_root = excluded.project_root,
                package_root = excluded.package_root,
                schema_ids_json = excluded.schema_ids_json,
                cache_status = excluded.cache_status,
                raw_source_stored = excluded.raw_source_stored,
                request_fingerprint = excluded.request_fingerprint,
                artifact_ids_json = excluded.artifact_ids_json,
                file_hashes_json = excluded.file_hashes_json,
                updated_at_ms = excluded.updated_at_ms",
        )
        .await
    {
        Ok(statement) => statement,
        Err(error) => {
            let _ = execute_turso_statement_with_lock_retry(
                &connection,
                "ROLLBACK",
                "failed to rollback Turso cache generation transaction after prepare",
            )
            .await;
            return Err(format!(
                "failed to prepare Turso cache generation upsert: {error}"
            ));
        }
    };
    let updated_at_ms = current_timestamp_ms();
    for generation in &manifest.generations {
        let project_root = normalized_project_root(Path::new(&generation.project_root));
        let schema_ids_json = serde_json::to_string(&generation.schema_ids)
            .map_err(|error| format!("failed to serialize Turso cache schema ids: {error}"))?;
        let artifact_ids_json = serde_json::to_string(
            generation.artifact_ids.as_deref().unwrap_or(&[]),
        )
        .map_err(|error| format!("failed to serialize Turso cache artifact ids: {error}"))?;
        let file_hashes_json =
            serde_json::to_string(generation.file_hashes.as_deref().unwrap_or(&[]))
                .map_err(|error| format!("failed to serialize Turso cache file hashes: {error}"))?;
        if let Err(error) = execute_turso_prepared_statement_with_lock_retry!(
            statement,
            (
                generation.generation_id.as_str(),
                generation.language_id.as_str(),
                generation.provider_id.as_str(),
                generation.provider_version.as_deref(),
                generation.export_method.as_deref(),
                project_root.as_str(),
                generation.package_root.as_deref(),
                schema_ids_json.as_str(),
                generation.cache_status.as_str(),
                generation.request_fingerprint.as_deref(),
                artifact_ids_json.as_str(),
                file_hashes_json.as_str(),
                updated_at_ms,
            ),
            "failed to upsert Turso cache generation",
        ) {
            let _ = execute_turso_statement_with_lock_retry(
                &connection,
                "ROLLBACK",
                "failed to rollback Turso cache generation transaction after upsert",
            )
            .await;
            return Err(error);
        }
    }
    execute_turso_statement_with_lock_retry(
        &connection,
        "COMMIT",
        "failed to commit Turso cache generation transaction",
    )
    .await?;
    Ok(manifest.generations.len())
}

/// Clear cache-generation metadata from the active Turso read model.
pub async fn clear_turso_cache_generations(db_path: &Path) -> Result<(), String> {
    let _operation_lock = acquire_turso_operation_lock(db_path, "cache-generation-clear")?;
    let connection = connect_turso_client_db(db_path).await?;
    bootstrap_turso_client_cache_schema(&connection).await?;
    execute_turso_operation_with_lock_retry(
        || async {
            connection
                .execute("DELETE FROM asp_cache_generation", ())
                .await
                .map_err(|error| error.to_string())
        },
        "failed to clear Turso cache generations",
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
    let _operation_lock = acquire_turso_operation_lock(db_path, "cache-generation-prune")?;
    let connection = connect_turso_client_db(db_path).await?;
    bootstrap_turso_client_cache_schema(&connection).await?;
    let keep_ids = manifest
        .generations
        .iter()
        .map(|generation| generation.generation_id.clone())
        .collect::<std::collections::HashSet<CacheGenerationId>>();
    let mut rows = run_turso_operation_with_lock_retry(
        || async {
            connection
                .query("SELECT generation_id FROM asp_cache_generation", ())
                .await
                .map_err(|error| error.to_string())
        },
        "failed to list Turso cache generations for prune",
    )
    .await?;
    let mut delete_ids = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso cache generation for prune: {error}"))?
    {
        let generation_id = CacheGenerationId::from(row.get::<String>(0).map_err(|error| {
            format!("failed to decode Turso cache generation id for prune: {error}")
        })?);
        if !keep_ids.contains(&generation_id) {
            delete_ids.push(generation_id);
        }
    }
    if delete_ids.is_empty() {
        return Ok(());
    }
    execute_turso_statement_with_lock_retry(
        &connection,
        "BEGIN TRANSACTION",
        "failed to begin Turso cache prune transaction",
    )
    .await?;
    let mut statement = match connection
        .prepare_cached("DELETE FROM asp_cache_generation WHERE generation_id = ?1")
        .await
    {
        Ok(statement) => statement,
        Err(error) => {
            let _ = execute_turso_statement_with_lock_retry(
                &connection,
                "ROLLBACK",
                "failed to rollback Turso cache prune transaction after prepare",
            )
            .await;
            return Err(format!(
                "failed to prepare Turso cache generation prune: {error}"
            ));
        }
    };
    for generation_id in delete_ids {
        if let Err(error) = execute_turso_prepared_statement_with_lock_retry!(
            statement,
            [generation_id.as_str()],
            "failed to prune Turso cache generation",
        ) {
            let _ = execute_turso_statement_with_lock_retry(
                &connection,
                "ROLLBACK",
                "failed to rollback Turso cache prune transaction after delete",
            )
            .await;
            return Err(error);
        }
    }
    execute_turso_statement_with_lock_retry(
        &connection,
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
    let _operation_lock = acquire_turso_operation_lock(db_path, "cache-generation-invalidate")?;
    let connection = connect_turso_client_db(db_path).await?;
    bootstrap_turso_client_cache_schema(&connection).await?;
    let project_root = normalized_project_root(project_root);
    let count = execute_turso_operation_with_lock_retry(
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
    .await?;
    Ok(count.min(u64::from(u32::MAX)) as u32)
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
    bootstrap_turso_client_cache_schema(&connection).await?;
    let project_root = normalized_project_root(project_root);
    let mut rows = run_turso_operation_with_lock_retry(
        || async {
            connection
                .query(
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
                       AND (?5 IS NULL OR request_fingerprint = ?5)
                       AND raw_source_stored = 0
                     ORDER BY updated_at_ms DESC
                     LIMIT ?6",
                    (
                        language_id.as_str(),
                        provider_id.as_str(),
                        project_root.as_str(),
                        export_method.as_str(),
                        request_fingerprint,
                        i64::from(limit),
                    ),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to query Turso cache generations",
    )
    .await?;
    let mut hits = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso cache generation row: {error}"))?
    {
        hits.push(turso_cache_generation_hit_from_row(
            row.get::<String>(0)
                .map_err(|error| format!("failed to read Turso cache language id: {error}"))?,
            row.get::<String>(1)
                .map_err(|error| format!("failed to read Turso cache provider id: {error}"))?,
            row.get::<String>(2)
                .map_err(|error| format!("failed to read Turso cache project root: {error}"))?,
            row.get::<String>(3)
                .map_err(|error| format!("failed to read Turso cache export method: {error}"))?,
            row.get::<String>(4)
                .map_err(|error| format!("failed to read Turso cache schema ids: {error}"))?,
            row.get::<Option<String>>(5)
                .map_err(|error| format!("failed to read Turso cache fingerprint: {error}"))?,
            row.get::<String>(6)
                .map_err(|error| format!("failed to read Turso cache artifact ids: {error}"))?,
            row.get::<String>(7)
                .map_err(|error| format!("failed to read Turso cache file hashes: {error}"))?,
        )?);
    }
    Ok(hits)
}

#[allow(clippy::too_many_arguments)]
fn turso_cache_generation_hit_from_row(
    language_id: String,
    provider_id: String,
    project_root: String,
    export_method: String,
    schema_ids_json: String,
    request_fingerprint: Option<String>,
    artifact_ids_json: String,
    file_hashes_json: String,
) -> Result<ClientDbGenerationHit, String> {
    let schema_ids = serde_json::from_str(&schema_ids_json)
        .map_err(|error| format!("failed to parse Turso cache schema ids: {error}"))?;
    let artifact_ids = serde_json::from_str::<Vec<CacheArtifactId>>(&artifact_ids_json)
        .map_err(|error| format!("failed to parse Turso cache artifact ids: {error}"))?;
    let file_hashes = serde_json::from_str::<Vec<ClientCacheFileHash>>(&file_hashes_json)
        .map_err(|error| format!("failed to parse Turso cache file hashes: {error}"))?;
    Ok(ClientDbGenerationHit {
        language_id: LanguageId::from(language_id),
        provider_id: ProviderId::from(provider_id),
        project_root: PathBuf::from(project_root),
        export_method: CacheExportMethod::from(export_method),
        schema_ids,
        request_fingerprint,
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
