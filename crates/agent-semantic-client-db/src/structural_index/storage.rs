use std::collections::{BTreeMap, BTreeSet};

use agent_semantic_client_core::{CacheExportMethod, CacheGenerationId, ClientCacheFileHash};
use rusqlite::{OptionalExtension, Transaction, params};

use crate::db::{ClientDb, normalized_project_root};

use super::text::{structural_search_projection, u32_to_i64, usize_to_i64};
use super::types::{
    ClientDbStructuralIndexImport, ClientDbStructuralIndexRefreshPlan,
    ClientDbStructuralIndexStats, ClientDbStructuralKind, ClientDbStructuralLocator,
    ClientDbStructuralName, ClientDbStructuralPath,
};

pub(super) fn plan_structural_index_refresh(
    db: &ClientDb,
    import: &ClientDbStructuralIndexImport,
) -> Result<ClientDbStructuralIndexRefreshPlan, String> {
    if import.file_hashes.is_empty() {
        return Err("structural index refresh requires file hash evidence".to_string());
    }
    let previous_generation = read_previous_structural_index_generation(db, import, false)?;
    build_structural_index_refresh_plan(
        import,
        previous_generation
            .as_ref()
            .map(|generation| generation.file_hashes_json.as_str()),
    )
}

pub(super) fn apply_structural_index_refresh_rows(
    db: &mut ClientDb,
    import: &ClientDbStructuralIndexImport,
) -> Result<ClientDbStructuralIndexStats, String> {
    if import.file_hashes.is_empty() {
        return Err("structural index refresh requires file hash evidence".to_string());
    }
    let previous_generation = read_previous_structural_index_generation(db, import, true)?;
    let plan = build_structural_index_refresh_plan(
        import,
        previous_generation
            .as_ref()
            .map(|generation| generation.file_hashes_json.as_str()),
    )?;
    let changed_paths = structural_path_set(&plan.changed_paths);
    let unchanged_paths = structural_path_set(&plan.unchanged_paths);
    let tx = db.conn.transaction().map_err(|error| {
        format!(
            "failed to start structural index transaction at {}: {error}",
            db.db_path.display()
        )
    })?;
    write_generation(&tx, import)?;
    clear_rows(&tx, &import.generation_id)?;

    let owner_count = write_owners_from(&tx, import, Some(&changed_paths), 0)?;
    let symbol_count = write_symbols_from(&tx, import, Some(&changed_paths), 0)?;
    let dependency_usage_count = write_dependencies_from(&tx, import, Some(&changed_paths), 0)?;
    if let Some(previous_generation) = previous_generation.as_ref() {
        fill_refresh_paths(&tx, &unchanged_paths)?;
        copy_unchanged_rows(
            &tx,
            import,
            previous_generation.generation_id.as_str(),
            owner_count,
            symbol_count,
            dependency_usage_count,
        )?;
    }
    tx.commit()
        .map_err(|error| format!("failed to commit structural index refresh rows: {error}"))?;
    db.structural_index_stats(&import.generation_id)
}

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

struct PreviousStructuralIndexGeneration {
    generation_id: CacheGenerationId,
    file_hashes_json: String,
}

fn read_previous_structural_index_generation(
    db: &ClientDb,
    import: &ClientDbStructuralIndexImport,
    exclude_current_generation: bool,
) -> Result<Option<PreviousStructuralIndexGeneration>, String> {
    let project_root = normalized_project_root(&import.project_root);
    let package_root = import
        .package_root
        .as_ref()
        .map(ClientDbStructuralPath::as_str);
    db.conn
        .query_row(
            "SELECT generation_id, file_hashes_json
             FROM structural_index_generation
             WHERE language_id = ?1
               AND provider_id = ?2
               AND project_root = ?3
               AND ((package_root IS NULL AND ?4 IS NULL) OR package_root = ?4)
               AND (?5 = 0 OR generation_id <> ?6)
             ORDER BY updated_at DESC, generation_id DESC
             LIMIT 1",
            params![
                import.language_id.as_str(),
                import.provider_id.as_str(),
                project_root.as_str(),
                package_root,
                if exclude_current_generation { 1 } else { 0 },
                import.generation_id.as_str(),
            ],
            |row| {
                Ok(PreviousStructuralIndexGeneration {
                    generation_id: CacheGenerationId::from(row.get::<_, String>(0)?),
                    file_hashes_json: row.get::<_, String>(1)?,
                })
            },
        )
        .optional()
        .map_err(|error| format!("failed to read previous structural index file hashes: {error}"))
}

