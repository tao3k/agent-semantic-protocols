use super::canonical::turso_source_index_canonical_selectors_by_owner;
use super::core::turso_source_index_selector_fingerprint;
use crate::engine::turso_statement::run_turso_operation;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct TursoSourceIndexOwnerRow {
    pub(super) file_hash: String,
    pub(super) owner_path: String,
    pub(super) language_id: Option<String>,
    pub(super) provider_id: Option<String>,
    pub(super) source_kind: String,
    pub(super) line_count: Option<i64>,
    pub(super) query_keys_json: String,
    pub(super) selector_facts_json: String,
    pub(super) term_tokens_json: String,
    pub(super) selector_count: i64,
}

async fn active_turso_source_index_generation(
    connection: &turso::Connection,
    project_root: &str,
    schema_id: &str,
    schema_version: &str,
) -> Result<Option<(String, String)>, String> {
    let mut rows = run_turso_operation(
        || async {
            connection
                .query(
                    "SELECT generation_id, selector_fingerprint
                     FROM asp_source_index_scope_v1
                     WHERE project_root = ?1
                       AND schema_id = ?2
                       AND schema_version = ?3
                     LIMIT 1",
                    (project_root, schema_id, schema_version),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to resolve active Turso source-index generation",
    )
    .await?;
    let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read active Turso source-index generation: {error}"))?
    else {
        return Ok(None);
    };
    Ok(Some((
        row.get::<String>(0).map_err(|error| {
            format!("failed to decode active Turso source-index generation: {error}")
        })?,
        row.get::<String>(1).map_err(|error| {
            format!("failed to decode active Turso source-index selector fingerprint: {error}")
        })?,
    )))
}

async fn active_turso_source_index_owner_rows(
    connection: &turso::Connection,
    project_root: &str,
    schema_id: &str,
    schema_version: &str,
) -> Result<
    (
        String,
        std::collections::HashMap<String, TursoSourceIndexOwnerRow>,
    ),
    String,
> {
    let Some((generation_id, _)) =
        active_turso_source_index_generation(connection, project_root, schema_id, schema_version)
            .await?
    else {
        return Ok((String::new(), std::collections::HashMap::new()));
    };
    let mut rows = run_turso_operation(
        || async {
            connection
                .query(
                    "SELECT file_hash, owner_path, language_id, provider_id, source_kind,
                            line_count, query_keys_json, selector_facts_json,
                            term_tokens_json, selector_count
                     FROM asp_source_index_owner_v1
                     WHERE project_root = ?1
                       AND schema_id = ?2
                       AND schema_version = ?3
                       AND generation_id = ?4",
                    (
                        project_root,
                        schema_id,
                        schema_version,
                        generation_id.as_str(),
                    ),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to load active Turso source-index owners",
    )
    .await?;
    let mut owner_rows = std::collections::HashMap::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read active Turso source-index owner: {error}"))?
    {
        let owner = TursoSourceIndexOwnerRow {
            file_hash: row.get::<String>(0).map_err(|error| {
                format!("failed to decode Turso source-index owner hash: {error}")
            })?,
            owner_path: row.get::<String>(1).map_err(|error| {
                format!("failed to decode Turso source-index owner path: {error}")
            })?,
            language_id: row.get::<Option<String>>(2).map_err(|error| {
                format!("failed to decode Turso source-index language: {error}")
            })?,
            provider_id: row.get::<Option<String>>(3).map_err(|error| {
                format!("failed to decode Turso source-index provider: {error}")
            })?,
            source_kind: row.get::<String>(4).map_err(|error| {
                format!("failed to decode Turso source-index source kind: {error}")
            })?,
            line_count: row.get::<Option<i64>>(5).map_err(|error| {
                format!("failed to decode Turso source-index line count: {error}")
            })?,
            query_keys_json: row.get::<String>(6).map_err(|error| {
                format!("failed to decode Turso source-index query keys: {error}")
            })?,
            selector_facts_json: row.get::<String>(7).map_err(|error| {
                format!("failed to decode Turso source-index selectors: {error}")
            })?,
            term_tokens_json: row
                .get::<String>(8)
                .map_err(|error| format!("failed to decode Turso source-index terms: {error}"))?,
            selector_count: row.get::<i64>(9).map_err(|error| {
                format!("failed to decode Turso source-index selector count: {error}")
            })?,
        };
        owner_rows.insert(owner.owner_path.clone(), owner);
    }
    Ok((generation_id, owner_rows))
}
use crate::ClientDbSourceIndexImport;

pub(super) struct PreparedTursoSourceIndexRows {
    pub(super) physical_generation_id: String,
    pub(super) selector_fingerprint: String,
    pub(super) changed_owner_paths: std::collections::BTreeSet<String>,
    pub(super) changed_owner_rows: Vec<TursoSourceIndexOwnerRow>,
    pub(super) semantic_term_count: usize,
}

pub(super) async fn prepare_turso_source_index_rows(
    connection: &turso::Connection,
    import: &ClientDbSourceIndexImport,
    project_root: &str,
    imported_membership: &std::collections::HashMap<&str, &str>,
    membership_changed_owner_paths: Vec<String>,
    projection_ready: bool,
    reuse_active_generation: bool,
) -> Result<PreparedTursoSourceIndexRows, String> {
    let selector_fingerprint = turso_source_index_selector_fingerprint(import)?;
    let active_generation = active_turso_source_index_generation(
        connection,
        project_root,
        import.schema_id.as_str(),
        import.schema_version.as_str(),
    )
    .await?;
    let requested_generation_id = import.generation_id.as_str().to_string();
    let physical_generation_id = if reuse_active_generation {
        active_generation
            .as_ref()
            .map(|(generation_id, _)| generation_id.clone())
            .ok_or_else(|| {
                "source-index Merkle overlay requires an active physical generation".to_string()
            })?
    } else {
        requested_generation_id
    };
    let selector_projection_unchanged = projection_ready
        && active_generation
            .as_ref()
            .is_some_and(|(generation_id, fingerprint)| {
                generation_id == &physical_generation_id && fingerprint == &selector_fingerprint
            });
    let use_membership_frontier = reuse_active_generation || selector_projection_unchanged;
    let row_owner_paths = if use_membership_frontier {
        membership_changed_owner_paths
            .iter()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>()
    } else {
        import
            .owners
            .iter()
            .map(|owner| owner.owner_path.as_str())
            .collect::<std::collections::BTreeSet<_>>()
    };
    let selectors_by_owner = turso_source_index_canonical_selectors_by_owner(
        import,
        imported_membership,
        &row_owner_paths,
    )?;

    let (all_owner_rows, semantic_term_count) = {
        let mut written_owner_paths = std::collections::BTreeSet::new();
        let mut rows = Vec::with_capacity(row_owner_paths.len());
        let mut semantic_term_count = 0;
        for owner in &import.owners {
            if !written_owner_paths.insert(owner.owner_path.as_str()) {
                return Err(format!(
                    "failed to write Turso source-index owner: duplicate owner path={}",
                    owner.owner_path.as_str()
                ));
            }
            if !row_owner_paths.contains(owner.owner_path.as_str()) {
                continue;
            }
            let query_keys_json = serde_json::to_string(
                &owner
                    .query_keys
                    .iter()
                    .map(|key| key.as_str())
                    .collect::<Vec<_>>(),
            )
            .map_err(|error| format!("failed to encode Turso source-index owner keys: {error}"))?;
            let file_hash = imported_membership
                .get(owner.owner_path.as_str())
                .expect("source-index membership validated owner path");
            let (selector_facts_json, selector_count, term_tokens) = selectors_by_owner
                .get(owner.owner_path.as_str())
                .expect("source-index canonical selectors validated owner path");
            semantic_term_count += term_tokens.len();
            let term_tokens_json = serde_json::to_string(term_tokens).map_err(|error| {
                format!("failed to encode Turso source-index owner terms: {error}")
            })?;
            rows.push(TursoSourceIndexOwnerRow {
                file_hash: (*file_hash).to_string(),
                owner_path: owner.owner_path.as_str().to_string(),
                language_id: owner
                    .language_id
                    .as_ref()
                    .map(|value| value.as_str().to_string()),
                provider_id: owner
                    .provider_id
                    .as_ref()
                    .map(|value| value.as_str().to_string()),
                source_kind: owner.source_kind.as_str().to_string(),
                line_count: owner.line_count.map(i64::from),
                query_keys_json,
                selector_facts_json: selector_facts_json.clone(),
                term_tokens_json,
                selector_count: *selector_count,
            });
        }
        (rows, semantic_term_count)
    };
    let changed_owner_paths = if use_membership_frontier {
        membership_changed_owner_paths
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>()
    } else if projection_ready
        && active_generation
            .as_ref()
            .is_some_and(|(generation_id, _)| generation_id == &physical_generation_id)
    {
        let (_, previous_owner_rows) = active_turso_source_index_owner_rows(
            connection,
            project_root,
            import.schema_id.as_str(),
            import.schema_version.as_str(),
        )
        .await?;
        all_owner_rows
            .iter()
            .filter(|row| {
                previous_owner_rows
                    .get(row.owner_path.as_str())
                    .is_none_or(|previous| previous != *row)
            })
            .map(|row| row.owner_path.clone())
            .collect::<std::collections::BTreeSet<_>>()
    } else {
        all_owner_rows
            .iter()
            .map(|row| row.owner_path.clone())
            .collect::<std::collections::BTreeSet<_>>()
    };
    let changed_owner_rows = all_owner_rows
        .into_iter()
        .filter(|row| changed_owner_paths.contains(row.owner_path.as_str()))
        .collect();
    Ok(PreparedTursoSourceIndexRows {
        physical_generation_id,
        selector_fingerprint,
        changed_owner_paths,
        changed_owner_rows,
        semantic_term_count,
    })
}
