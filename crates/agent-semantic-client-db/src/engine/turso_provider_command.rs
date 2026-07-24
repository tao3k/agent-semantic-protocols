//! Turso provider command selection cache adapter.

use std::path::Path;

use crate::types::{ClientDbProviderCommandSelection, normalized_project_root};

use super::turso::connect_turso_client_db;
use super::turso_statement::{
    execute_turso_operation, execute_turso_statement, run_turso_operation,
};

async fn bootstrap_turso_provider_command_schema(
    connection: &turso::Connection,
) -> Result<(), String> {
    for statement in [
        "CREATE TABLE IF NOT EXISTS asp_provider_command_selection (
            project_root TEXT NOT NULL,
            context_fingerprint TEXT NOT NULL,
            manifest_id TEXT NOT NULL,
            manifest_digest TEXT NOT NULL,
            language_id TEXT NOT NULL,
            provider_id TEXT NOT NULL,
            binary TEXT NOT NULL,
            execution TEXT NOT NULL,
            provider_command_prefix_json TEXT NOT NULL,
            executable_path TEXT,
            executable_len INTEGER,
            executable_mtime_ms INTEGER,
            PRIMARY KEY (project_root, context_fingerprint, language_id, provider_id, manifest_id)
        )",
        "CREATE INDEX IF NOT EXISTS asp_provider_command_selection_project_idx
            ON asp_provider_command_selection(project_root, context_fingerprint)",
    ] {
        execute_turso_statement(
            connection,
            statement,
            "failed to bootstrap Turso provider command selection schema",
        )
        .await?;
    }
    Ok(())
}

pub async fn replace_turso_provider_command_selections(
    db_path: &Path,
    project_root: &Path,
    context_fingerprint: &str,
    selections: &[ClientDbProviderCommandSelection],
) -> Result<(), String> {
    let project_root = normalized_project_root(project_root);
    let connection = connect_turso_client_db(db_path).await?;
    bootstrap_turso_provider_command_schema(&connection).await?;
    replace_turso_provider_command_selections_with_connection(
        &connection,
        project_root.as_str(),
        context_fingerprint,
        selections,
    )
    .await
}

async fn replace_turso_provider_command_selections_with_connection(
    connection: &turso::Connection,
    project_root: &str,
    context_fingerprint: &str,
    selections: &[ClientDbProviderCommandSelection],
) -> Result<(), String> {
    execute_turso_statement(
        connection,
        "BEGIN IMMEDIATE",
        "failed to begin Turso provider command selection transaction",
    )
    .await?;
    if let Err(error) = execute_turso_operation(
        || async {
            connection
                .execute(
                    "DELETE FROM asp_provider_command_selection
             WHERE project_root = ?1 AND context_fingerprint = ?2",
                    (project_root, context_fingerprint),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to delete Turso provider command selections",
    )
    .await
    {
        rollback_turso_provider_command_selection_transaction(connection).await;
        return Err(error);
    }
    for selection in selections {
        let command_prefix_json = serde_json::to_string(selection.provider_command_prefix())
            .map_err(|error| {
                format!("failed to serialize Turso provider command prefix: {error}")
            })?;
        if let Err(error) = execute_turso_operation(
            || async {
                connection
                    .execute(
                        "INSERT INTO asp_provider_command_selection (
                    project_root,
                    context_fingerprint,
                    manifest_id,
                    manifest_digest,
                    language_id,
                    provider_id,
                    binary,
                    execution,
                    provider_command_prefix_json,
                    executable_path,
                    executable_len,
                    executable_mtime_ms
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                        (
                            project_root,
                            context_fingerprint,
                            selection.manifest_id(),
                            selection.manifest_digest(),
                            selection.language_id(),
                            selection.provider_id(),
                            selection.binary(),
                            selection.execution(),
                            command_prefix_json.as_str(),
                            selection.executable_path(),
                            selection.executable_len(),
                            selection.executable_mtime_ms(),
                        ),
                    )
                    .await
                    .map_err(|error| error.to_string())
            },
            "failed to write Turso provider command selection",
        )
        .await
        {
            rollback_turso_provider_command_selection_transaction(connection).await;
            return Err(error);
        }
    }
    if let Err(error) = execute_turso_statement(
        connection,
        "COMMIT",
        "failed to commit Turso provider command selection transaction",
    )
    .await
    {
        rollback_turso_provider_command_selection_transaction(connection).await;
        return Err(error);
    }
    Ok(())
}

async fn rollback_turso_provider_command_selection_transaction(connection: &turso::Connection) {
    let _ = execute_turso_statement(
        connection,
        "ROLLBACK",
        "failed to rollback Turso provider command selection transaction",
    )
    .await;
}

pub async fn lookup_turso_provider_command_selections(
    db_path: &Path,
    project_root: &Path,
    context_fingerprint: &str,
) -> Result<Option<Vec<ClientDbProviderCommandSelection>>, String> {
    if !db_path.exists() {
        return Ok(None);
    }
    let connection = connect_turso_client_db(db_path).await?;
    bootstrap_turso_provider_command_schema(&connection).await?;
    let project_root = normalized_project_root(project_root);
    let mut rows = run_turso_operation(
        || async {
            connection
                .query(
                    "SELECT manifest_id,
                    manifest_digest,
                    language_id,
                    provider_id,
                    binary,
                    execution,
                    provider_command_prefix_json,
                    executable_path,
                    executable_len,
                    executable_mtime_ms
             FROM asp_provider_command_selection
             WHERE project_root = ?1 AND context_fingerprint = ?2
             ORDER BY language_id, provider_id, manifest_id",
                    (project_root.as_str(), context_fingerprint),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to query Turso provider command selections",
    )
    .await?;
    let mut selections = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso provider command selection row: {error}"))?
    {
        selections.push(turso_provider_command_selection_from_row(&row)?);
    }
    Ok((!selections.is_empty()).then_some(selections))
}

fn turso_provider_command_selection_from_row(
    row: &turso::Row,
) -> Result<ClientDbProviderCommandSelection, String> {
    let command_prefix_json = row
        .get::<String>(6)
        .map_err(|error| format!("failed to read Turso provider command prefix: {error}"))?;
    let provider_command_prefix = serde_json::from_str::<Vec<String>>(&command_prefix_json)
        .map_err(|error| format!("failed to decode Turso provider command prefix: {error}"))?;
    Ok(ClientDbProviderCommandSelection::new(
        row.get::<String>(0)
            .map_err(|error| format!("failed to read Turso manifest id: {error}"))?
            .into(),
        row.get::<String>(1)
            .map_err(|error| format!("failed to read Turso manifest digest: {error}"))?
            .into(),
        row.get::<String>(2)
            .map_err(|error| format!("failed to read Turso language id: {error}"))?
            .into(),
        row.get::<String>(3)
            .map_err(|error| format!("failed to read Turso provider id: {error}"))?
            .into(),
        row.get::<String>(4)
            .map_err(|error| format!("failed to read Turso binary: {error}"))?
            .into(),
        row.get::<String>(5)
            .map_err(|error| format!("failed to read Turso execution: {error}"))?
            .into(),
        provider_command_prefix
            .into_iter()
            .map(Into::into)
            .collect(),
        row.get::<Option<String>>(7)
            .map_err(|error| format!("failed to read Turso executable path: {error}"))?
            .map(Into::into),
        row.get::<Option<i64>>(8)
            .map_err(|error| format!("failed to read Turso executable length: {error}"))?,
        row.get::<Option<i64>>(9)
            .map_err(|error| format!("failed to read Turso executable mtime: {error}"))?,
    ))
}