fn build_structural_index_refresh_plan(
    import: &ClientDbStructuralIndexImport,
    previous_file_hashes_json: Option<&str>,
) -> Result<ClientDbStructuralIndexRefreshPlan, String> {
    let current = file_hash_map(&import.file_hashes);
    let Some(previous_file_hashes_json) = previous_file_hashes_json else {
        return Ok(ClientDbStructuralIndexRefreshPlan {
            unchanged_paths: Vec::new(),
            changed_paths: current
                .keys()
                .cloned()
                .map(ClientDbStructuralPath::from)
                .collect(),
            deleted_paths: Vec::new(),
        });
    };
    let previous_file_hashes =
        serde_json::from_str::<Vec<ClientCacheFileHash>>(previous_file_hashes_json)
            .map_err(|error| format!("failed to decode structural index file hashes: {error}"))?;
    let previous = file_hash_map(&previous_file_hashes);

    let unchanged_paths = current
        .iter()
        .filter(|(path, sha256)| previous.get(*path) == Some(*sha256))
        .map(|(path, _)| ClientDbStructuralPath::from(path.clone()))
        .collect();
    let changed_paths = current
        .iter()
        .filter(|(path, sha256)| previous.get(*path) != Some(*sha256))
        .map(|(path, _)| ClientDbStructuralPath::from(path.clone()))
        .collect();
    let deleted_paths = previous
        .keys()
        .filter(|path| !current.contains_key(*path))
        .cloned()
        .map(ClientDbStructuralPath::from)
        .collect();

    Ok(ClientDbStructuralIndexRefreshPlan {
        unchanged_paths,
        changed_paths,
        deleted_paths,
    })
}

fn file_hash_map(file_hashes: &[ClientCacheFileHash]) -> BTreeMap<String, String> {
    file_hashes
        .iter()
        .map(|file_hash| (file_hash.path.clone(), file_hash.sha256.clone()))
        .collect()
}

fn structural_path_set(paths: &[ClientDbStructuralPath]) -> BTreeSet<String> {
    paths.iter().map(|path| path.as_str().to_string()).collect()
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
    write_owners_from(tx, import, None, 0).map(|_| ())
}

fn write_owners_from(
    tx: &Transaction<'_>,
    import: &ClientDbStructuralIndexImport,
    included_paths: Option<&BTreeSet<String>>,
    ordinal_start: usize,
) -> Result<usize, String> {
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
    let mut owner_count = 0usize;
    for (owner_ordinal, owner) in import
        .owners
        .iter()
        .filter(|owner| includes_owner_path(included_paths, owner.owner_path.as_str()))
        .enumerate()
    {
        let search_projection = structural_search_projection(
            [owner.owner_path.as_str(), owner.owner_kind.as_str()],
            &owner.query_keys,
        )?;
        insert_owner
            .execute(params![
                import.generation_id.as_str(),
                usize_to_i64(ordinal_start + owner_ordinal),
                owner.owner_path.as_str(),
                owner.owner_kind.as_str(),
                owner.source_authority.as_str(),
                owner.start_line.map(u32_to_i64),
                owner.end_line.map(u32_to_i64),
                search_projection.query_keys_json.as_str(),
                search_projection.search_text.as_str(),
            ])
            .map_err(|error| format!("failed to write structural owner row: {error}"))?;
        owner_count += 1;
    }
    Ok(owner_count)
}

fn write_symbols(
    tx: &Transaction<'_>,
    import: &ClientDbStructuralIndexImport,
) -> Result<(), String> {
    write_symbols_from(tx, import, None, 0).map(|_| ())
}

fn write_symbols_from(
    tx: &Transaction<'_>,
    import: &ClientDbStructuralIndexImport,
    included_paths: Option<&BTreeSet<String>>,
    ordinal_start: usize,
) -> Result<usize, String> {
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
    let mut symbol_count = 0usize;
    for (symbol_ordinal, symbol) in import
        .symbols
        .iter()
        .filter(|symbol| includes_owner_path(included_paths, symbol.owner_path.as_str()))
        .enumerate()
    {
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
                usize_to_i64(ordinal_start + symbol_ordinal),
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
        symbol_count += 1;
    }
    Ok(symbol_count)
}

fn write_dependencies(
    tx: &Transaction<'_>,
    import: &ClientDbStructuralIndexImport,
) -> Result<(), String> {
    write_dependencies_from(tx, import, None, 0).map(|_| ())
}

fn write_dependencies_from(
    tx: &Transaction<'_>,
    import: &ClientDbStructuralIndexImport,
    included_paths: Option<&BTreeSet<String>>,
    ordinal_start: usize,
) -> Result<usize, String> {
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
    let mut dependency_usage_count = 0usize;
    for (usage_ordinal, usage) in import
        .dependency_usages
        .iter()
        .filter(|usage| includes_owner_path(included_paths, usage.owner_path.as_str()))
        .enumerate()
    {
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
                usize_to_i64(ordinal_start + usage_ordinal),
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
        dependency_usage_count += 1;
    }
    Ok(dependency_usage_count)
}

