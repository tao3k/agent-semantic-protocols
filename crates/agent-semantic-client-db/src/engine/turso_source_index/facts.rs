use crate::{
    engine::turso_statement::{
        execute_turso_operation_with_lock_retry, execute_turso_statement_with_lock_retry,
        run_turso_operation_with_lock_retry,
    },
    source_index::ClientDbSourceIndexImport,
};

use super::core::TURSO_SOURCE_INDEX_TERM_PROJECTION_VERSION;

const TURSO_SOURCE_INDEX_COLD_WRITE_BUDGET: std::time::Duration =
    std::time::Duration::from_secs(30);
const TURSO_SOURCE_INDEX_OWNER_BATCH_SIZE: usize = 128;

fn source_index_db_trace(stage: &str, started: std::time::Instant) {
    if std::env::var_os("ASP_SOURCE_INDEX_TRACE").is_some() {
        eprintln!(
            "[source-index-db-trace] stage={stage} elapsedMs={}",
            started.elapsed().as_millis()
        );
    }
}

fn source_index_db_trace_row_counts(
    stage: &str,
    started: std::time::Instant,
    owner_count: usize,
    term_count: usize,
) {
    if std::env::var_os("ASP_SOURCE_INDEX_TRACE").is_some() {
        eprintln!(
            "[source-index-db-trace] stage={stage} elapsedMs={} owners={owner_count} terms={term_count}",
            started.elapsed().as_millis()
        );
    }
}

fn source_index_db_trace_posting_projection(started: std::time::Instant, posting_count: usize) {
    if std::env::var_os("ASP_SOURCE_INDEX_TRACE").is_some() {
        eprintln!(
            "[source-index-db-trace] stage=snapshot-posting-projection-written elapsedMs={} postings={posting_count}",
            started.elapsed().as_millis(),
        );
    }
}

struct TursoSourceIndexOwnerRow {
    file_hash: String,
    owner_path: String,
    language_id: Option<String>,
    provider_id: Option<String>,
    source_kind: String,
    line_count: Option<i64>,
    query_keys_json: String,
    selector_facts_json: String,
    term_tokens_json: String,
    selector_count: i64,
}

fn turso_source_index_values_clause(row_count: usize, column_count: usize) -> String {
    let row = format!("({})", vec!["?"; column_count].join(","));
    std::iter::repeat_n(row, row_count)
        .collect::<Vec<_>>()
        .join(",")
}

fn turso_source_index_text_value(value: &str) -> turso::Value {
    turso::Value::Text(value.to_string())
}

fn turso_source_index_nullable_text_value(value: Option<&str>) -> turso::Value {
    value.map_or(turso::Value::Null, turso_source_index_text_value)
}

async fn write_turso_source_index_owner_rows(
    connection: &turso::Connection,
    owner_rows: &[TursoSourceIndexOwnerRow],
    project_root: &str,
    schema_id: &str,
    schema_version: &str,
) -> Result<(), String> {
    for batch in owner_rows.chunks(TURSO_SOURCE_INDEX_OWNER_BATCH_SIZE) {
        let statement = format!(
            "INSERT INTO asp_source_index_owner_v1 (
                project_root, schema_id, schema_version, file_hash, owner_path,
                language_id, provider_id, source_kind, line_count,
                query_keys_json, selector_facts_json, term_tokens_json, selector_count
             ) VALUES {}
             ON CONFLICT(project_root, schema_id, schema_version, owner_path)
             DO UPDATE SET
                file_hash = excluded.file_hash,
                language_id = excluded.language_id,
                provider_id = excluded.provider_id,
                source_kind = excluded.source_kind,
                line_count = excluded.line_count,
                query_keys_json = excluded.query_keys_json,
                selector_facts_json = excluded.selector_facts_json,
                term_tokens_json = excluded.term_tokens_json,
                selector_count = excluded.selector_count",
            turso_source_index_values_clause(batch.len(), 13),
        );
        let mut params = Vec::with_capacity(batch.len() * 13);
        for row in batch {
            params.extend([
                turso_source_index_text_value(project_root),
                turso_source_index_text_value(schema_id),
                turso_source_index_text_value(schema_version),
                turso_source_index_text_value(&row.file_hash),
                turso_source_index_text_value(&row.owner_path),
                turso_source_index_nullable_text_value(row.language_id.as_deref()),
                turso_source_index_nullable_text_value(row.provider_id.as_deref()),
                turso_source_index_text_value(&row.source_kind),
                row.line_count
                    .map_or(turso::Value::Null, turso::Value::Integer),
                turso_source_index_text_value(&row.query_keys_json),
                turso_source_index_text_value(&row.selector_facts_json),
                turso_source_index_text_value(&row.term_tokens_json),
                turso::Value::Integer(row.selector_count),
            ]);
        }
        execute_turso_operation_with_lock_retry(
            || async {
                connection
                    .execute(statement.as_str(), params.clone())
                    .await
                    .map_err(|error| error.to_string())
            },
            "failed to write Turso source-index owner facts",
        )
        .await?;
    }
    Ok(())
}

