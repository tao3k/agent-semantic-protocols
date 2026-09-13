// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

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
    hasher.update(include_bytes!("canonical.rs"));
    hasher.update(&[0]);
    hasher.update(include_bytes!("generation_snapshot.rs"));
    hasher.update(&[0]);
    hasher.update(include_bytes!("relation.rs"));
    hasher.update(&[0]);
    hasher.update(include_bytes!("source_blob.rs"));
    hasher.update(&[0]);
    hasher.update(include_bytes!("materialization.rs"));
    hasher.update(&[0]);
    hasher.update(include_bytes!("provider_incremental_schema.rs"));
    format!(
        "{WORKSPACE_DB_SCHEMA_ID}/v{WORKSPACE_DB_SCHEMA_VERSION}/blake3-256:{}",
        hasher.finalize().to_hex()
    )
}

async fn reset_derived_generations_on_schema_contract_change(
    connection: &turso::Connection,
) -> Result<(), String> {
    connection
        .execute(
            "CREATE TABLE IF NOT EXISTS asp_workspace_db_schema_receipt_v1 (
                schema_digest TEXT PRIMARY KEY
            )",
            (),
        )
        .await
        .map_err(|error| format!("failed to inspect workspace DB schema receipt: {error}"))?;
    let mut rows = connection
        .query(
            "SELECT schema_digest FROM asp_workspace_db_schema_receipt_v1 LIMIT 1",
            (),
        )
        .await
        .map_err(|error| format!("failed to read workspace DB schema receipt: {error}"))?;
    let existing = rows
        .next()
        .await
        .map_err(|error| format!("failed to advance workspace DB schema receipt: {error}"))?
        .map(|row| {
            row.get::<String>(0)
                .map_err(|error| format!("failed to decode workspace DB schema receipt: {error}"))
        })
        .transpose()?;
    if existing.as_deref() == Some(workspace_db_schema_contract_marker().as_str()) {
        return Ok(());
    }

    // Every table below is a derived, generation-addressed projection. The
    // workspace source and provider receipts remain the rebuild authority, so
    // carrying rows across an incompatible schema would be ordinary legacy
    // decoding rather than a valid migration.
    for table in [
        "asp_workspace_generation_materialization_v1",
        "asp_source_index_relation_v1",
        "asp_source_index_token_owner_v1",
        "asp_source_index_selector_v1",
        "asp_source_index_owner_v1",
        "asp_source_index_layout_v1",
        "asp_source_index_scope_v1",
        "asp_source_index_blob_v1",
    ] {
        connection
            .execute(format!("DROP TABLE IF EXISTS {table}").as_str(), ())
            .await
            .map_err(|error| {
                format!("failed to reset incompatible workspace DB projection {table}: {error}")
            })?;
    }
    Ok(())
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
    reset_derived_generations_on_schema_contract_change(connection).await?;
    for statement in [
        "CREATE TABLE IF NOT EXISTS asp_source_index_blob_v1 (content_digest TEXT PRIMARY KEY NOT NULL, size_bytes INTEGER NOT NULL, source_bytes BLOB NOT NULL)",
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
        "CREATE TABLE IF NOT EXISTS asp_workspace_generation_materialization_v1 (
            workspace_identity TEXT NOT NULL,
            project_root TEXT NOT NULL,
            materialization_schema_id TEXT NOT NULL,
            materialization_schema_version TEXT NOT NULL,
            source_index_schema_id TEXT NOT NULL,
            source_index_schema_version TEXT NOT NULL,
            generation_id TEXT NOT NULL,
            source_snapshot_root_digest TEXT NOT NULL,
            import_digest TEXT NOT NULL,
            materialization_json BLOB NOT NULL,
            updated_at_ms INTEGER NOT NULL,
            PRIMARY KEY (
                workspace_identity,
                project_root,
                source_index_schema_id,
                source_index_schema_version,
                generation_id
            )
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
        "CREATE TABLE IF NOT EXISTS asp_source_index_relation_v1 (
            project_root TEXT NOT NULL,
            schema_id TEXT NOT NULL,
            schema_version TEXT NOT NULL,
            generation_id TEXT NOT NULL,
            owner_path TEXT NOT NULL CHECK(length(owner_path) > 0),
            from_kind TEXT NOT NULL CHECK(length(from_kind) > 0),
            from_id TEXT NOT NULL CHECK(length(from_id) > 0),
            relation_kind TEXT NOT NULL CHECK(length(relation_kind) > 0),
            to_kind TEXT NOT NULL CHECK(length(to_kind) > 0),
            to_id TEXT NOT NULL CHECK(length(to_id) > 0),
            PRIMARY KEY (
                project_root,
                schema_id,
                schema_version,
                generation_id,
                owner_path,
                from_kind,
                from_id,
                relation_kind,
                to_kind,
                to_id
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
    execute_turso_statement(
        connection,
        "CREATE INDEX IF NOT EXISTS asp_source_index_relation_v1_endpoint_idx
            ON asp_source_index_relation_v1(
                project_root,
                schema_id,
                schema_version,
                generation_id,
                from_kind,
                from_id
            )",
        "failed to bootstrap Turso source-index schema",
    )
    .await?;
    super::provider_incremental_schema::bootstrap_provider_incremental_schema(connection).await?;
    publish_workspace_db_schema_receipt(connection).await?;
    Ok(())
}