fn includes_owner_path(included_paths: Option<&BTreeSet<String>>, owner_path: &str) -> bool {
    match included_paths {
        Some(paths) => paths.contains(owner_path),
        None => true,
    }
}

fn fill_refresh_paths(tx: &Transaction<'_>, paths: &BTreeSet<String>) -> Result<(), String> {
    tx.execute(
        "CREATE TEMP TABLE IF NOT EXISTS structural_index_refresh_path (
            path TEXT PRIMARY KEY
        )",
        [],
    )
    .map_err(|error| format!("failed to create structural index refresh path table: {error}"))?;
    tx.execute("DELETE FROM structural_index_refresh_path", [])
        .map_err(|error| format!("failed to clear structural index refresh path table: {error}"))?;
    let mut insert_path = tx
        .prepare("INSERT INTO structural_index_refresh_path(path) VALUES (?1)")
        .map_err(|error| {
            format!("failed to prepare structural index refresh path insert: {error}")
        })?;
    for path in paths {
        insert_path
            .execute(params![path.as_str()])
            .map_err(|error| format!("failed to write structural index refresh path: {error}"))?;
    }
    Ok(())
}

fn copy_unchanged_rows(
    tx: &Transaction<'_>,
    import: &ClientDbStructuralIndexImport,
    previous_generation_id: &str,
    owner_ordinal_start: usize,
    symbol_ordinal_start: usize,
    dependency_usage_ordinal_start: usize,
) -> Result<(), String> {
    copy_unchanged_owners(tx, import, previous_generation_id, owner_ordinal_start)?;
    copy_unchanged_symbols(tx, import, previous_generation_id, symbol_ordinal_start)?;
    copy_unchanged_dependencies(
        tx,
        import,
        previous_generation_id,
        dependency_usage_ordinal_start,
    )?;
    tx.execute("DELETE FROM structural_index_refresh_path", [])
        .map_err(|error| format!("failed to clear structural index refresh path table: {error}"))?;
    Ok(())
}

fn copy_unchanged_owners(
    tx: &Transaction<'_>,
    import: &ClientDbStructuralIndexImport,
    previous_generation_id: &str,
    ordinal_start: usize,
) -> Result<(), String> {
    tx.execute(
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
        )
        SELECT
            ?1,
            ?2 + ROW_NUMBER() OVER (ORDER BY owner_ordinal) - 1,
            owner_path,
            owner_kind,
            source_authority,
            start_line,
            end_line,
            query_keys_json,
            search_text
        FROM structural_index_owner
        WHERE generation_id = ?3
          AND owner_path IN (SELECT path FROM structural_index_refresh_path)
        ORDER BY owner_ordinal",
        params![
            import.generation_id.as_str(),
            usize_to_i64(ordinal_start),
            previous_generation_id,
        ],
    )
    .map_err(|error| format!("failed to copy unchanged structural owners: {error}"))?;
    Ok(())
}

fn copy_unchanged_symbols(
    tx: &Transaction<'_>,
    import: &ClientDbStructuralIndexImport,
    previous_generation_id: &str,
    ordinal_start: usize,
) -> Result<(), String> {
    tx.execute(
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
        )
        SELECT
            ?1,
            ?2 + ROW_NUMBER() OVER (ORDER BY symbol_ordinal) - 1,
            owner_path,
            name,
            kind,
            visibility,
            source_locator,
            query_keys_json,
            search_text
        FROM structural_index_symbol
        WHERE generation_id = ?3
          AND owner_path IN (SELECT path FROM structural_index_refresh_path)
        ORDER BY symbol_ordinal",
        params![
            import.generation_id.as_str(),
            usize_to_i64(ordinal_start),
            previous_generation_id,
        ],
    )
    .map_err(|error| format!("failed to copy unchanged structural symbols: {error}"))?;
    Ok(())
}

fn copy_unchanged_dependencies(
    tx: &Transaction<'_>,
    import: &ClientDbStructuralIndexImport,
    previous_generation_id: &str,
    ordinal_start: usize,
) -> Result<(), String> {
    tx.execute(
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
        )
        SELECT
            ?1,
            ?2 + ROW_NUMBER() OVER (ORDER BY usage_ordinal) - 1,
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
        FROM structural_index_dependency_usage
        WHERE generation_id = ?3
          AND owner_path IN (SELECT path FROM structural_index_refresh_path)
        ORDER BY usage_ordinal",
        params![
            import.generation_id.as_str(),
            usize_to_i64(ordinal_start),
            previous_generation_id,
        ],
    )
    .map_err(|error| format!("failed to copy unchanged structural dependencies: {error}"))?;
    Ok(())
}
