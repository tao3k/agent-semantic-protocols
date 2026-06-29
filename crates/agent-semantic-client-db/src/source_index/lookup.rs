use agent_semantic_client_core::CacheGenerationId;
use agent_semantic_client_core::{SemanticSchemaId, SemanticSchemaVersion};
use rusqlite::{Error as SqlError, params};

use crate::db::{ClientDb, normalized_project_root};

use super::text::{parse_query_keys, source_index_like_query, u32_to_i64};
use super::types::{
    ClientDbSourceIndexLookup, ClientDbSourceIndexOwner, ClientDbSourceIndexPath,
    ClientDbSourceIndexSelector, ClientDbSourceIndexSelectorLookup, ClientDbSourceIndexSource,
    ClientDbSourceIndexStats,
};

pub(super) fn source_index_stats(
    db: &ClientDb,
    generation_id: &CacheGenerationId,
) -> Result<ClientDbSourceIndexStats, String> {
    Ok(ClientDbSourceIndexStats {
        generation_id: generation_id.clone(),
        owner_count: count_generation_rows(
            db,
            "source_index_owner",
            "SELECT COUNT(*) FROM source_index_owner WHERE generation_id = ?1",
            generation_id,
        )?,
        selector_count: count_generation_rows(
            db,
            "source_index_selector",
            "SELECT COUNT(*) FROM source_index_selector WHERE generation_id = ?1",
            generation_id,
        )?,
    })
}

pub(super) fn lookup_source_index_owners(
    db: &ClientDb,
    lookup: &ClientDbSourceIndexLookup,
) -> Result<Vec<ClientDbSourceIndexOwner>, String> {
    if lookup.limit == 0 {
        return Ok(Vec::new());
    }
    let project_root = normalized_project_root(&lookup.project_root);
    let exact_query = lookup.query.as_str().trim().to_ascii_lowercase();
    if !exact_query.is_empty() {
        let exact_owners =
            lookup_source_index_owners_by_key(db, lookup, &project_root, &exact_query)?;
        if !exact_owners.is_empty() {
            return Ok(exact_owners);
        }
    }
    let like_query = source_index_like_query(lookup.query.as_str());
    if let Some(language_id) = &lookup.language_id {
        let mut statement = db
            .conn
            .prepare_cached(
                "SELECT
                o.owner_path,
                o.language_id,
                o.provider_id,
                o.source_kind,
                o.line_count,
                o.query_keys_json
            FROM source_index_owner o
            JOIN source_index_generation g ON g.generation_id = o.generation_id
            WHERE g.project_root = ?1
              AND g.generation_id = (
                SELECT latest.generation_id
                FROM source_index_generation latest
                WHERE latest.project_root = ?1
                ORDER BY latest.updated_at DESC, latest.generation_id DESC
                LIMIT 1
              )
              AND o.search_text LIKE ?2 ESCAPE '\\'
              AND o.language_id = ?3
            ORDER BY g.updated_at DESC, o.owner_ordinal
            LIMIT ?4",
            )
            .map_err(|error| format!("failed to prepare source owner lookup: {error}"))?;
        let rows = statement
            .query_map(
                params![
                    project_root,
                    like_query,
                    language_id.as_str(),
                    u32_to_i64(lookup.limit)
                ],
                source_owner_from_row,
            )
            .map_err(|error| format!("failed to read source owners: {error}"))?;
        return collect_rows(rows, "source owner");
    }
    let mut statement = db
        .conn
        .prepare_cached(
            "SELECT
                o.owner_path,
                o.language_id,
                o.provider_id,
                o.source_kind,
                o.line_count,
                o.query_keys_json
            FROM source_index_owner o
            JOIN source_index_generation g ON g.generation_id = o.generation_id
            WHERE g.project_root = ?1
              AND g.generation_id = (
                SELECT latest.generation_id
                FROM source_index_generation latest
                WHERE latest.project_root = ?1
                ORDER BY latest.updated_at DESC, latest.generation_id DESC
                LIMIT 1
              )
              AND o.search_text LIKE ?2 ESCAPE '\\'
            ORDER BY g.updated_at DESC, o.owner_ordinal
            LIMIT ?3",
        )
        .map_err(|error| format!("failed to prepare source owner lookup: {error}"))?;
    let rows = statement
        .query_map(
            params![project_root, like_query, u32_to_i64(lookup.limit)],
            source_owner_from_row,
        )
        .map_err(|error| format!("failed to read source owners: {error}"))?;
    collect_rows(rows, "source owner")
}

