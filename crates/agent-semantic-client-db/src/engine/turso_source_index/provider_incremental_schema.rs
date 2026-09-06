// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Turso 0.7 schema owned by provider-scoped incremental reasoning search.

pub(super) async fn bootstrap_provider_incremental_schema(
    connection: &turso::Connection,
) -> Result<(), String> {
    for statement in [
        "CREATE TABLE IF NOT EXISTS provider_active_generation_v1 (
            project_root TEXT NOT NULL,
            workspace_identity TEXT NOT NULL,
            provider_workspace_identity_digest TEXT NOT NULL,
            provider_id TEXT NOT NULL,
            language_id TEXT NOT NULL,
            provider_workspace_root TEXT NOT NULL,
            generation_id TEXT NOT NULL,
            owner_count INTEGER NOT NULL,
            updated_at_ms INTEGER NOT NULL,
            PRIMARY KEY (
                project_root,
                workspace_identity,
                provider_workspace_identity_digest,
                provider_id
            )
        )",
        "CREATE TABLE IF NOT EXISTS provider_owner_fingerprint_v1 (
            project_root TEXT NOT NULL,
            workspace_identity TEXT NOT NULL,
            provider_workspace_identity_digest TEXT NOT NULL,
            provider_id TEXT NOT NULL,
            language_id TEXT NOT NULL,
            owner_path TEXT NOT NULL,
            file_identity TEXT NOT NULL,
            size_bytes INTEGER NOT NULL,
            modified_unix_nanos INTEGER NOT NULL,
            change_time_unix_nanos INTEGER NOT NULL,
            content_digest TEXT NOT NULL,
            merkle_leaf_digest TEXT NOT NULL,
            source_bytes BLOB NOT NULL DEFAULT X'',
            PRIMARY KEY (
                project_root,
                workspace_identity,
                provider_workspace_identity_digest,
                provider_id,
                owner_path
            )
        )",
        "CREATE TABLE IF NOT EXISTS provider_selector_projection_v1 (
            project_root TEXT NOT NULL,
            workspace_identity TEXT NOT NULL,
            provider_workspace_identity_digest TEXT NOT NULL,
            provider_id TEXT NOT NULL,
            owner_path TEXT NOT NULL,
            source_content_digest TEXT NOT NULL,
            structural_selector TEXT NOT NULL,
            capture_name TEXT NOT NULL,
            signature TEXT NOT NULL,
            item_kind TEXT NOT NULL,
            item_name TEXT NOT NULL,
            source_byte_start INTEGER NOT NULL,
            source_byte_end INTEGER NOT NULL,
            PRIMARY KEY (
                project_root,
                workspace_identity,
                provider_workspace_identity_digest,
                provider_id,
                owner_path,
                structural_selector,
                capture_name,
                source_byte_start,
                source_byte_end
            )
        )",
        "CREATE TABLE IF NOT EXISTS provider_merkle_node_v1 (
            project_root TEXT NOT NULL,
            workspace_identity TEXT NOT NULL,
            provider_workspace_identity_digest TEXT NOT NULL,
            provider_id TEXT NOT NULL,
            node_key TEXT NOT NULL,
            parent_node_key TEXT NOT NULL,
            node_kind TEXT NOT NULL,
            digest TEXT NOT NULL,
            PRIMARY KEY (
                project_root,
                workspace_identity,
                provider_workspace_identity_digest,
                provider_id,
                node_key
            )
        )",
        "CREATE INDEX IF NOT EXISTS provider_selector_projection_v1_owner_idx
            ON provider_selector_projection_v1(
                project_root,
                workspace_identity,
                provider_workspace_identity_digest,
                provider_id,
                owner_path
            )",
        "CREATE INDEX IF NOT EXISTS provider_merkle_node_v1_parent_idx
            ON provider_merkle_node_v1(
                project_root,
                workspace_identity,
                provider_workspace_identity_digest,
                provider_id,
                parent_node_key,
                node_key
            )",
        "CREATE TABLE IF NOT EXISTS provider_owner_inventory_v1 (
            project_root TEXT NOT NULL,
            workspace_identity TEXT NOT NULL,
            provider_workspace_identity_digest TEXT NOT NULL,
            provider_id TEXT NOT NULL,
            language_id TEXT NOT NULL,
            provider_workspace_root TEXT NOT NULL,
            inventory_state TEXT NOT NULL,
            inventory_digest TEXT NOT NULL,
            inventory_generation TEXT NOT NULL,
            known_owner_count INTEGER NOT NULL,
            updated_at_ms INTEGER NOT NULL,
            PRIMARY KEY (
                project_root,
                workspace_identity,
                provider_workspace_identity_digest,
                provider_id
            )
        )",
        "CREATE TABLE IF NOT EXISTS provider_owner_inventory_entry_v1 (
            project_root TEXT NOT NULL,
            workspace_identity TEXT NOT NULL,
            provider_workspace_identity_digest TEXT NOT NULL,
            provider_id TEXT NOT NULL,
            inventory_generation TEXT NOT NULL,
            owner_path TEXT NOT NULL,
            owner_content_digest TEXT,
            owner_state TEXT NOT NULL,
            PRIMARY KEY (
                project_root,
                workspace_identity,
                provider_workspace_identity_digest,
                provider_id,
                owner_path
            )
        )",
        "CREATE TABLE IF NOT EXISTS provider_treesitter_query_v1 (
            project_root TEXT NOT NULL,
            workspace_identity TEXT NOT NULL,
            provider_workspace_identity_digest TEXT NOT NULL,
            provider_id TEXT NOT NULL,
            query_digest TEXT NOT NULL,
            capture_names_json TEXT NOT NULL,
            updated_at_ms INTEGER NOT NULL,
            PRIMARY KEY (
                project_root,
                workspace_identity,
                provider_workspace_identity_digest,
                provider_id,
                query_digest
            )
        )",
        "CREATE TABLE IF NOT EXISTS provider_treesitter_query_owner_v1 (
            project_root TEXT NOT NULL,
            workspace_identity TEXT NOT NULL,
            provider_workspace_identity_digest TEXT NOT NULL,
            provider_id TEXT NOT NULL,
            query_digest TEXT NOT NULL,
            owner_path TEXT NOT NULL,
            owner_content_digest TEXT NOT NULL,
            inventory_generation TEXT NOT NULL,
            capture_count INTEGER NOT NULL,
            complete_owner_refresh_count INTEGER NOT NULL,
            processed_at_ms INTEGER NOT NULL,
            PRIMARY KEY (
                project_root,
                workspace_identity,
                provider_workspace_identity_digest,
                provider_id,
                query_digest,
                owner_path,
                owner_content_digest
            )
        )",
        "CREATE TABLE IF NOT EXISTS provider_treesitter_capture_projection_v1 (
            project_root TEXT NOT NULL,
            workspace_identity TEXT NOT NULL,
            provider_workspace_identity_digest TEXT NOT NULL,
            provider_id TEXT NOT NULL,
            query_digest TEXT NOT NULL,
            owner_path TEXT NOT NULL,
            owner_content_digest TEXT NOT NULL,
            structural_selector TEXT NOT NULL,
            signature TEXT NOT NULL,
            item_kind TEXT NOT NULL,
            item_name TEXT NOT NULL,
            capture_name TEXT NOT NULL,
            item_source_byte_start INTEGER NOT NULL,
            item_source_byte_end INTEGER NOT NULL,
            source_byte_start INTEGER NOT NULL,
            source_byte_end INTEGER NOT NULL,
            PRIMARY KEY (
                project_root,
                workspace_identity,
                provider_workspace_identity_digest,
                provider_id,
                query_digest,
                owner_path,
                owner_content_digest,
                capture_name,
                source_byte_start,
                source_byte_end,
                structural_selector
            )
        )",
        "CREATE INDEX IF NOT EXISTS provider_owner_inventory_entry_v1_queue_idx
            ON provider_owner_inventory_entry_v1(
                project_root,
                workspace_identity,
                provider_workspace_identity_digest,
                provider_id,
                owner_path
            )",
        "CREATE INDEX IF NOT EXISTS provider_treesitter_capture_projection_v1_owner_idx
            ON provider_treesitter_capture_projection_v1(
                project_root,
                workspace_identity,
                provider_workspace_identity_digest,
                provider_id,
                query_digest,
                owner_path,
                owner_content_digest,
                source_byte_start
            )",
    ] {
        connection.execute(statement, ()).await.map_err(|error| {
            format!("failed to bootstrap provider incremental search schema: {error}")
        })?;
    }
    ensure_provider_owner_source_bytes(connection).await?;
    Ok(())
}

async fn ensure_provider_owner_source_bytes(connection: &turso::Connection) -> Result<(), String> {
    let mut rows = connection
        .query("PRAGMA table_info(provider_owner_fingerprint_v1)", ())
        .await
        .map_err(|error| format!("failed to inspect provider owner schema: {error}"))?;
    while let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read provider owner schema: {error}"))?
    {
        if row
            .get::<String>(1)
            .map_err(|error| format!("failed to decode provider owner schema column: {error}"))?
            == "source_bytes"
        {
            return Ok(());
        }
    }
    connection
        .execute(
            "ALTER TABLE provider_owner_fingerprint_v1
             ADD COLUMN source_bytes BLOB NOT NULL DEFAULT X''",
            (),
        )
        .await
        .map_err(|error| format!("failed to add provider owner source bytes: {error}"))?;
    Ok(())
}
