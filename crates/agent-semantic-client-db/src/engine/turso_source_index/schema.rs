//! Canonical Turso source-index schema bootstrap.

use crate::engine::turso_statement::execute_turso_statement;

const WORKSPACE_DB_SCHEMA_RECEIPT_TABLE: &str = "asp_workspace_db_schema_receipt_v1";
const WORKSPACE_DB_SCHEMA_ID: &str = "agent.semantic-protocols.workspace-db-schema";
const WORKSPACE_DB_SCHEMA_VERSION: &str = "1";

fn workspace_db_schema_contract_marker() -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(WORKSPACE_DB_SCHEMA_ID.as_bytes());
    hasher.update(&[0]);
    hasher.update(WORKSPACE_DB_SCHEMA_VERSION.as_bytes());
    hasher.update(&[0]);
    hasher.update(include_bytes!("schema.rs"));
    hasher.update(&[0]);
    hasher.update(include_bytes!("provider_incremental_schema.rs"));
    format!(
        "{WORKSPACE_DB_SCHEMA_ID}/v{WORKSPACE_DB_SCHEMA_VERSION}/blake3-256:{}",
        hasher.finalize().to_hex()
    )
}

async fn workspace_db_schema_is_current(connection: &turso::Connection) -> Result<bool, String> {
    let mut rows = connection
        .query(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = ?1 LIMIT 1",
            (WORKSPACE_DB_SCHEMA_RECEIPT_TABLE,),
        )
        .await
        .map_err(|error| format!("failed to inspect workspace DB schema receipt: {error}"))?;
    let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read workspace DB schema receipt: {error}"))?
    else {
        return Ok(false);
    };
    let sql = row
        .get::<String>(0)
        .map_err(|error| format!("failed to decode workspace DB schema receipt: {error}"))?;
    Ok(sql.contains(&workspace_db_schema_contract_marker()))
}

async fn publish_workspace_db_schema_receipt(connection: &turso::Connection) -> Result<(), String> {
    connection
        .execute(
            "DROP TABLE IF EXISTS asp_workspace_db_schema_receipt_v1",
            (),
        )
        .await
        .map_err(|error| format!("failed to replace workspace DB schema receipt: {error}"))?;
    let schema_contract_marker = workspace_db_schema_contract_marker();
    let statement = format!(
        "CREATE TABLE {WORKSPACE_DB_SCHEMA_RECEIPT_TABLE} (
            schema_digest TEXT PRIMARY KEY
                CHECK(schema_digest = '{schema_contract_marker}')
        )"
    );
    connection
        .execute(&statement, ())
        .await
        .map_err(|error| format!("failed to publish workspace DB schema receipt: {error}"))?;
    Ok(())
}