fn lookup_source_index_owners_by_key(
    db: &ClientDb,
    lookup: &ClientDbSourceIndexLookup,
    project_root: &str,
    exact_query: &str,
) -> Result<Vec<ClientDbSourceIndexOwner>, String> {
    if let Some(language_id) = &lookup.language_id {
        let mut statement = match db.conn.prepare_cached(
            "SELECT
                o.owner_path,
                o.language_id,
                o.provider_id,
                o.source_kind,
                o.line_count,
                o.query_keys_json
            FROM source_index_owner_key k
            JOIN source_index_owner o
              ON o.generation_id = k.generation_id
             AND o.owner_ordinal = k.owner_ordinal
            JOIN source_index_generation g ON g.generation_id = k.generation_id
            WHERE g.project_root = ?1
              AND g.generation_id = (
                SELECT latest.generation_id
                FROM source_index_generation latest
                WHERE latest.project_root = ?1
                ORDER BY latest.updated_at DESC, latest.generation_id DESC
                LIMIT 1
              )
              AND k.query_key = ?2
              AND k.language_id = ?3
            ORDER BY o.owner_ordinal
            LIMIT ?4",
        ) {
            Ok(statement) => statement,
            Err(error) if missing_owner_key_table(&error) => return Ok(Vec::new()),
            Err(error) => {
                return Err(format!(
                    "failed to prepare source owner key lookup: {error}"
                ));
            }
        };
        let rows = statement
            .query_map(
                params![
                    project_root,
                    exact_query,
                    language_id.as_str(),
                    u32_to_i64(lookup.limit)
                ],
                source_owner_from_row,
            )
            .map_err(|error| format!("failed to read source owner keys: {error}"))?;
        return collect_rows(rows, "source owner key");
    }
    let mut statement = match db.conn.prepare_cached(
        "SELECT
                o.owner_path,
                o.language_id,
                o.provider_id,
                o.source_kind,
                o.line_count,
                o.query_keys_json
            FROM source_index_owner_key k
            JOIN source_index_owner o
              ON o.generation_id = k.generation_id
             AND o.owner_ordinal = k.owner_ordinal
            JOIN source_index_generation g ON g.generation_id = k.generation_id
            WHERE g.project_root = ?1
              AND g.generation_id = (
                SELECT latest.generation_id
                FROM source_index_generation latest
                WHERE latest.project_root = ?1
                ORDER BY latest.updated_at DESC, latest.generation_id DESC
                LIMIT 1
              )
              AND k.query_key = ?2
            ORDER BY o.owner_ordinal
            LIMIT ?3",
    ) {
        Ok(statement) => statement,
        Err(error) if missing_owner_key_table(&error) => return Ok(Vec::new()),
        Err(error) => {
            return Err(format!(
                "failed to prepare source owner key lookup: {error}"
            ));
        }
    };
    let rows = statement
        .query_map(
            params![project_root, exact_query, u32_to_i64(lookup.limit)],
            source_owner_from_row,
        )
        .map_err(|error| format!("failed to read source owner keys: {error}"))?;
    collect_rows(rows, "source owner key")
}

fn missing_owner_key_table(error: &SqlError) -> bool {
    matches!(
        error,
        SqlError::SqliteFailure(_, Some(message))
            if message.contains("no such table: source_index_owner_key")
    )
}

pub(super) fn latest_source_index_generation_owners(
    db: &ClientDb,
    project_root: &std::path::Path,
    schema_id: &SemanticSchemaId,
    schema_version: &SemanticSchemaVersion,
) -> Result<Vec<ClientDbSourceIndexOwner>, String> {
    let project_root = normalized_project_root(project_root);
    let mut statement = db
        .conn
        .prepare_cached(
            "SELECT
                o.owner_path,
                o.language_id,
                o.provider_id,
                o.source_kind,
                o.line_count,
                o.query_keys_json
            FROM source_index_owner o
            JOIN source_index_generation g ON g.generation_id = o.generation_id
            WHERE g.project_root = ?1
              AND g.schema_id = ?2
              AND g.schema_version = ?3
              AND g.generation_id = (
                SELECT latest.generation_id
                FROM source_index_generation latest
                WHERE latest.project_root = ?1
                  AND latest.schema_id = ?2
                  AND latest.schema_version = ?3
                ORDER BY latest.updated_at DESC, latest.generation_id DESC
                LIMIT 1
              )
            ORDER BY o.owner_ordinal",
        )
        .map_err(|error| format!("failed to prepare latest source owner lookup: {error}"))?;
    let rows = statement
        .query_map(
            params![project_root, schema_id.as_str(), schema_version.as_str()],
            source_owner_from_row,
        )
        .map_err(|error| format!("failed to read latest source owners: {error}"))?;
    collect_rows(rows, "latest source owner")
}

