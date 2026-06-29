use std::collections::BTreeSet;
use std::path::Path;

use agent_semantic_client_core::{
    CacheGenerationId, ClientCacheFileHash, SemanticSchemaId, SemanticSchemaVersion,
};
use rusqlite::{OptionalExtension, Transaction, params};

use crate::db::{ClientDb, normalized_project_root};

use super::text::{source_index_search_projection, u32_to_i64, usize_to_i64};
use super::types::{ClientDbSourceIndexImport, ClientDbSourceIndexStats};

pub(super) fn replace_source_index_rows(
    db: &mut ClientDb,
    import: &ClientDbSourceIndexImport,
) -> Result<ClientDbSourceIndexStats, String> {
    if import.file_hashes.is_empty() {
        return Err("source index import requires file hash evidence".to_string());
    }
    let file_hashes_json = source_index_file_hashes_json(&import.file_hashes)?;
    if let Some(stats) = reusable_generation_stats_for_json(
        db,
        &import.project_root,
        &import.schema_id,
        &import.schema_version,
        &file_hashes_json,
    )? {
        return Ok(stats);
    }
    let tx = db.conn.transaction().map_err(|error| {
        format!(
            "failed to start source index transaction at {}: {error}",
            db.db_path.display()
        )
    })?;
    write_generation(&tx, import, &file_hashes_json)?;
    clear_rows(&tx, &import.generation_id)?;
    write_owners(&tx, import)?;
    write_selectors(&tx, import)?;
    tx.commit()
        .map_err(|error| format!("failed to commit source index rows: {error}"))?;
    db.source_index_stats(&import.generation_id)
}

pub(super) fn reusable_source_index_generation_stats(
    db: &ClientDb,
    project_root: &Path,
    schema_id: &SemanticSchemaId,
    schema_version: &SemanticSchemaVersion,
    file_hashes: &[ClientCacheFileHash],
) -> Result<Option<ClientDbSourceIndexStats>, String> {
    if file_hashes.is_empty() {
        return Err("source index reuse requires file hash evidence".to_string());
    }
    let file_hashes_json = source_index_file_hashes_json(file_hashes)?;
    reusable_generation_stats_for_json(
        db,
        project_root,
        schema_id,
        schema_version,
        &file_hashes_json,
    )
}

pub(super) fn latest_source_index_file_hashes(
    db: &ClientDb,
    project_root: &Path,
    schema_id: &SemanticSchemaId,
    schema_version: &SemanticSchemaVersion,
) -> Result<Option<Vec<ClientCacheFileHash>>, String> {
    let project_root = normalized_project_root(project_root);
    let file_hashes_json = db
        .conn
        .query_row(
            "SELECT file_hashes_json
            FROM source_index_generation
            WHERE project_root = ?1
              AND schema_id = ?2
              AND schema_version = ?3
            ORDER BY updated_at DESC, generation_id DESC
            LIMIT 1",
            params![
                project_root.as_str(),
                schema_id.as_str(),
                schema_version.as_str(),
            ],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| format!("failed to read latest source index file hashes: {error}"))?;
    file_hashes_json
        .map(|value| {
            serde_json::from_str::<Vec<ClientCacheFileHash>>(&value).map_err(|error| {
                format!("failed to parse latest source index file hashes: {error}")
            })
        })
        .transpose()
}

fn reusable_generation_stats_for_json(
    db: &ClientDb,
    project_root: &Path,
    schema_id: &SemanticSchemaId,
    schema_version: &SemanticSchemaVersion,
    file_hashes_json: &str,
) -> Result<Option<ClientDbSourceIndexStats>, String> {
    let project_root = normalized_project_root(project_root);
    let generation_id = db
        .conn
        .query_row(
            "SELECT generation_id
            FROM source_index_generation
            WHERE project_root = ?1
              AND schema_id = ?2
              AND schema_version = ?3
              AND file_hashes_json = ?4
            ORDER BY updated_at DESC, generation_id DESC
            LIMIT 1",
            params![
                project_root.as_str(),
                schema_id.as_str(),
                schema_version.as_str(),
                file_hashes_json,
            ],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| format!("failed to read reusable source index generation: {error}"))?;
    generation_id
        .map(|generation_id| db.source_index_stats(&CacheGenerationId::from(generation_id)))
        .transpose()
}

fn source_index_file_hashes_json(file_hashes: &[ClientCacheFileHash]) -> Result<String, String> {
    let mut canonical = file_hashes.to_vec();
    canonical.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.byte_len.cmp(&right.byte_len))
            .then_with(|| left.mtime_ms.cmp(&right.mtime_ms))
            .then_with(|| left.sha256.cmp(&right.sha256))
    });
    serde_json::to_string(&canonical)
        .map_err(|error| format!("failed to serialize source index file hashes: {error}"))
}

