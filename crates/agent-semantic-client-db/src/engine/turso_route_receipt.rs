//! Turso-backed search route receipt adapter.

use std::path::Path;

use serde::Serialize;

use super::turso::connect_turso_client_db;

/// Bounded search/route receipt written through the Turso DB Engine.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TursoClientDbRouteReceipt {
    pub receipt_id: String,
    pub repo_id: String,
    pub workspace_id: String,
    pub scope_id: String,
    pub session_id: Option<String>,
    pub query: String,
    pub route_source: String,
    pub selected_selector: Option<String>,
    pub next_command: Option<String>,
    pub hit_count: u32,
    pub evidence_ids: Vec<String>,
    pub created_at_ms: i64,
}

/// Insert or replace one search route receipt in the Turso DB Engine file.
pub async fn upsert_turso_route_receipt(
    db_path: &Path,
    receipt: &TursoClientDbRouteReceipt,
) -> Result<(), String> {
    let evidence_ids_json = serde_json::to_string(&receipt.evidence_ids)
        .map_err(|error| format!("failed to encode Turso route receipt evidence ids: {error}"))?;
    let connection = connect_turso_client_db(db_path).await?;
    connection
        .execute(
            "INSERT INTO asp_route_receipt (
                receipt_id,
                repo_id,
                workspace_id,
                scope_id,
                session_id,
                query,
                route_source,
                selected_selector,
                next_command,
                hit_count,
                evidence_ids_json,
                created_at_ms
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(receipt_id) DO UPDATE SET
                repo_id = excluded.repo_id,
                workspace_id = excluded.workspace_id,
                scope_id = excluded.scope_id,
                session_id = excluded.session_id,
                query = excluded.query,
                route_source = excluded.route_source,
                selected_selector = excluded.selected_selector,
                next_command = excluded.next_command,
                hit_count = excluded.hit_count,
                evidence_ids_json = excluded.evidence_ids_json,
                created_at_ms = excluded.created_at_ms",
            (
                receipt.receipt_id.as_str(),
                receipt.repo_id.as_str(),
                receipt.workspace_id.as_str(),
                receipt.scope_id.as_str(),
                receipt.session_id.as_deref(),
                receipt.query.as_str(),
                receipt.route_source.as_str(),
                receipt.selected_selector.as_deref(),
                receipt.next_command.as_deref(),
                i64::from(receipt.hit_count),
                evidence_ids_json.as_str(),
                receipt.created_at_ms,
            ),
        )
        .await
        .map_err(|error| format!("failed to upsert Turso route receipt: {error}"))?;
    Ok(())
}

/// List bounded search route receipts for a workspace, newest first.
pub async fn list_turso_route_receipts(
    db_path: &Path,
    repo_id: &str,
    workspace_id: &str,
    session_id: Option<&str>,
    limit: u32,
) -> Result<Vec<TursoClientDbRouteReceipt>, String> {
    if limit == 0 {
        return Ok(Vec::new());
    }
    let connection = connect_turso_client_db(db_path).await?;
    let sql_with_session = "SELECT
            receipt_id,
            repo_id,
            workspace_id,
            scope_id,
            session_id,
            query,
            route_source,
            selected_selector,
            next_command,
            hit_count,
            evidence_ids_json,
            created_at_ms
         FROM asp_route_receipt
         WHERE repo_id = ?1 AND workspace_id = ?2 AND session_id = ?3
         ORDER BY created_at_ms DESC, receipt_id
         LIMIT ?4";
    let sql_without_session = "SELECT
            receipt_id,
            repo_id,
            workspace_id,
            scope_id,
            session_id,
            query,
            route_source,
            selected_selector,
            next_command,
            hit_count,
            evidence_ids_json,
            created_at_ms
         FROM asp_route_receipt
         WHERE repo_id = ?1 AND workspace_id = ?2
         ORDER BY created_at_ms DESC, receipt_id
         LIMIT ?3";
    let mut rows = if let Some(session_id) = session_id {
        connection
            .query(sql_with_session, (repo_id, workspace_id, session_id, limit))
            .await
            .map_err(|error| format!("failed to query Turso route receipts: {error}"))?
    } else {
        connection
            .query(sql_without_session, (repo_id, workspace_id, limit))
            .await
            .map_err(|error| format!("failed to query Turso route receipts: {error}"))?
    };
    let mut receipts = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso route receipt row: {error}"))?
    {
        let hit_count = row
            .get::<i64>(9)
            .map_err(|error| format!("failed to read Turso route receipt hit count: {error}"))?;
        let evidence_ids_json = row
            .get::<String>(10)
            .map_err(|error| format!("failed to read Turso route receipt evidence ids: {error}"))?;
        receipts.push(TursoClientDbRouteReceipt {
            receipt_id: row
                .get::<String>(0)
                .map_err(|error| format!("failed to read Turso route receipt id: {error}"))?,
            repo_id: row
                .get::<String>(1)
                .map_err(|error| format!("failed to read Turso route receipt repo id: {error}"))?,
            workspace_id: row.get::<String>(2).map_err(|error| {
                format!("failed to read Turso route receipt workspace id: {error}")
            })?,
            scope_id: row
                .get::<String>(3)
                .map_err(|error| format!("failed to read Turso route receipt scope id: {error}"))?,
            session_id: row.get::<Option<String>>(4).map_err(|error| {
                format!("failed to read Turso route receipt session id: {error}")
            })?,
            query: row
                .get::<String>(5)
                .map_err(|error| format!("failed to read Turso route receipt query: {error}"))?,
            route_source: row.get::<String>(6).map_err(|error| {
                format!("failed to read Turso route receipt route source: {error}")
            })?,
            selected_selector: row.get::<Option<String>>(7).map_err(|error| {
                format!("failed to read Turso route receipt selected selector: {error}")
            })?,
            next_command: row.get::<Option<String>>(8).map_err(|error| {
                format!("failed to read Turso route receipt next command: {error}")
            })?,
            hit_count: u32::try_from(hit_count).map_err(|error| {
                format!("failed to decode Turso route receipt hit count: {error}")
            })?,
            evidence_ids: serde_json::from_str::<Vec<String>>(&evidence_ids_json).map_err(
                |error| format!("failed to decode Turso route receipt evidence ids: {error}"),
            )?,
            created_at_ms: row.get::<i64>(11).map_err(|error| {
                format!("failed to read Turso route receipt created timestamp: {error}")
            })?,
        });
    }
    Ok(receipts)
}