pub(super) fn lookup_source_index_selectors(
    db: &ClientDb,
    lookup: &ClientDbSourceIndexSelectorLookup,
) -> Result<Vec<ClientDbSourceIndexSelector>, String> {
    if lookup.limit == 0 {
        return Ok(Vec::new());
    }
    let project_root = normalized_project_root(&lookup.project_root);
    let like_query = lookup
        .query
        .as_ref()
        .map(|query| source_index_like_query(query.as_str()));
    let mut statement = db
        .conn
        .prepare_cached(
            "SELECT
                s.owner_path,
                s.selector_id,
                s.symbol,
                s.kind,
                s.start_line,
                s.end_line,
                s.source,
                s.query_keys_json
            FROM source_index_selector s
            JOIN source_index_generation g ON g.generation_id = s.generation_id
            JOIN source_index_owner o
              ON o.generation_id = s.generation_id
             AND o.owner_path = s.owner_path
            WHERE g.project_root = ?1
              AND g.generation_id = (
                SELECT latest.generation_id
                FROM source_index_generation latest
                WHERE latest.project_root = ?1
                ORDER BY latest.updated_at DESC, latest.generation_id DESC
                LIMIT 1
              )
              AND (?2 IS NULL OR o.language_id = ?2)
              AND (?3 IS NULL OR s.kind = ?3)
              AND (?4 IS NULL OR s.search_text LIKE ?4 ESCAPE '\\')
            ORDER BY g.updated_at DESC, s.selector_ordinal
            LIMIT ?5",
        )
        .map_err(|error| format!("failed to prepare source selector lookup: {error}"))?;
    let rows = statement
        .query_map(
            params![
                project_root,
                lookup.language_id.as_ref().map(|value| value.as_str()),
                lookup.kind.as_deref(),
                like_query.as_deref(),
                u32_to_i64(lookup.limit),
            ],
            source_selector_from_row,
        )
        .map_err(|error| format!("failed to read source selectors: {error}"))?;
    collect_rows(rows, "source selector")
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

fn source_owner_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ClientDbSourceIndexOwner> {
    let query_keys_json = row.get::<_, String>(5)?;
    Ok(ClientDbSourceIndexOwner {
        owner_path: ClientDbSourceIndexPath::new(row.get::<_, String>(0)?),
        language_id: row
            .get::<_, Option<String>>(1)?
            .map(agent_semantic_client_core::LanguageId::from),
        provider_id: row
            .get::<_, Option<String>>(2)?
            .map(agent_semantic_client_core::ProviderId::from),
        source_kind: ClientDbSourceIndexSource::new(row.get::<_, String>(3)?),
        line_count: row
            .get::<_, Option<i64>>(4)?
            .map(|value| value.max(0).min(i64::from(u32::MAX)) as u32),
        query_keys: parse_query_keys(&query_keys_json, 5)?,
    })
}

fn source_selector_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<ClientDbSourceIndexSelector> {
    let query_keys_json = row.get::<_, String>(7)?;
    Ok(ClientDbSourceIndexSelector {
        owner_path: ClientDbSourceIndexPath::new(row.get::<_, String>(0)?),
        selector_id: row.get::<_, String>(1)?,
        symbol: row.get::<_, Option<String>>(2)?,
        kind: row.get::<_, Option<String>>(3)?,
        start_line: row.get::<_, i64>(4)?.max(0).min(i64::from(u32::MAX)) as u32,
        end_line: row.get::<_, i64>(5)?.max(0).min(i64::from(u32::MAX)) as u32,
        source: ClientDbSourceIndexSource::new(row.get::<_, String>(6)?),
        query_keys: parse_query_keys(&query_keys_json, 7)?,
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
