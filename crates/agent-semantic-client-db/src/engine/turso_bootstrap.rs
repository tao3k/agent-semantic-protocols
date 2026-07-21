//! Turso DB Engine bootstrap and shared schema convergence.

use std::path::Path;

use super::turso::{
    TursoClientDbEngineReport, bootstrap_turso_schema_version, connect_turso_client_db,
    prepare_turso_client_db_path, turso_bootstrap_report,
};
use super::turso_cache::bootstrap_turso_client_cache_schema;
use super::turso_statement::execute_turso_statement;
use super::turso_syntax::bootstrap_turso_syntax_query_schema;

/// Bootstrap the active local Turso backend file and schema.
pub async fn bootstrap_turso_client_db(
    db_path: &Path,
) -> Result<TursoClientDbEngineReport, String> {
    let turso_path = prepare_turso_client_db_path(db_path)?;
    let mut connection = connect_turso_client_db(&turso_path).await?;
    bootstrap_turso_schema_version(&mut connection).await?;
    bootstrap_turso_client_cache_schema(&connection).await?;
    bootstrap_turso_syntax_query_schema(&connection).await?;
    let mut search_projection_connection =
        super::turso::connect_turso_search_projection_db(&turso_path).await?;
    bootstrap_turso_schema_version(&mut search_projection_connection).await?;
    bootstrap_turso_client_search_schema(&search_projection_connection).await?;
    Ok(turso_bootstrap_report(db_path))
}

/// Bootstrap only the base state required by source-index v1 persistence.
pub async fn bootstrap_turso_source_index_db(
    db_path: &Path,
) -> Result<TursoClientDbEngineReport, String> {
    let turso_path = prepare_turso_client_db_path(db_path)?;
    let mut connection = connect_turso_client_db(&turso_path).await?;
    bootstrap_turso_schema_version(&mut connection).await?;
    Ok(turso_bootstrap_report(db_path))
}

/// Bootstrap the discardable, root-bound depth-zero search projection.
pub(super) async fn bootstrap_turso_client_search_schema(
    connection: &turso::Connection,
) -> Result<(), String> {
    for statement in [
        "CREATE TABLE IF NOT EXISTS asp_search_projection_generation (
            namespace TEXT NOT NULL PRIMARY KEY,
            snapshot_root TEXT NOT NULL,
            provider_digest TEXT NOT NULL
        )",
        "CREATE TABLE IF NOT EXISTS asp_search_projection_document (
            namespace TEXT NOT NULL,
            snapshot_root TEXT NOT NULL,
            document_id TEXT NOT NULL,
            entity_id TEXT NOT NULL,
            selector TEXT,
            document TEXT NOT NULL,
            PRIMARY KEY(namespace, snapshot_root, document_id)
        )",
    ] {
        execute_turso_statement(
            connection,
            statement,
            "failed to bootstrap Turso search schema",
        )
        .await?;
    }

    for statement in [
        "CREATE INDEX IF NOT EXISTS asp_search_projection_document_entity_idx
         ON asp_search_projection_document(namespace, snapshot_root, entity_id)",
        "CREATE INDEX IF NOT EXISTS asp_search_projection_document_fts_idx
         ON asp_search_projection_document USING fts (document, selector)",
    ] {
        execute_turso_statement(
            connection,
            statement,
            "failed to bootstrap Turso search schema",
        )
        .await?;
    }
    Ok(())
}
