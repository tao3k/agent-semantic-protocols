//! Turso DB Engine bootstrap and shared schema convergence.

use std::path::Path;

use super::turso::{
    TursoClientDbEngineReport, bootstrap_turso_schema_version, connect_turso_client_db,
    prepare_turso_client_db_path, turso_bootstrap_report, turso_table_column_exists,
};
use super::turso_cache::bootstrap_turso_client_cache_schema;
use super::turso_evidence_graph::TURSO_ENTITY_TABLE;
use super::turso_operation_lock::acquire_turso_operation_lock;
use super::turso_statement::execute_turso_statement_with_lock_retry;
use super::turso_syntax::bootstrap_turso_syntax_query_schema;

/// Bootstrap the active local Turso backend file and schema.
pub async fn bootstrap_turso_client_db(
    db_path: &Path,
) -> Result<TursoClientDbEngineReport, String> {
    let turso_path = prepare_turso_client_db_path(db_path)?;
    let _operation_lock = acquire_turso_operation_lock(&turso_path, "bootstrap")?;
    let connection = connect_turso_client_db(&turso_path).await?;
    bootstrap_turso_schema_version(&connection).await?;
    bootstrap_turso_client_cache_schema(&connection).await?;
    bootstrap_turso_syntax_query_schema(&connection).await?;
    bootstrap_turso_client_search_schema(&connection).await?;
    Ok(turso_bootstrap_report(db_path))
}

/// Bootstrap only the base state required by source-index v1 persistence.
pub async fn bootstrap_turso_source_index_db(
    db_path: &Path,
) -> Result<TursoClientDbEngineReport, String> {
    let turso_path = prepare_turso_client_db_path(db_path)?;
    let _operation_lock = acquire_turso_operation_lock(&turso_path, "source-index-bootstrap")?;
    let connection = connect_turso_client_db(&turso_path).await?;
    bootstrap_turso_schema_version(&connection).await?;
    Ok(turso_bootstrap_report(db_path))
}

/// Bootstrap graph, search-document, overlay, and route receipt tables.
pub(super) async fn bootstrap_turso_client_search_schema(
    connection: &turso::Connection,
) -> Result<(), String> {
    for statement in [
        "CREATE TABLE IF NOT EXISTS asp_graph_entity (
            id TEXT PRIMARY KEY,
            kind TEXT NOT NULL,
            semantic_kind TEXT,
            label TEXT NOT NULL,
            selector TEXT,
            path TEXT,
            language_id TEXT,
            provider_id TEXT,
            query_keys_json TEXT NOT NULL DEFAULT '[]'
        )",
        "CREATE TABLE IF NOT EXISTS asp_graph_edge (
            from_id TEXT NOT NULL,
            to_id TEXT NOT NULL,
            kind TEXT NOT NULL,
            PRIMARY KEY(from_id, to_id, kind)
        )",
        "CREATE TABLE IF NOT EXISTS asp_search_document (
            namespace TEXT NOT NULL,
            document_id TEXT NOT NULL,
            entity_id TEXT NOT NULL,
            selector TEXT,
            document TEXT NOT NULL,
            PRIMARY KEY(namespace, document_id)
        )",
        "CREATE TABLE IF NOT EXISTS asp_overlay_document (
            repo_id TEXT NOT NULL,
            workspace_id TEXT NOT NULL,
            session_id TEXT NOT NULL,
            base_generation TEXT NOT NULL,
            document_id TEXT NOT NULL,
            selector TEXT,
            document TEXT NOT NULL,
            PRIMARY KEY(repo_id, workspace_id, session_id, base_generation, document_id)
        )",
        "CREATE TABLE IF NOT EXISTS asp_route_receipt (
            receipt_id TEXT PRIMARY KEY,
            repo_id TEXT NOT NULL,
            workspace_id TEXT NOT NULL,
            scope_id TEXT NOT NULL,
            session_id TEXT,
            query TEXT NOT NULL,
            route_source TEXT NOT NULL,
            selected_selector TEXT,
            next_command TEXT,
            hit_count INTEGER NOT NULL,
            evidence_ids_json TEXT NOT NULL DEFAULT '[]',
            created_at_ms INTEGER NOT NULL
        )",
    ] {
        execute_turso_statement_with_lock_retry(
            connection,
            statement,
            "failed to bootstrap Turso search schema",
        )
        .await?;
    }

    ensure_turso_graph_entity_columns(connection).await?;

    for statement in [
        "CREATE INDEX IF NOT EXISTS asp_graph_entity_kind_idx ON asp_graph_entity(kind)",
        "CREATE INDEX IF NOT EXISTS asp_graph_entity_language_idx ON asp_graph_entity(kind, language_id)",
        "CREATE INDEX IF NOT EXISTS asp_graph_entity_owner_selector_idx ON asp_graph_entity(kind, path, language_id)",
        "CREATE INDEX IF NOT EXISTS asp_graph_edge_kind_idx ON asp_graph_edge(kind)",
        "CREATE INDEX IF NOT EXISTS asp_graph_edge_to_idx ON asp_graph_edge(to_id)",
        "CREATE INDEX IF NOT EXISTS asp_search_document_entity_idx ON asp_search_document(entity_id)",
        "CREATE INDEX IF NOT EXISTS asp_search_document_fts_idx ON asp_search_document USING fts (document, selector)",
        "CREATE INDEX IF NOT EXISTS asp_overlay_document_session_idx ON asp_overlay_document(repo_id, workspace_id, session_id)",
        "CREATE INDEX IF NOT EXISTS asp_overlay_document_fts_idx ON asp_overlay_document USING fts (document, selector)",
        "CREATE INDEX IF NOT EXISTS asp_route_receipt_workspace_idx ON asp_route_receipt(repo_id, workspace_id, created_at_ms)",
        "CREATE INDEX IF NOT EXISTS asp_route_receipt_session_idx ON asp_route_receipt(repo_id, workspace_id, session_id, created_at_ms)",
    ] {
        execute_turso_statement_with_lock_retry(
            connection,
            statement,
            "failed to bootstrap Turso search schema",
        )
        .await?;
    }
    Ok(())
}

async fn ensure_turso_graph_entity_columns(connection: &turso::Connection) -> Result<(), String> {
    for (column, definition) in [
        ("selector", "TEXT"),
        ("semantic_kind", "TEXT"),
        ("path", "TEXT"),
        ("language_id", "TEXT"),
        ("provider_id", "TEXT"),
        ("query_keys_json", "TEXT NOT NULL DEFAULT '[]'"),
    ] {
        if !turso_table_column_exists(connection, TURSO_ENTITY_TABLE, column).await? {
            let statement =
                format!("ALTER TABLE {TURSO_ENTITY_TABLE} ADD COLUMN {column} {definition}");
            execute_turso_statement_with_lock_retry(
                connection,
                statement.as_str(),
                "failed to converge Turso graph entity column",
            )
            .await?;
        }
    }
    Ok(())
}