pub(in crate::engine) async fn bootstrap_turso_source_index_schema(
    connection: &turso::Connection,
) -> Result<(), String> {
    if workspace_db_schema_is_current(connection).await? {
        return Ok(());
    }
    for statement in [
        "CREATE TABLE IF NOT EXISTS asp_source_index_scope_v1 (
            project_root TEXT NOT NULL,
            schema_id TEXT NOT NULL,
            schema_version TEXT NOT NULL,
            generation_id TEXT NOT NULL,
            file_hashes_json TEXT NOT NULL,
            source_snapshot_json TEXT NOT NULL DEFAULT '',
            selector_fingerprint TEXT NOT NULL DEFAULT '',
            owner_count INTEGER NOT NULL,
            selector_count INTEGER NOT NULL,
            updated_at_ms INTEGER NOT NULL,
            PRIMARY KEY (project_root, schema_id, schema_version)
        )",
        "CREATE TABLE IF NOT EXISTS asp_source_index_owner_v1 (
            project_root TEXT NOT NULL,
            schema_id TEXT NOT NULL,
            schema_version TEXT NOT NULL,
            generation_id TEXT NOT NULL,
            file_hash TEXT NOT NULL,
            owner_path TEXT NOT NULL,
            language_id TEXT,
            provider_id TEXT,
            source_kind TEXT NOT NULL,
            line_count INTEGER,
            query_keys_json TEXT NOT NULL,
            selector_facts_json TEXT NOT NULL,
            term_tokens_json TEXT NOT NULL,
            selector_count INTEGER NOT NULL,
            PRIMARY KEY (project_root, schema_id, schema_version, generation_id, owner_path)
        )",
        "CREATE TABLE IF NOT EXISTS asp_source_index_selector_v1 (
            project_root TEXT NOT NULL,
            schema_id TEXT NOT NULL,
            schema_version TEXT NOT NULL,
            generation_id TEXT NOT NULL,
            owner_path TEXT NOT NULL,
            owner_content_digest TEXT NOT NULL,
            language_id TEXT NOT NULL,
            parser_identity_digest TEXT NOT NULL,
            query_pack_digest TEXT NOT NULL,
            item_kind TEXT NOT NULL,
            item_symbol TEXT NOT NULL,
            scopes_json TEXT NOT NULL,
            structural_selector TEXT NOT NULL,
            PRIMARY KEY (
                project_root,
                schema_id,
                schema_version,
                generation_id,
                structural_selector
            )
        )",
        "CREATE TABLE IF NOT EXISTS asp_source_index_layout_v1 (
            project_root TEXT NOT NULL,
            schema_id TEXT NOT NULL,
            schema_version TEXT NOT NULL,
            term_projection_version INTEGER NOT NULL,
            token_projection_generation_id TEXT NOT NULL DEFAULT '',
            PRIMARY KEY (project_root, schema_id, schema_version)
        )",
        "CREATE TABLE IF NOT EXISTS asp_source_index_token_owner_v1 (
            project_root TEXT NOT NULL,
            schema_id TEXT NOT NULL,
            schema_version TEXT NOT NULL,
            generation_id TEXT NOT NULL,
            token TEXT NOT NULL,
            owner_path TEXT NOT NULL,
            PRIMARY KEY (project_root, schema_id, schema_version, generation_id, token, owner_path)
        )",
        "CREATE TABLE IF NOT EXISTS asp_exact_selector_projection_v1 (
            language_id TEXT NOT NULL,
            workspace_root_digest TEXT NOT NULL,
            owner_path TEXT NOT NULL,
            owner_subtree_digest TEXT NOT NULL,
            source_blob_digest TEXT NOT NULL,
            parser_identity_digest TEXT NOT NULL,
            query_pack_digest TEXT NOT NULL,
            structural_selector TEXT NOT NULL,
            projection_mode TEXT NOT NULL,
            record_json TEXT NOT NULL,
            PRIMARY KEY (
                language_id,
                workspace_root_digest,
                owner_path,
                owner_subtree_digest,
                source_blob_digest,
                parser_identity_digest,
                query_pack_digest,
                structural_selector,
                projection_mode
            )
        )",
    ] {
        execute_turso_statement(
            connection,
            statement,
            "failed to bootstrap Turso source-index schema",
        )
        .await?;
    }

    execute_turso_statement(
        connection,
        "CREATE INDEX IF NOT EXISTS asp_source_index_owner_v1_lookup_idx
            ON asp_source_index_owner_v1(project_root, schema_id, schema_version, generation_id, language_id, owner_path)",
        "failed to bootstrap Turso source-index schema",
    )
    .await?;
    execute_turso_statement(
        connection,
        "CREATE INDEX IF NOT EXISTS asp_source_index_selector_v1_identity_idx
            ON asp_source_index_selector_v1(
                project_root,
                schema_id,
                schema_version,
                generation_id,
                language_id,
                parser_identity_digest,
                query_pack_digest,
                item_kind,
                item_symbol,
                scopes_json
            )",
        "failed to bootstrap Turso source-index schema",
    )
    .await?;
    super::provider_incremental_schema::bootstrap_provider_incremental_schema(connection).await?;
    publish_workspace_db_schema_receipt(connection).await?;
    Ok(())
}
