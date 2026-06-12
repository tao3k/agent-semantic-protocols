use agent_semantic_client_core::{CacheExportMethod, CacheGenerationId};
use rusqlite::{Transaction, params};

use crate::db::{ClientDb, normalized_project_root};

use super::text::{structural_search_projection, u32_to_i64, usize_to_i64};
use super::types::{
    ClientDbStructuralIndexImport, ClientDbStructuralIndexStats, ClientDbStructuralKind,
    ClientDbStructuralLocator, ClientDbStructuralName, ClientDbStructuralPath,
};

pub(super) fn replace_structural_index_rows(
    db: &mut ClientDb,
    import: &ClientDbStructuralIndexImport,
) -> Result<ClientDbStructuralIndexStats, String> {
    if import.file_hashes.is_empty() {
        return Err("structural index import requires file hash evidence".to_string());
    }
    let tx = db.conn.transaction().map_err(|error| {
        format!(
            "failed to start structural index transaction at {}: {error}",
            db.db_path.display()
        )
    })?;
    write_generation(&tx, import)?;
    clear_rows(&tx, &import.generation_id)?;
    write_owners(&tx, import)?;
    write_symbols(&tx, import)?;
    write_dependencies(&tx, import)?;
    tx.commit()
        .map_err(|error| format!("failed to commit structural index rows: {error}"))?;
    db.structural_index_stats(&import.generation_id)
}

fn write_generation(
    tx: &Transaction<'_>,
    import: &ClientDbStructuralIndexImport,
) -> Result<(), String> {
    let project_root = normalized_project_root(&import.project_root);
    let file_hashes_json = serde_json::to_string(&import.file_hashes)
        .map_err(|error| format!("failed to serialize structural index file hashes: {error}"))?;
    tx.execute(
        "INSERT INTO structural_index_generation (
            generation_id,
            language_id,
            provider_id,
            provider_version,
            export_method,
            project_root,
            package_root,
            schema_id,
            schema_version,
            source_artifact_id,
            file_hashes_json,
            raw_source_stored
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 0)
        ON CONFLICT(generation_id) DO UPDATE SET
            language_id = excluded.language_id,
            provider_id = excluded.provider_id,
            provider_version = excluded.provider_version,
            export_method = excluded.export_method,
            project_root = excluded.project_root,
            package_root = excluded.package_root,
            schema_id = excluded.schema_id,
            schema_version = excluded.schema_version,
            source_artifact_id = excluded.source_artifact_id,
            file_hashes_json = excluded.file_hashes_json,
            raw_source_stored = 0,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
        params![
            import.generation_id.as_str(),
            import.language_id.as_str(),
            import.provider_id.as_str(),
            import
                .provider_version
                .as_ref()
                .map(ClientDbStructuralName::as_str),
            import.export_method.as_ref().map(CacheExportMethod::as_str),
            project_root.as_str(),
            import
                .package_root
                .as_ref()
                .map(ClientDbStructuralPath::as_str),
            import.schema_id.as_str(),
            import.schema_version.as_str(),
            import
                .source_artifact_id
                .as_ref()
                .map(agent_semantic_client_core::CacheArtifactId::as_str),
            file_hashes_json,
        ],
    )
    .map_err(|error| format!("failed to write structural index generation: {error}"))?;
    Ok(())
}

fn clear_rows(tx: &Transaction<'_>, generation_id: &CacheGenerationId) -> Result<(), String> {
    for table in [
        "structural_index_owner",
        "structural_index_symbol",
        "structural_index_dependency_usage",
    ] {
        tx.execute(
            &format!("DELETE FROM {table} WHERE generation_id = ?1"),
            params![generation_id.as_str()],
        )
        .map_err(|error| format!("failed to clear {table} rows: {error}"))?;
    }
    Ok(())
}