async fn refresh_turso_source_index_posting_projection(
    connection: &turso::Connection,
    project_root: &str,
    schema_id: &str,
    schema_version: &str,
    owner_paths: &[String],
) -> Result<usize, String> {
    if owner_paths.is_empty() {
        return Ok(0);
    }
    let owner_paths_json = serde_json::to_string(owner_paths)
        .map_err(|error| format!("failed to encode Turso source-index posting owners: {error}"))?;
    execute_turso_operation_with_lock_retry(
        || async {
            connection
                .execute(
                    "DELETE FROM asp_source_index_token_owner_v1
                     WHERE project_root = ?1
                       AND schema_id = ?2
                       AND schema_version = ?3
                       AND owner_path IN (SELECT value FROM json_each(?4))",
                    (
                        project_root,
                        schema_id,
                        schema_version,
                        owner_paths_json.as_str(),
                    ),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to clear Turso source-index postings",
    )
    .await?;
    let posting_count = execute_turso_operation_with_lock_retry(
        || async {
            connection
                .execute(
                    "INSERT INTO asp_source_index_token_owner_v1 (
                        project_root, schema_id, schema_version, token, owner_path
                     )
                     SELECT DISTINCT owner.project_root,
                            owner.schema_id,
                            owner.schema_version,
                            lower(term.value),
                            owner.owner_path
                     FROM asp_source_index_owner_v1 AS owner,
                          json_each(owner.term_tokens_json) AS term
                     WHERE owner.project_root = ?1
                       AND owner.schema_id = ?2
                       AND owner.schema_version = ?3
                       AND owner.owner_path IN (SELECT value FROM json_each(?4))",
                    (
                        project_root,
                        schema_id,
                        schema_version,
                        owner_paths_json.as_str(),
                    ),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to refresh Turso source-index postings",
    )
    .await?;
    Ok(posting_count as usize)
}

async fn rollback_turso_source_index_transaction(
    connection: &turso::Connection,
    write_error: String,
) -> Result<(), String> {
    match execute_turso_statement_with_lock_retry(
        connection,
        "ROLLBACK",
        "failed to roll back Turso source-index transaction",
    )
    .await
    {
        Ok(()) => Err(write_error),
        Err(rollback_error) => Err(format!("{write_error}; rollbackError={rollback_error}")),
    }
}

async fn return_turso_source_index_write_failure(
    connection: &turso::Connection,
    transaction_started: bool,
    write_error: String,
) -> Result<(), String> {
    if transaction_started {
        rollback_turso_source_index_transaction(connection, write_error).await
    } else {
        Err(write_error)
    }
}

fn unix_time_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}

#[derive(serde::Serialize)]
struct TursoSourceIndexCanonicalSelectorFact {
    selector_id: String,
    symbol: Option<String>,
    kind: Option<String>,
    start_line: u32,
    end_line: u32,
    source: String,
    payload_kind: Option<String>,
    payload_bounded: bool,
    query_keys: Vec<String>,
}

fn turso_source_index_terms(value: &str, terms: &mut std::collections::BTreeSet<String>) {
    for token in value
        .split(|character: char| {
            !character.is_alphanumeric() && character != '_' && character != '-'
        })
        .filter(|token| !token.is_empty())
    {
        let token = token.to_ascii_lowercase();
        terms.insert(token.clone());
        for component in token
            .split(['_', '-'])
            .filter(|component| !component.is_empty())
        {
            terms.insert(component.to_string());
        }
    }
}

pub(super) async fn turso_source_index_projection_ready(
    connection: &turso::Connection,
    project_root: &str,
    schema_id: &str,
    schema_version: &str,
) -> Result<bool, String> {
    let mut rows = run_turso_operation_with_lock_retry(
        || async {
            connection
                .query(
                    "SELECT layout.term_projection_version,
                            layout.token_projection_generation_id,
                            scope.generation_id
                     FROM asp_source_index_layout_v1 AS layout
                     JOIN asp_source_index_scope_v1 AS scope
                       ON scope.project_root = layout.project_root
                      AND scope.schema_id = layout.schema_id
                      AND scope.schema_version = layout.schema_version
                     WHERE layout.project_root = ?1
                       AND layout.schema_id = ?2
                       AND layout.schema_version = ?3
                     LIMIT 1",
                    (project_root, schema_id, schema_version),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to inspect Turso source-index term projection scope",
    )
    .await?;
    let Some(row) = rows.next().await.map_err(|error| {
        format!("failed to read Turso source-index term projection scope: {error}")
    })?
    else {
        return Ok(false);
    };
    let projection_version = row.get::<i64>(0).map_err(|error| {
        format!("failed to decode Turso source-index term projection scope: {error}")
    })?;
    let projection_generation_id = row.get::<String>(1).map_err(|error| {
        format!("failed to decode Turso source-index token projection generation: {error}")
    })?;
    let scope_generation_id = row.get::<String>(2).map_err(|error| {
        format!("failed to decode Turso source-index scope generation: {error}")
    })?;
    if projection_version != TURSO_SOURCE_INDEX_TERM_PROJECTION_VERSION
        || projection_generation_id != scope_generation_id
    {
        return Ok(false);
    }
    let mut token_rows = run_turso_operation_with_lock_retry(
        || async {
            connection
                .query(
                    "SELECT 1
                     FROM asp_source_index_token_owner_v1
                     WHERE project_root = ?1
                       AND schema_id = ?2
                       AND schema_version = ?3
                     LIMIT 1",
                    (project_root, schema_id, schema_version),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to inspect Turso source-index token projection",
    )
    .await?;
    token_rows
        .next()
        .await
        .map(|row| row.is_some())
        .map_err(|error| format!("failed to read Turso source-index token projection: {error}"))
}

fn turso_source_index_canonical_selectors_by_owner(
    import: &ClientDbSourceIndexImport,
    membership: &std::collections::BTreeMap<String, String>,
) -> Result<std::collections::BTreeMap<String, (String, i64, Vec<String>)>, String> {
    let mut selectors_by_owner = std::collections::BTreeMap::<
        String,
        std::collections::BTreeMap<String, TursoSourceIndexCanonicalSelectorFact>,
    >::new();
    for selector in &import.selectors {
        let owner_path = selector.owner_path.as_str();
        if !membership.contains_key(owner_path) {
            return Err(format!(
                "source-index selector has no owner file hash: owner_path={owner_path}"
            ));
        }
        let selector_id = selector.selector_id.as_str().to_string();
        selectors_by_owner
            .entry(owner_path.to_string())
            .or_default()
            .entry(selector_id.clone())
            .or_insert_with(|| TursoSourceIndexCanonicalSelectorFact {
                selector_id,
                symbol: selector.symbol.clone(),
                kind: selector.kind.clone(),
                start_line: selector.start_line,
                end_line: selector.end_line,
                source: selector.source.as_str().to_string(),
                payload_kind: selector
                    .payload_proof
                    .as_ref()
                    .map(|proof| proof.payload_kind.as_str().to_string()),
                payload_bounded: selector
                    .payload_proof
                    .as_ref()
                    .is_some_and(|proof| proof.bounded),
                query_keys: selector
                    .query_keys
                    .iter()
                    .map(|key| key.as_str().to_string())
                    .collect(),
            });
    }

    import
        .owners
        .iter()
        .map(|owner| {
            let selectors = selectors_by_owner
                .remove(owner.owner_path.as_str())
                .unwrap_or_default()
                .into_values()
                .collect::<Vec<_>>();
            let selector_count = selectors.len().min(i64::MAX as usize) as i64;
            let selector_facts_json = serde_json::to_string(&selectors).map_err(|error| {
                format!("failed to encode Turso source-index canonical selectors: {error}")
            })?;
            let mut terms = std::collections::BTreeSet::new();
            for value in [
                owner.owner_path.as_str(),
                owner
                    .language_id
                    .as_ref()
                    .map_or("", |value| value.as_str()),
                owner
                    .provider_id
                    .as_ref()
                    .map_or("", |value| value.as_str()),
                owner.source_kind.as_str(),
            ] {
                turso_source_index_terms(value, &mut terms);
            }
            for query_key in &owner.query_keys {
                turso_source_index_terms(query_key.as_str(), &mut terms);
            }
            for selector in &selectors {
                for value in [
                    selector.selector_id.as_str(),
                    selector.symbol.as_deref().unwrap_or_default(),
                    selector.kind.as_deref().unwrap_or_default(),
                    selector.source.as_str(),
                ] {
                    turso_source_index_terms(value, &mut terms);
                }
                for query_key in &selector.query_keys {
                    turso_source_index_terms(query_key, &mut terms);
                }
            }
            Ok((
                owner.owner_path.as_str().to_string(),
                (
                    selector_facts_json,
                    selector_count,
                    terms.into_iter().collect(),
                ),
            ))
        })
        .collect()
}

fn turso_source_index_import_membership(
    import: &ClientDbSourceIndexImport,
) -> Result<std::collections::BTreeMap<String, String>, String> {
    let mut file_hashes_by_path = std::collections::BTreeMap::new();
    for file_hash in &import.file_hashes {
        if file_hashes_by_path
            .insert(file_hash.path.as_str(), file_hash.sha256.as_str())
            .is_some()
        {
            return Err(format!(
                "source-index import has duplicate file hash path: path={}",
                file_hash.path
            ));
        }
    }

    let mut membership = std::collections::BTreeMap::new();
    for owner in &import.owners {
        let owner_path = owner.owner_path.as_str();
        let file_hash = file_hashes_by_path.get(owner_path).ok_or_else(|| {
            format!("source-index owner has no file hash: owner_path={owner_path}")
        })?;
        membership.insert(owner_path.to_string(), (*file_hash).to_string());
    }
    Ok(membership)
}

fn validate_turso_source_index_selector_payload_proofs(
    import: &ClientDbSourceIndexImport,
) -> Result<(), String> {
    for selector in &import.selectors {
        if let Some(proof) = &selector.payload_proof
            && proof.structural_selector != selector.selector_id
        {
            return Err(format!(
                "source-index selector payload proof selector mismatch: selector_id={} proof={}",
                selector.selector_id, proof.structural_selector
            ));
        }
    }
    Ok(())
}

async fn turso_source_index_precanonical_storage_exists(
    connection: &turso::Connection,
) -> Result<bool, String> {
    let mut rows = run_turso_operation_with_lock_retry(
        || async {
            connection
                .query(
                    "SELECT EXISTS (
                        SELECT 1
                        FROM sqlite_master
                        WHERE type = 'table'
                          AND name IN (
                              'asp_source_index_generation',
                              'asp_source_index_owner',
                              'asp_source_index_selector',
                              'asp_source_index_owner_file_fact',
                              'asp_source_index_selector_file_fact',
                              'asp_source_index_scoped_owner_file_fact',
                              'asp_source_index_scoped_selector_file_fact',
                              'asp_source_index_active_file_membership',
                              'asp_source_index_active_generation',
                              'asp_source_index_active_file_membership_v1',
                              'asp_source_index_active_owner_file_fact_v1',
                              'asp_source_index_active_selector_file_fact_v1',
                              'asp_source_index_active_fact_scope_v1',
                              'asp_source_index_active_packed_fact_scope_v1'
                          )
                     )",
                    (),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to inspect pre-canonical Turso source-index storage",
    )
    .await?;
    let Some(row) = rows.next().await.map_err(|error| {
        format!("failed to read pre-canonical Turso source-index storage: {error}")
    })?
    else {
        return Ok(false);
    };
    row.get::<i64>(0).map(|value| value != 0).map_err(|error| {
        format!("failed to decode pre-canonical Turso source-index storage: {error}")
    })
}

async fn retire_turso_source_index_precanonical_tables(
    connection: &turso::Connection,
) -> Result<(), String> {
    for table in [
        "asp_source_index_generation",
        "asp_source_index_owner",
        "asp_source_index_selector",
        "asp_source_index_owner_file_fact",
        "asp_source_index_selector_file_fact",
        "asp_source_index_scoped_owner_file_fact",
        "asp_source_index_scoped_selector_file_fact",
        "asp_source_index_active_file_membership",
        "asp_source_index_active_generation",
        "asp_source_index_active_file_membership_v1",
        "asp_source_index_active_owner_file_fact_v1",
        "asp_source_index_active_selector_file_fact_v1",
        "asp_source_index_active_fact_scope_v1",
        "asp_source_index_active_packed_fact_scope_v1",
    ] {
        execute_turso_statement_with_lock_retry(
            connection,
            format!("DROP TABLE IF EXISTS {table}").as_str(),
            "failed to retire pre-canonical Turso source-index storage",
        )
        .await?;
    }
    Ok(())
}

async fn turso_source_index_snapshot_membership(
    connection: &turso::Connection,
    project_root: &str,
    schema_id: &str,
    schema_version: &str,
) -> Result<std::collections::BTreeMap<String, String>, String> {
    let mut rows = run_turso_operation_with_lock_retry(
        || async {
            connection
                .query(
                    "SELECT owner_path, file_hash
                     FROM asp_source_index_owner_v1
                     WHERE project_root = ?1
                       AND schema_id = ?2
                       AND schema_version = ?3",
                    (project_root, schema_id, schema_version),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to query Turso source-index snapshot membership",
    )
    .await?;
    let mut membership = std::collections::BTreeMap::new();
    while let Some(row) = rows.next().await.map_err(|error| {
        format!("failed to read Turso source-index snapshot membership: {error}")
    })? {
        let owner_path = row.get::<String>(0).map_err(|error| {
            format!("failed to decode Turso source-index snapshot owner path: {error}")
        })?;
        let file_hash = row.get::<String>(1).map_err(|error| {
            format!("failed to decode Turso source-index snapshot file hash: {error}")
        })?;
        if membership.insert(owner_path.clone(), file_hash).is_some() {
            return Err(format!(
                "Turso source-index snapshot has duplicate owner path: owner_path={owner_path}"
            ));
        }
    }
    Ok(membership)
}

pub(super) async fn write_turso_source_index_rows(
    connection: &turso::Connection,
    import: &ClientDbSourceIndexImport,
    project_root: &str,
    file_hashes_json: &str,
) -> Result<(), String> {
    let cold_write_started = std::time::Instant::now();
    let transaction_started = std::sync::atomic::AtomicBool::new(false);
    validate_turso_source_index_selector_payload_proofs(import)?;
    let imported_membership = turso_source_index_import_membership(import)?;
    let selectors_by_owner =
        turso_source_index_canonical_selectors_by_owner(import, &imported_membership)?;
    let retire_precanonical_storage =
        turso_source_index_precanonical_storage_exists(connection).await?;
    let write_result = tokio::time::timeout(TURSO_SOURCE_INDEX_COLD_WRITE_BUDGET, async {
        execute_turso_statement_with_lock_retry(
            connection,
            "BEGIN IMMEDIATE",
            "failed to begin Turso source-index transaction",
        )
        .await?;
        transaction_started.store(true, std::sync::atomic::Ordering::Release);

        let snapshot_membership = turso_source_index_snapshot_membership(
            connection,
            project_root,
            import.schema_id.as_str(),
            import.schema_version.as_str(),
        )
        .await?;
        let projection_ready = turso_source_index_projection_ready(
            connection,
            project_root,
            import.schema_id.as_str(),
            import.schema_version.as_str(),
        )
        .await?;
        source_index_db_trace("snapshot-membership-loaded", cold_write_started);
        let changed_owner_paths = imported_membership
            .iter()
            .filter_map(|(owner_path, file_hash)| {
                (!projection_ready || snapshot_membership.get(owner_path) != Some(file_hash))
                    .then_some(owner_path.as_str())
            })
            .collect::<std::collections::BTreeSet<_>>();
        let removed_owner_paths = snapshot_membership
            .keys()
            .filter(|owner_path| !imported_membership.contains_key(owner_path.as_str()))
            .map(String::as_str)
            .collect::<Vec<_>>();

        let (changed_owner_rows, semantic_term_count) = {
            let mut written_owner_paths = std::collections::BTreeSet::new();
            let mut rows = Vec::with_capacity(changed_owner_paths.len());
            let mut semantic_term_count = 0;
            for owner in &import.owners {
                if !changed_owner_paths.contains(owner.owner_path.as_str()) {
                    continue;
                }
                if !written_owner_paths.insert(owner.owner_path.as_str()) {
                    return Err(format!(
                        "failed to write Turso source-index owner: duplicate owner path={}",
                        owner.owner_path.as_str()
                    ));
                }
                let query_keys_json = serde_json::to_string(
                    &owner
                        .query_keys
                        .iter()
                        .map(|key| key.as_str())
                        .collect::<Vec<_>>(),
                )
                .map_err(|error| {
                    format!("failed to encode Turso source-index owner keys: {error}")
                })?;
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
                    file_hash: file_hash.as_str().to_string(),
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
        source_index_db_trace_row_counts(
            "snapshot-rows-built",
            cold_write_started,
            changed_owner_rows.len(),
            semantic_term_count,
        );
        write_turso_source_index_owner_rows(
            connection,
            &changed_owner_rows,
            project_root,
            import.schema_id.as_str(),
            import.schema_version.as_str(),
        )
        .await?;
        source_index_db_trace("snapshot-owner-rows-written", cold_write_started);
        let removed_owner_paths_json =
            serde_json::to_string(&removed_owner_paths.iter().copied().collect::<Vec<_>>())
                .map_err(|error| {
                    format!("failed to encode Turso source-index removed owners: {error}")
                })?;
        execute_turso_operation_with_lock_retry(
            || async {
                connection
                    .execute(
                        "DELETE FROM asp_source_index_owner_v1
                         WHERE project_root = ?1
                           AND schema_id = ?2
                           AND schema_version = ?3
                           AND owner_path IN (SELECT value FROM json_each(?4))",
                        (
                            project_root,
                            import.schema_id.as_str(),
                            import.schema_version.as_str(),
                            removed_owner_paths_json.as_str(),
                        ),
                    )
                    .await
                    .map_err(|error| error.to_string())
            },
            "failed to delete Turso source-index snapshot owners",
        )
        .await?;
        source_index_db_trace("snapshot-owners-pruned", cold_write_started);
        let mut projection_owner_paths = changed_owner_paths
            .iter()
            .map(|owner_path| (*owner_path).to_string())
            .collect::<Vec<_>>();
        projection_owner_paths.extend(
            removed_owner_paths
                .iter()
                .map(|owner_path| (*owner_path).to_string()),
        );
        projection_owner_paths.sort();
        projection_owner_paths.dedup();
        let posting_count = refresh_turso_source_index_posting_projection(
            connection,
            project_root,
            import.schema_id.as_str(),
            import.schema_version.as_str(),
            &projection_owner_paths,
        )
        .await?;
        source_index_db_trace_posting_projection(cold_write_started, posting_count);
        execute_turso_statement_with_lock_retry(
            connection,
            "DROP TABLE IF EXISTS asp_source_index_term_v1",
            "failed to retire Turso source-index term projection",
        )
        .await?;
        execute_turso_statement_with_lock_retry(
            connection,
            "DROP TABLE IF EXISTS asp_source_index_token_v1",
            "failed to retire Turso source-index JSON token dictionary",
        )
        .await?;
        source_index_db_trace("legacy-term-projection-retired", cold_write_started);

        execute_turso_operation_with_lock_retry(
            || async {
                connection
                    .execute(
                        "INSERT INTO asp_source_index_scope_v1 (
                            project_root,
                            schema_id,
                            schema_version,
                            generation_id,
                            file_hashes_json,
                            owner_count,
                            selector_count,
                            updated_at_ms
                        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                        ON CONFLICT(project_root, schema_id, schema_version) DO UPDATE SET
                            generation_id = excluded.generation_id,
                            file_hashes_json = excluded.file_hashes_json,
                            owner_count = excluded.owner_count,
                            selector_count = excluded.selector_count,
                            updated_at_ms = excluded.updated_at_ms",
                        (
                            project_root,
                            import.schema_id.as_str(),
                            import.schema_version.as_str(),
                            import.generation_id.as_str(),
                            file_hashes_json,
                            import.owners.len().min(i64::MAX as usize) as i64,
                            import.selectors.len().min(i64::MAX as usize) as i64,
                            unix_time_ms(),
                        ),
                    )
                    .await
                    .map_err(|error| error.to_string())
            },
            "failed to publish Turso source-index snapshot scope",
        )
        .await?;
        execute_turso_operation_with_lock_retry(
            || async {
                connection
                    .execute(
                        "INSERT INTO asp_source_index_layout_v1 (
                            project_root,
                            schema_id,
                            schema_version,
                            term_projection_version,
                            token_projection_generation_id
                        ) VALUES (?1, ?2, ?3, ?4, ?5)
                        ON CONFLICT(project_root, schema_id, schema_version) DO UPDATE SET
                            term_projection_version = excluded.term_projection_version,
                            token_projection_generation_id = excluded.token_projection_generation_id",
                        (
                            project_root,
                            import.schema_id.as_str(),
                            import.schema_version.as_str(),
                            TURSO_SOURCE_INDEX_TERM_PROJECTION_VERSION,
                            import.generation_id.as_str(),
                        ),
                    )
                    .await
                    .map_err(|error| error.to_string())
            },
            "failed to publish Turso source-index term projection layout",
        )
        .await?;
        if retire_precanonical_storage {
            retire_turso_source_index_precanonical_tables(connection).await?;
        }
        source_index_db_trace("snapshot-scope-published", cold_write_started);
        Ok(())
    })
    .await;

    match write_result {
        Ok(Ok(())) => {
            let commit_result = execute_turso_statement_with_lock_retry(
                connection,
                "COMMIT",
                "failed to commit Turso source-index transaction",
            )
            .await;
            if let Err(error) = commit_result {
                return rollback_turso_source_index_transaction(connection, error).await;
            }
            source_index_db_trace("transaction-committed", cold_write_started);
            let mut checkpoint_rows = run_turso_operation_with_lock_retry(
                || async {
                    connection
                        .query("PRAGMA wal_checkpoint(TRUNCATE)", ())
                        .await
                        .map_err(|error| error.to_string())
                },
                "failed to checkpoint committed Turso source-index snapshot",
            )
            .await?;
            while checkpoint_rows
                .next()
                .await
                .map_err(|error| format!("failed to read Turso source-index checkpoint: {error}"))?
                .is_some()
            {}
            source_index_db_trace("wal-checkpoint-completed", cold_write_started);
            Ok(())
        }
        Ok(Err(error)) => {
            return_turso_source_index_write_failure(
                connection,
                transaction_started.load(std::sync::atomic::Ordering::Acquire),
                error,
            )
            .await
        }
        Err(_) => {
            return_turso_source_index_write_failure(
                connection,
                transaction_started.load(std::sync::atomic::Ordering::Acquire),
                format!(
                    "source-index cold-write budget exhausted: budgetMs={} elapsedMs={} owners={} selectors={}",
                    TURSO_SOURCE_INDEX_COLD_WRITE_BUDGET.as_millis(),
                    cold_write_started.elapsed().as_millis(),
                    import.owners.len(),
                    import.selectors.len(),
                ),
            )
            .await
        }
    }
}
