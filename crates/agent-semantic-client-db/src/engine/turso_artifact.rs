//! Turso graph-turbo artifact event adapter.

use std::path::Path;

use crate::types::ClientDbArtifactEvent;

use super::turso::connect_turso_client_db;
use super::turso_operation_lock::acquire_turso_operation_lock;
use super::turso_statement::{
    execute_turso_operation_with_lock_retry, execute_turso_statement_with_lock_retry,
    run_turso_operation_with_lock_retry,
};

async fn bootstrap_turso_artifact_events_schema(
    connection: &turso::Connection,
) -> Result<(), String> {
    for statement in [
        "CREATE TABLE IF NOT EXISTS asp_artifact_event (
            artifact_path TEXT NOT NULL,
            event_ordinal INTEGER NOT NULL,
            timestamp_ms INTEGER NOT NULL,
            kind TEXT NOT NULL,
            language TEXT NOT NULL,
            method TEXT NOT NULL,
            target TEXT NOT NULL,
            query TEXT NOT NULL,
            project_root TEXT NOT NULL,
            project_root_arg TEXT NOT NULL,
            bytes INTEGER NOT NULL,
            PRIMARY KEY (artifact_path, event_ordinal)
        )",
        "CREATE INDEX IF NOT EXISTS asp_artifact_event_timeline_idx
            ON asp_artifact_event(timestamp_ms, artifact_path, event_ordinal)",
    ] {
        execute_turso_statement_with_lock_retry(
            connection,
            statement,
            "failed to bootstrap Turso artifact-event schema",
        )
        .await?;
    }
    Ok(())
}

pub async fn upsert_turso_artifact_events(
    db_path: &Path,
    events: &[ClientDbArtifactEvent],
) -> Result<u32, String> {
    if events.is_empty() {
        return Ok(0);
    }
    let _operation_lock = acquire_turso_operation_lock(db_path, "artifact-event-upsert")?;
    let connection = connect_turso_client_db(db_path).await?;
    bootstrap_turso_artifact_events_schema(&connection).await?;
    upsert_turso_artifact_events_with_connection(&connection, events).await?;
    Ok(u32::try_from(events.len()).unwrap_or(u32::MAX))
}

async fn upsert_turso_artifact_events_with_connection(
    connection: &turso::Connection,
    events: &[ClientDbArtifactEvent],
) -> Result<(), String> {
    execute_turso_statement_with_lock_retry(
        connection,
        "BEGIN IMMEDIATE",
        "failed to begin Turso artifact-event transaction",
    )
    .await?;
    for event in events {
        let bytes = i64::try_from(event.bytes).unwrap_or(i64::MAX);
        if let Err(error) = execute_turso_operation_with_lock_retry(
            || async {
                connection
                    .execute(
                        "INSERT INTO asp_artifact_event (
                    artifact_path,
                    event_ordinal,
                    timestamp_ms,
                    kind,
                    language,
                    method,
                    target,
                    query,
                    project_root,
                    project_root_arg,
                    bytes
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                ON CONFLICT(artifact_path, event_ordinal) DO UPDATE SET
                    timestamp_ms = excluded.timestamp_ms,
                    kind = excluded.kind,
                    language = excluded.language,
                    method = excluded.method,
                    target = excluded.target,
                    query = excluded.query,
                    project_root = excluded.project_root,
                    project_root_arg = excluded.project_root_arg,
                    bytes = excluded.bytes",
                        (
                            event.artifact_path.as_str(),
                            i64::from(event.event_ordinal),
                            event.timestamp_ms,
                            event.kind.as_str(),
                            event.language.as_str(),
                            event.method.as_str(),
                            event.target.as_str(),
                            event.query.as_str(),
                            event.project_root.as_str(),
                            event.project_root_arg.as_str(),
                            bytes,
                        ),
                    )
                    .await
                    .map_err(|error| error.to_string())
            },
            "failed to upsert Turso artifact event in transaction",
        )
        .await
        {
            rollback_turso_artifact_events_transaction(connection).await;
            return Err(error);
        }
    }
    if let Err(error) = execute_turso_statement_with_lock_retry(
        connection,
        "COMMIT",
        "failed to commit Turso artifact-event transaction",
    )
    .await
    {
        rollback_turso_artifact_events_transaction(connection).await;
        return Err(error);
    }
    Ok(())
}

async fn rollback_turso_artifact_events_transaction(connection: &turso::Connection) {
    let _ = execute_turso_statement_with_lock_retry(
        connection,
        "ROLLBACK",
        "failed to rollback Turso artifact-event transaction",
    )
    .await;
}

pub async fn lookup_turso_artifact_events(
    db_path: &Path,
    since_timestamp_ms: Option<i64>,
    limit: u32,
) -> Result<Vec<ClientDbArtifactEvent>, String> {
    if limit == 0 || !db_path.exists() {
        return Ok(Vec::new());
    }
    let connection = connect_turso_client_db(db_path).await?;
    bootstrap_turso_artifact_events_schema(&connection).await?;
    let mut rows = run_turso_operation_with_lock_retry(
        || async {
            connection
                .query(
                    "SELECT artifact_path,
                    event_ordinal,
                    timestamp_ms,
                    kind,
                    language,
                    method,
                    target,
                    query,
                    project_root,
                    project_root_arg,
                    bytes
             FROM asp_artifact_event
             WHERE (?1 IS NULL OR timestamp_ms >= ?1)
             ORDER BY timestamp_ms ASC, artifact_path ASC, event_ordinal ASC
             LIMIT ?2",
                    (since_timestamp_ms, i64::from(limit)),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to query Turso artifact events",
    )
    .await?;
    let mut events = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso artifact-event row: {error}"))?
    {
        events.push(turso_artifact_event_from_row(&row)?);
    }
    Ok(events)
}

fn turso_artifact_event_from_row(row: &turso::Row) -> Result<ClientDbArtifactEvent, String> {
    let event_ordinal = row
        .get::<i64>(1)
        .map_err(|error| format!("failed to read Turso artifact event ordinal: {error}"))?
        .max(0)
        .min(i64::from(u32::MAX)) as u32;
    let bytes = row
        .get::<i64>(10)
        .map_err(|error| format!("failed to read Turso artifact event bytes: {error}"))?
        .max(0) as u64;
    Ok(ClientDbArtifactEvent {
        artifact_path: row
            .get::<String>(0)
            .map_err(|error| format!("failed to read Turso artifact path: {error}"))?,
        event_ordinal,
        timestamp_ms: row
            .get::<i64>(2)
            .map_err(|error| format!("failed to read Turso artifact timestamp: {error}"))?,
        kind: row
            .get::<String>(3)
            .map_err(|error| format!("failed to read Turso artifact kind: {error}"))?,
        language: row
            .get::<String>(4)
            .map_err(|error| format!("failed to read Turso artifact language: {error}"))?,
        method: row
            .get::<String>(5)
            .map_err(|error| format!("failed to read Turso artifact method: {error}"))?,
        target: row
            .get::<String>(6)
            .map_err(|error| format!("failed to read Turso artifact target: {error}"))?,
        query: row
            .get::<String>(7)
            .map_err(|error| format!("failed to read Turso artifact query: {error}"))?,
        project_root: row
            .get::<String>(8)
            .map_err(|error| format!("failed to read Turso artifact project root: {error}"))?,
        project_root_arg: row.get::<String>(9).map_err(|error| {
            format!("failed to read Turso artifact project root argument: {error}")
        })?,
        bytes,
    })
}