fn write_owners(
    tx: &Transaction<'_>,
    import: &ClientDbStructuralIndexImport,
) -> Result<(), String> {
    let mut insert_owner = tx
        .prepare(
            "INSERT INTO structural_index_owner (
                generation_id,
                owner_ordinal,
                owner_path,
                owner_kind,
                source_authority,
                start_line,
                end_line,
                query_keys_json,
                search_text
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )
        .map_err(|error| format!("failed to prepare structural owner insert: {error}"))?;
    for (owner_ordinal, owner) in import.owners.iter().enumerate() {
        let search_projection = structural_search_projection(
            [owner.owner_path.as_str(), owner.owner_kind.as_str()],
            &owner.query_keys,
        )?;
        insert_owner
            .execute(params![
                import.generation_id.as_str(),
                usize_to_i64(owner_ordinal),
                owner.owner_path.as_str(),
                owner.owner_kind.as_str(),
                owner.source_authority.as_str(),
                owner.start_line.map(u32_to_i64),
                owner.end_line.map(u32_to_i64),
                search_projection.query_keys_json.as_str(),
                search_projection.search_text.as_str(),
            ])
            .map_err(|error| format!("failed to write structural owner row: {error}"))?;
    }
    Ok(())
}

fn write_symbols(
    tx: &Transaction<'_>,
    import: &ClientDbStructuralIndexImport,
) -> Result<(), String> {
    let mut insert_symbol = tx
        .prepare(
            "INSERT INTO structural_index_symbol (
                generation_id,
                symbol_ordinal,
                owner_path,
                name,
                kind,
                visibility,
                source_locator,
                query_keys_json,
                search_text
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )
        .map_err(|error| format!("failed to prepare structural symbol insert: {error}"))?;
    for (symbol_ordinal, symbol) in import.symbols.iter().enumerate() {
        let search_projection = structural_search_projection(
            [
                symbol.owner_path.as_str(),
                symbol.name.as_str(),
                symbol.kind.as_str(),
            ],
            &symbol.query_keys,
        )?;
        insert_symbol
            .execute(params![
                import.generation_id.as_str(),
                usize_to_i64(symbol_ordinal),
                symbol.owner_path.as_str(),
                symbol.name.as_str(),
                symbol.kind.as_str(),
                symbol
                    .visibility
                    .as_ref()
                    .map(ClientDbStructuralKind::as_str),
                symbol
                    .source_locator
                    .as_ref()
                    .map(ClientDbStructuralLocator::as_str),
                search_projection.query_keys_json.as_str(),
                search_projection.search_text.as_str(),
            ])
            .map_err(|error| format!("failed to write structural symbol row: {error}"))?;
    }
    Ok(())
}

fn write_dependencies(
    tx: &Transaction<'_>,
    import: &ClientDbStructuralIndexImport,
) -> Result<(), String> {
    let mut insert_dependency = tx
        .prepare(
            "INSERT INTO structural_index_dependency_usage (
                generation_id,
                usage_ordinal,
                owner_path,
                package_name,
                package_version,
                api_name,
                import_path,
                manifest_path,
                lockfile_hash,
                source,
                source_locator,
                query_keys_json,
                search_text
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        )
        .map_err(|error| format!("failed to prepare structural dependency insert: {error}"))?;
    for (usage_ordinal, usage) in import.dependency_usages.iter().enumerate() {
        let search_projection = structural_search_projection(
            [
                usage.owner_path.as_str(),
                usage.package_name.as_str(),
                usage
                    .api_name
                    .as_ref()
                    .map(ClientDbStructuralName::as_str)
                    .unwrap_or_default(),
                usage
                    .import_path
                    .as_ref()
                    .map(ClientDbStructuralPath::as_str)
                    .unwrap_or_default(),
                usage
                    .source_locator
                    .as_ref()
                    .map(ClientDbStructuralLocator::as_str)
                    .unwrap_or_default(),
            ],
            &usage.query_keys,
        )?;
        insert_dependency
            .execute(params![
                import.generation_id.as_str(),
                usize_to_i64(usage_ordinal),
                usage.owner_path.as_str(),
                usage.package_name.as_str(),
                usage
                    .package_version
                    .as_ref()
                    .map(ClientDbStructuralName::as_str),
                usage.api_name.as_ref().map(ClientDbStructuralName::as_str),
                usage
                    .import_path
                    .as_ref()
                    .map(ClientDbStructuralPath::as_str),
                usage
                    .manifest_path
                    .as_ref()
                    .map(ClientDbStructuralPath::as_str),
                usage
                    .lockfile_hash
                    .as_ref()
                    .map(super::types::ClientDbStructuralHash::as_str),
                usage.source.as_str(),
                usage
                    .source_locator
                    .as_ref()
                    .map(ClientDbStructuralLocator::as_str),
                search_projection.query_keys_json.as_str(),
                search_projection.search_text.as_str(),
            ])
            .map_err(|error| format!("failed to write structural dependency row: {error}"))?;
    }
    Ok(())
}
