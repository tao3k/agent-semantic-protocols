//! Turso syntax-query replay read model adapter.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::syntax_query::syntax_query_replay_and_selector_from_packet_import;
use crate::types::{ClientDbSyntaxQueryLookup, ClientDbSyntaxQueryReplay};

use super::turso::connect_turso_client_db;
use super::turso_operation_lock::acquire_turso_operation_lock;
use super::turso_statement::{
    execute_turso_operation_with_lock_retry, execute_turso_statement_with_lock_retry,
    run_turso_operation_with_lock_retry,
};

/// Bootstrap Turso syntax replay table used by DB Engine syntax replay lookup.
pub async fn bootstrap_turso_syntax_query_schema(
    connection: &turso::Connection,
) -> Result<(), String> {
    for statement in [
        "CREATE TABLE IF NOT EXISTS asp_syntax_query_replay (
            generation_id TEXT PRIMARY KEY,
            language_id TEXT NOT NULL,
            provider_id TEXT NOT NULL,
            project_root TEXT NOT NULL,
            query_ast_fingerprint TEXT NOT NULL,
            selector TEXT,
            replay_json TEXT NOT NULL,
            updated_at_ms INTEGER NOT NULL
        )",
        "CREATE INDEX IF NOT EXISTS asp_syntax_query_replay_lookup_idx
            ON asp_syntax_query_replay(language_id, provider_id, project_root, query_ast_fingerprint, selector, updated_at_ms)",
    ] {
        execute_turso_statement_with_lock_retry(
            connection,
            statement,
            "failed to bootstrap Turso syntax query schema",
        )
        .await?;
    }
    Ok(())
}

/// Store one syntax replay object in the active Turso read model.
pub async fn upsert_turso_syntax_query_replay(
    db_path: &Path,
    generation: &agent_semantic_client_core::ClientCacheGeneration,
    packet_bytes: &[u8],
) -> Result<(), String> {
    let (replay, selector) =
        syntax_query_replay_and_selector_from_packet_import(generation, packet_bytes)?;
    let _operation_lock = acquire_turso_operation_lock(db_path, "syntax-query-replay-upsert")?;
    let connection = connect_turso_client_db(db_path).await?;
    bootstrap_turso_syntax_query_schema(&connection).await?;
    let replay_json = serde_json::to_string(&replay)
        .map_err(|error| format!("failed to serialize Turso syntax replay: {error}"))?;
    let project_root = crate::types::normalized_project_root(Path::new(&generation.project_root));
    execute_turso_operation_with_lock_retry(
        || async {
            connection
                .execute(
                    "INSERT INTO asp_syntax_query_replay (
                        generation_id,
                        language_id,
                        provider_id,
                        project_root,
                        query_ast_fingerprint,
                        selector,
                        replay_json,
                        updated_at_ms
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                    ON CONFLICT(generation_id) DO UPDATE SET
                        language_id = excluded.language_id,
                        provider_id = excluded.provider_id,
                        project_root = excluded.project_root,
                        query_ast_fingerprint = excluded.query_ast_fingerprint,
                        selector = excluded.selector,
                        replay_json = excluded.replay_json,
                        updated_at_ms = excluded.updated_at_ms",
                    (
                        replay.generation_id.as_str(),
                        replay.language_id.as_str(),
                        generation.provider_id.as_str(),
                        project_root.as_str(),
                        replay.query_ast_fingerprint.as_str(),
                        selector.as_deref(),
                        replay_json.as_str(),
                        current_timestamp_ms(),
                    ),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to upsert Turso syntax replay",
    )
    .await?;
    Ok(())
}

/// Return syntax replay rows from the active Turso read model.
pub async fn lookup_turso_syntax_query_replay(
    db_path: &Path,
    lookup: &ClientDbSyntaxQueryLookup,
) -> Result<Option<ClientDbSyntaxQueryReplay>, String> {
    if !db_path.exists() {
        return Ok(None);
    }
    let connection = connect_turso_client_db(db_path).await?;
    bootstrap_turso_syntax_query_schema(&connection).await?;
    let project_root = crate::types::normalized_project_root(&lookup.project_root);
    let mut rows = run_turso_operation_with_lock_retry(
        || async {
            connection
                .query(
                    "SELECT replay_json
                     FROM asp_syntax_query_replay
                     WHERE language_id = ?1
                       AND provider_id = ?2
                       AND project_root = ?3
                       AND query_ast_fingerprint = ?4
                       AND ((?5 IS NULL AND selector IS NULL) OR selector = ?5)
                     ORDER BY updated_at_ms DESC
                     LIMIT 1",
                    (
                        lookup.language_id.as_str(),
                        lookup.provider_id.as_str(),
                        project_root.as_str(),
                        lookup.query_ast_fingerprint.as_str(),
                        lookup.selector.as_deref(),
                    ),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to query Turso syntax replay",
    )
    .await?;
    let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso syntax replay row: {error}"))?
    else {
        return Ok(None);
    };
    let replay_json = row
        .get::<String>(0)
        .map_err(|error| format!("failed to read Turso syntax replay JSON: {error}"))?;
    serde_json::from_str(&replay_json)
        .map(Some)
        .map_err(|error| format!("failed to decode Turso syntax replay JSON: {error}"))
}

/// Flush syntax replay rows from the active Turso read model.
pub async fn flush_turso_syntax_query_replay(db_path: &Path) -> Result<u32, String> {
    let _operation_lock = acquire_turso_operation_lock(db_path, "syntax-query-replay-flush")?;
    let connection = connect_turso_client_db(db_path).await?;
    bootstrap_turso_syntax_query_schema(&connection).await?;
    let count = execute_turso_operation_with_lock_retry(
        || async {
            connection
                .execute("DELETE FROM asp_syntax_query_replay", ())
                .await
                .map_err(|error| error.to_string())
        },
        "failed to flush Turso syntax replay rows",
    )
    .await?;
    Ok(count.min(u64::from(u32::MAX)) as u32)
}

fn current_timestamp_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}
