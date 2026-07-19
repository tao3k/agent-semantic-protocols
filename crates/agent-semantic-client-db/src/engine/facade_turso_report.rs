use std::collections::BTreeSet;
use std::path::Path;

use agent_semantic_client_core::{ClientDbJournalMode, ClientDbStatus};

use crate::types::{ClientDbReport, ClientDbRuntimePragmas, ClientDbSyntaxQueryReplay};

use super::facade::block_on_db_engine_async;
use super::turso::connect_turso_client_db;
use super::turso_lock_policy::TURSO_CLIENT_DB_BUSY_TIMEOUT_MS;

pub(super) fn turso_client_db_report(db_path: &Path) -> ClientDbReport {
    let status = if db_path.exists() {
        ClientDbStatus::Present
    } else {
        ClientDbStatus::Missing
    };
    let mut reason = None;
    let counts = if db_path.exists() {
        match turso_client_db_counts(db_path) {
            Ok(counts) => counts,
            Err(error) => {
                reason = Some(error);
                TursoClientDbCounts::default()
            }
        }
    } else {
        TursoClientDbCounts::default()
    };
    let runtime_pragmas_available = status == ClientDbStatus::Present;
    ClientDbReport {
        db_path: db_path.to_path_buf(),
        status,
        generation_count: counts.cache_generations,
        syntax_row_generation_count: counts.syntax_replays,
        syntax_row_match_count: counts.syntax_row_matches,
        syntax_row_capture_count: counts.syntax_row_captures,
        structural_index_generation_count: counts.structural_index_generations,
        structural_index_owner_count: counts.structural_index_owners,
        structural_index_symbol_count: counts.structural_index_symbols,
        structural_index_dependency_usage_count: counts.structural_index_dependency_usages,
        source_index_generation_count: counts.source_index_generations,
        source_index_owner_count: counts.source_index_owners,
        source_index_selector_count: counts.source_index_selectors,
        artifact_event_count: counts.artifact_events,
        raw_source_stored: false,
        runtime_pragmas: runtime_pragmas_available.then(|| ClientDbRuntimePragmas {
            journal_mode: ClientDbJournalMode::from("wal"),
            synchronous: 1,
            busy_timeout_ms: TURSO_CLIENT_DB_BUSY_TIMEOUT_MS as i64,
            foreign_keys: true,
        }),
        reason,
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct TursoClientDbCounts {
    cache_generations: u32,
    syntax_replays: u32,
    syntax_row_matches: u32,
    syntax_row_captures: u32,
    structural_index_generations: u32,
    structural_index_owners: u32,
    structural_index_symbols: u32,
    structural_index_dependency_usages: u32,
    source_index_generations: u32,
    source_index_owners: u32,
    source_index_selectors: u32,
    artifact_events: u32,
}

fn turso_client_db_counts(db_path: &Path) -> Result<TursoClientDbCounts, String> {
    let db_path = db_path.to_path_buf();
    block_on_db_engine_async(async move {
        let connection = connect_turso_client_db(&db_path).await?;
        let syntax_row_counts = count_turso_syntax_replay_rows_or_zero(&connection).await;
        Ok(TursoClientDbCounts {
            cache_generations: count_turso_rows_or_zero(&connection, "asp_cache_generation").await,
            syntax_replays: count_turso_rows_or_zero(&connection, "asp_syntax_query_replay").await,
            syntax_row_matches: syntax_row_counts.matches,
            syntax_row_captures: syntax_row_counts.captures,
            structural_index_generations: count_turso_structural_generations_or_zero(&connection)
                .await,
            structural_index_owners: count_turso_graph_kind_or_zero(
                &connection,
                "structural-owner",
            )
            .await,
            structural_index_symbols: count_turso_graph_kind_or_zero(&connection, "symbol").await,
            structural_index_dependency_usages: count_turso_graph_kind_or_zero(
                &connection,
                "dependency-usage",
            )
            .await,
            source_index_generations: count_turso_rows_or_zero(
                &connection,
                "asp_source_index_scope_v1",
            )
            .await,
            source_index_owners: count_turso_rows_or_zero(&connection, "asp_source_index_owner_v1")
                .await,
            source_index_selectors: count_turso_source_index_selector_rows_or_zero(&connection)
                .await,
            artifact_events: count_turso_rows_or_zero(&connection, "asp_artifact_event").await,
        })
    })
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct TursoSyntaxReplayRowCounts {
    matches: u32,
    captures: u32,
}

async fn count_turso_syntax_replay_rows_or_zero(
    connection: &turso::Connection,
) -> TursoSyntaxReplayRowCounts {
    count_turso_syntax_replay_rows(connection)
        .await
        .unwrap_or_default()
}

async fn count_turso_syntax_replay_rows(
    connection: &turso::Connection,
) -> Result<TursoSyntaxReplayRowCounts, String> {
    let mut rows = connection
        .query("SELECT replay_json FROM asp_syntax_query_replay", ())
        .await
        .map_err(|error| format!("failed to query Turso syntax replay rows: {error}"))?;
    let mut counts = TursoSyntaxReplayRowCounts::default();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso syntax replay row: {error}"))?
    {
        let replay_json = row
            .get::<String>(0)
            .map_err(|error| format!("failed to read Turso syntax replay JSON: {error}"))?;
        let replay = serde_json::from_str::<ClientDbSyntaxQueryReplay>(&replay_json)
            .map_err(|error| format!("failed to decode Turso syntax replay JSON: {error}"))?;
        let matches = replay
            .rows
            .iter()
            .map(|row| row.match_locator.as_str())
            .collect::<BTreeSet<_>>()
            .len();
        counts.matches = counts
            .matches
            .saturating_add(matches.min(u32::MAX as usize) as u32);
        counts.captures = counts
            .captures
            .saturating_add(replay.rows.len().min(u32::MAX as usize) as u32);
    }
    Ok(counts)
}

async fn count_turso_rows_or_zero(connection: &turso::Connection, table: &str) -> u32 {
    count_turso_rows(connection, table).await.unwrap_or(0)
}

async fn count_turso_rows(connection: &turso::Connection, table: &str) -> Result<u32, String> {
    let sql = format!("SELECT COUNT(*) FROM {table}");
    count_turso_query(connection, &sql, ())
        .await
        .or_else(|error| {
            if error.contains("no such table") {
                Ok(0)
            } else {
                Err(error)
            }
        })
}

async fn count_turso_source_index_selector_rows_or_zero(connection: &turso::Connection) -> u32 {
    let mut rows = match connection
        .query(
            "SELECT COALESCE(SUM(selector_count), 0) FROM asp_source_index_owner_v1",
            (),
        )
        .await
    {
        Ok(rows) => rows,
        Err(error) if error.to_string().contains("no such table") => return 0,
        Err(_) => return 0,
    };
    match rows.next().await {
        Ok(Some(row)) => row
            .get::<i64>(0)
            .map(|count| count.max(0).min(i64::from(u32::MAX)) as u32)
            .unwrap_or(0),
        _ => 0,
    }
}

async fn count_turso_graph_kind_or_zero(connection: &turso::Connection, kind: &str) -> u32 {
    count_turso_graph_kind(connection, kind).await.unwrap_or(0)
}

async fn count_turso_graph_kind(connection: &turso::Connection, kind: &str) -> Result<u32, String> {
    count_turso_query(
        connection,
        "SELECT COUNT(*) FROM asp_graph_entity WHERE kind = ?1",
        [kind],
    )
    .await
    .or_else(|error| {
        if error.contains("no such table") {
            Ok(0)
        } else {
            Err(error)
        }
    })
}

async fn count_turso_structural_generations_or_zero(connection: &turso::Connection) -> u32 {
    count_turso_structural_generations(connection)
        .await
        .unwrap_or(0)
}

async fn count_turso_structural_generations(connection: &turso::Connection) -> Result<u32, String> {
    let structural_entities = count_turso_query(
        connection,
        "SELECT COUNT(*) FROM asp_graph_entity WHERE kind IN ('structural-owner', 'symbol', 'dependency-usage')",
        (),
    )
    .await
    .or_else(|error| {
        if error.contains("no such table") {
            Ok(0)
        } else {
            Err(error)
        }
    })?;
    Ok((structural_entities > 0) as u32)
}

async fn count_turso_query<P>(
    connection: &turso::Connection,
    sql: &str,
    params: P,
) -> Result<u32, String>
where
    P: turso::params::IntoParams,
{
    let mut rows = connection
        .query(sql, params)
        .await
        .map_err(|error| format!("failed to count Turso DB rows for `{sql}`: {error}"))?;
    let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso DB row count for `{sql}`: {error}"))?
    else {
        return Ok(0);
    };
    let count = row
        .get::<i64>(0)
        .map_err(|error| format!("failed to decode Turso DB row count for `{sql}`: {error}"))?;
    Ok(count.max(0).min(i64::from(u32::MAX)) as u32)
}