fn write_generation(
    tx: &Transaction<'_>,
    import: &ClientDbSourceIndexImport,
    file_hashes_json: &str,
) -> Result<(), String> {
    let project_root = normalized_project_root(&import.project_root);
    tx.execute(
        "INSERT INTO source_index_generation (
            generation_id,
            project_root,
            schema_id,
            schema_version,
            file_hashes_json,
            raw_source_stored
        ) VALUES (?1, ?2, ?3, ?4, ?5, 0)
        ON CONFLICT(generation_id) DO UPDATE SET
            project_root = excluded.project_root,
            schema_id = excluded.schema_id,
            schema_version = excluded.schema_version,
            file_hashes_json = excluded.file_hashes_json,
            raw_source_stored = 0,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
        params![
            import.generation_id.as_str(),
            project_root.as_str(),
            import.schema_id.as_str(),
            import.schema_version.as_str(),
            file_hashes_json,
        ],
    )
    .map_err(|error| format!("failed to write source index generation: {error}"))?;
    Ok(())
}

fn clear_rows(tx: &Transaction<'_>, generation_id: &CacheGenerationId) -> Result<(), String> {
    for table in [
        "source_index_owner_key",
        "source_index_owner",
        "source_index_selector",
    ] {
        tx.execute(
            &format!("DELETE FROM {table} WHERE generation_id = ?1"),
            params![generation_id.as_str()],
        )
        .map_err(|error| format!("failed to clear {table} rows: {error}"))?;
    }
    Ok(())
}

fn write_owners(tx: &Transaction<'_>, import: &ClientDbSourceIndexImport) -> Result<(), String> {
    let mut insert_owner = tx
        .prepare(
            "INSERT INTO source_index_owner (
                generation_id,
                owner_ordinal,
                owner_path,
                language_id,
                provider_id,
                source_kind,
                line_count,
                query_keys_json,
                search_text
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )
        .map_err(|error| format!("failed to prepare source owner insert: {error}"))?;
    let mut insert_owner_key = tx
        .prepare(
            "INSERT INTO source_index_owner_key (
                generation_id,
                query_key,
                owner_ordinal,
                owner_path,
                language_id,
                provider_id
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .map_err(|error| format!("failed to prepare source owner key insert: {error}"))?;
    for (owner_ordinal, owner) in import.owners.iter().enumerate() {
        let search_projection = source_index_search_projection(
            [owner.owner_path.as_str(), owner.source_kind.as_str()],
            &owner.query_keys,
        )?;
        insert_owner
            .execute(params![
                import.generation_id.as_str(),
                usize_to_i64(owner_ordinal),
                owner.owner_path.as_str(),
                owner.language_id.as_ref().map(|value| value.as_str()),
                owner.provider_id.as_ref().map(|value| value.as_str()),
                owner.source_kind.as_str(),
                owner.line_count.map(u32_to_i64),
                search_projection.query_keys_json.as_str(),
                search_projection.search_text.as_str(),
            ])
            .map_err(|error| format!("failed to write source owner row: {error}"))?;
        for query_key in source_index_owner_query_keys(owner) {
            insert_owner_key
                .execute(params![
                    import.generation_id.as_str(),
                    query_key.as_str(),
                    usize_to_i64(owner_ordinal),
                    owner.owner_path.as_str(),
                    owner.language_id.as_ref().map(|value| value.as_str()),
                    owner.provider_id.as_ref().map(|value| value.as_str()),
                ])
                .map_err(|error| format!("failed to write source owner key row: {error}"))?;
        }
    }
    Ok(())
}

fn source_index_owner_query_keys(
    owner: &super::types::ClientDbSourceIndexOwner,
) -> BTreeSet<String> {
    let mut query_keys = BTreeSet::new();
    for value in [owner.owner_path.as_str(), owner.source_kind.as_str()] {
        insert_source_index_owner_query_key(&mut query_keys, value);
    }
    for query_key in &owner.query_keys {
        insert_source_index_owner_query_key(&mut query_keys, query_key.as_str());
    }
    query_keys
}

fn insert_source_index_owner_query_key(query_keys: &mut BTreeSet<String>, value: &str) {
    let value = value.trim();
    if !value.is_empty() {
        query_keys.insert(value.to_ascii_lowercase());
    }
}

fn write_selectors(tx: &Transaction<'_>, import: &ClientDbSourceIndexImport) -> Result<(), String> {
    let mut insert_selector = tx
        .prepare(
            "INSERT INTO source_index_selector (
                generation_id,
                selector_ordinal,
                owner_path,
                selector_id,
                symbol,
                kind,
                start_line,
                end_line,
                source,
                query_keys_json,
                search_text
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        )
        .map_err(|error| format!("failed to prepare source selector insert: {error}"))?;
    for (selector_ordinal, selector) in import.selectors.iter().enumerate() {
        let search_projection = source_index_search_projection(
            [
                selector.owner_path.as_str(),
                selector.selector_id.as_str(),
                selector.symbol.as_deref().unwrap_or_default(),
                selector.kind.as_deref().unwrap_or_default(),
            ],
            &selector.query_keys,
        )?;
        insert_selector
            .execute(params![
                import.generation_id.as_str(),
                usize_to_i64(selector_ordinal),
                selector.owner_path.as_str(),
                selector.selector_id.as_str(),
                selector.symbol.as_deref(),
                selector.kind.as_deref(),
                u32_to_i64(selector.start_line),
                u32_to_i64(selector.end_line),
                selector.source.as_str(),
                search_projection.query_keys_json.as_str(),
                search_projection.search_text.as_str(),
            ])
            .map_err(|error| format!("failed to write source selector row: {error}"))?;
    }
    Ok(())
}
