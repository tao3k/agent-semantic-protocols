use agent_semantic_client_core::CacheGenerationId;
use rusqlite::params;

use crate::db::{ClientDb, normalized_project_root};

use super::text::{parse_query_keys, structural_like_query, u32_to_i64};
use super::types::{
    ClientDbStructuralDependencyUsage, ClientDbStructuralHash, ClientDbStructuralIndexLookup,
    ClientDbStructuralIndexStats, ClientDbStructuralKind, ClientDbStructuralLocator,
    ClientDbStructuralName, ClientDbStructuralPath, ClientDbStructuralSymbol,
};

pub(super) fn structural_index_stats(
    db: &ClientDb,
    generation_id: &CacheGenerationId,
) -> Result<ClientDbStructuralIndexStats, String> {
    Ok(ClientDbStructuralIndexStats {
        generation_id: generation_id.clone(),
        owner_count: count_generation_rows(
            db,
            "structural_index_owner",
            "SELECT COUNT(*) FROM structural_index_owner WHERE generation_id = ?1",
            generation_id,
        )?,
        symbol_count: count_generation_rows(
            db,
            "structural_index_symbol",
            "SELECT COUNT(*) FROM structural_index_symbol WHERE generation_id = ?1",
            generation_id,
        )?,
        dependency_usage_count: count_generation_rows(
            db,
            "structural_index_dependency_usage",
            "SELECT COUNT(*) FROM structural_index_dependency_usage WHERE generation_id = ?1",
            generation_id,
        )?,
    })
}

pub(super) fn lookup_structural_symbols(
    db: &ClientDb,
    lookup: &ClientDbStructuralIndexLookup,
) -> Result<Vec<ClientDbStructuralSymbol>, String> {
    if lookup.limit == 0 {
        return Ok(Vec::new());
    }
    let project_root = normalized_project_root(&lookup.project_root);
    let like_query = structural_like_query(lookup.query.as_str());
    let mut statement = db
        .conn
        .prepare(
            "SELECT
                s.owner_path,
                s.name,
                s.kind,
                s.visibility,
                s.source_locator,
                s.query_keys_json
            FROM structural_index_symbol s
            JOIN structural_index_generation g ON g.generation_id = s.generation_id
            WHERE g.language_id = ?1
              AND g.provider_id = ?2
              AND g.project_root = ?3
              AND s.search_text LIKE ?4 ESCAPE '\\'
            ORDER BY g.updated_at DESC, s.symbol_ordinal
            LIMIT ?5",
        )
        .map_err(|error| format!("failed to prepare structural symbol lookup: {error}"))?;
    let rows = statement
        .query_map(
            params![
                lookup.language_id.as_str(),
                lookup.provider_id.as_str(),
                project_root,
                like_query,
                u32_to_i64(lookup.limit),
            ],
            structural_symbol_from_row,
        )
        .map_err(|error| format!("failed to read structural symbols: {error}"))?;
    collect_rows(rows, "structural symbol")
}

pub(super) fn lookup_structural_dependency_usages(
    db: &ClientDb,
    lookup: &ClientDbStructuralIndexLookup,
) -> Result<Vec<ClientDbStructuralDependencyUsage>, String> {
    if lookup.limit == 0 {
        return Ok(Vec::new());
    }
    let project_root = normalized_project_root(&lookup.project_root);
    let like_query = structural_like_query(lookup.query.as_str());
    let mut statement = db
        .conn
        .prepare(
            "SELECT
                d.owner_path,
                d.package_name,
                d.package_version,
                d.api_name,
                d.import_path,
                d.manifest_path,
                d.lockfile_hash,
                d.source,
                d.source_locator,
                d.query_keys_json
            FROM structural_index_dependency_usage d
            JOIN structural_index_generation g ON g.generation_id = d.generation_id
            WHERE g.language_id = ?1
              AND g.provider_id = ?2
              AND g.project_root = ?3
              AND d.search_text LIKE ?4 ESCAPE '\\'
            ORDER BY g.updated_at DESC, d.usage_ordinal
            LIMIT ?5",
        )
        .map_err(|error| format!("failed to prepare structural dependency lookup: {error}"))?;
    let rows = statement
        .query_map(
            params![
                lookup.language_id.as_str(),
                lookup.provider_id.as_str(),
                project_root,
                like_query,
                u32_to_i64(lookup.limit),
            ],
            structural_dependency_from_row,
        )
        .map_err(|error| format!("failed to read structural dependencies: {error}"))?;
    collect_rows(rows, "structural dependency")
}

fn count_generation_rows(
    db: &ClientDb,
    table: &str,
    sql: &str,
    generation_id: &CacheGenerationId,
) -> Result<u32, String> {
    let count: i64 = db
        .conn
        .query_row(sql, params![generation_id.as_str()], |row| row.get(0))
        .map_err(|error| format!("failed to count {table} rows: {error}"))?;
    Ok(count.max(0).min(i64::from(u32::MAX)) as u32)
}

fn structural_symbol_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<ClientDbStructuralSymbol> {
    let query_keys_json = row.get::<_, String>(5)?;
    Ok(ClientDbStructuralSymbol {
        owner_path: ClientDbStructuralPath::new(row.get::<_, String>(0)?),
        name: ClientDbStructuralName::new(row.get::<_, String>(1)?),
        kind: ClientDbStructuralKind::new(row.get::<_, String>(2)?),
        visibility: row
            .get::<_, Option<String>>(3)?
            .map(ClientDbStructuralKind::new),
        source_locator: row
            .get::<_, Option<String>>(4)?
            .map(ClientDbStructuralLocator::new),
        query_keys: parse_query_keys(&query_keys_json, 5)?,
    })
}

fn structural_dependency_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<ClientDbStructuralDependencyUsage> {
    let query_keys_json = row.get::<_, String>(9)?;
    Ok(ClientDbStructuralDependencyUsage {
        owner_path: ClientDbStructuralPath::new(row.get::<_, String>(0)?),
        package_name: ClientDbStructuralName::new(row.get::<_, String>(1)?),
        package_version: row
            .get::<_, Option<String>>(2)?
            .map(ClientDbStructuralName::new),
        api_name: row
            .get::<_, Option<String>>(3)?
            .map(ClientDbStructuralName::new),
        import_path: row
            .get::<_, Option<String>>(4)?
            .map(ClientDbStructuralPath::new),
        manifest_path: row
            .get::<_, Option<String>>(5)?
            .map(ClientDbStructuralPath::new),
        lockfile_hash: row
            .get::<_, Option<String>>(6)?
            .map(ClientDbStructuralHash::new),
        source: super::types::ClientDbStructuralSource::new(row.get::<_, String>(7)?),
        source_locator: row
            .get::<_, Option<String>>(8)?
            .map(ClientDbStructuralLocator::new),
        query_keys: parse_query_keys(&query_keys_json, 9)?,
    })
}

fn collect_rows<T>(
    rows: impl Iterator<Item = rusqlite::Result<T>>,
    label: &str,
) -> Result<Vec<T>, String> {
    let mut values = Vec::new();
    for row in rows {
        values.push(row.map_err(|error| format!("failed to read {label}: {error}"))?);
    }
    Ok(values)
}
