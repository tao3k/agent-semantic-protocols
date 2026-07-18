use super::prepare::TursoSourceIndexOwnerRow;

const TURSO_SOURCE_INDEX_OWNER_WINDOW_PER_TOKEN: usize = 128;
use crate::engine::turso_statement::{
    execute_turso_operation_with_lock_retry, execute_turso_statement_with_lock_retry,
};

const TURSO_SOURCE_INDEX_OWNER_BATCH_SIZE: usize = 128;

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

pub(super) async fn write_turso_source_index_owner_rows(
    connection: &turso::Connection,
    owner_rows: &[TursoSourceIndexOwnerRow],
    project_root: &str,
    schema_id: &str,
    schema_version: &str,
    generation_id: &str,
) -> Result<(), String> {
    for batch in owner_rows.chunks(TURSO_SOURCE_INDEX_OWNER_BATCH_SIZE) {
        let statement = format!(
            "INSERT INTO asp_source_index_owner_v1 (
                project_root, schema_id, schema_version, generation_id, file_hash, owner_path,
                language_id, provider_id, source_kind, line_count,
                query_keys_json, selector_facts_json, term_tokens_json, selector_count
             ) VALUES {}
             ON CONFLICT(project_root, schema_id, schema_version, generation_id, owner_path)
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
            turso_source_index_values_clause(batch.len(), 14),
        );
        let mut params = Vec::with_capacity(batch.len() * 14);
        for row in batch {
            params.extend([
                turso_source_index_text_value(project_root),
                turso_source_index_text_value(schema_id),
                turso_source_index_text_value(schema_version),
                turso_source_index_text_value(generation_id),
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

pub(super) async fn refresh_turso_source_index_posting_projection(
    connection: &turso::Connection,
    project_root: &str,
    schema_id: &str,
    schema_version: &str,
    generation_id: &str,
    owner_paths: &[String],
    owner_rows: &[super::prepare::TursoSourceIndexOwnerRow],
) -> Result<usize, String> {
    let posting_projection_started = std::time::Instant::now();
    if owner_paths.is_empty() {
        return Ok(0);
    }
    let owner_paths_json = serde_json::to_string(owner_paths)
        .map_err(|error| format!("failed to encode Turso source-index posting owners: {error}"))?;
    execute_turso_statement_with_lock_retry(
        connection,
        "CREATE TEMP TABLE IF NOT EXISTS asp_source_index_changed_membership_v1 (
            owner_path TEXT NOT NULL PRIMARY KEY
        )",
        "failed to create Turso source-index changed membership staging table",
    )
    .await?;
    execute_turso_statement_with_lock_retry(
        connection,
        "DELETE FROM asp_source_index_changed_membership_v1",
        "failed to clear Turso source-index changed membership staging table",
    )
    .await?;
    let mut owners_by_token =
        std::collections::BTreeMap::<String, std::collections::BTreeSet<String>>::new();
    for owner_row in owner_rows {
        let terms = serde_json::from_str::<Vec<String>>(owner_row.term_tokens_json.as_str())
            .map_err(|error| {
                format!(
                    "failed to decode Turso source-index owner posting terms: owner={} error={error}",
                    owner_row.owner_path
                )
            })?;
        for term in terms {
            owners_by_token
                .entry(term.to_lowercase())
                .or_default()
                .insert(owner_row.owner_path.clone());
        }
    }
    let posting_rows = owners_by_token
        .into_iter()
        .flat_map(|(token, owner_paths)| {
            owner_paths
                .into_iter()
                .take(TURSO_SOURCE_INDEX_OWNER_WINDOW_PER_TOKEN)
                .map(move |owner_path| (token.clone(), owner_path))
        })
        .collect::<Vec<_>>();
    let posting_rows_json = serde_json::to_string(&posting_rows)
        .map_err(|error| format!("failed to encode Turso source-index postings: {error}"))?;
    execute_turso_operation_with_lock_retry(
        || async {
            connection
                .execute(
                    "INSERT INTO asp_source_index_changed_membership_v1 (owner_path)
                     SELECT DISTINCT value
                     FROM json_each(?1)",
                    (owner_paths_json.as_str(),),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to stage Turso source-index changed membership",
    )
    .await?;
    execute_turso_operation_with_lock_retry(
        || async {
            connection
                .execute(
                    "DELETE FROM asp_source_index_token_owner_v1
                     WHERE project_root = ?1
                       AND schema_id = ?2
                       AND schema_version = ?3
                       AND generation_id = ?4
                       AND owner_path IN (
                           SELECT owner_path
                           FROM asp_source_index_changed_membership_v1
                       )",
                    (project_root, schema_id, schema_version, generation_id),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to clear Turso source-index postings",
    )
    .await?;
    execute_turso_operation_with_lock_retry(
        || async {
            connection
                .execute(
                    "INSERT INTO asp_source_index_token_owner_v1 (
                        project_root, schema_id, schema_version, generation_id, token, owner_path
                     )
                     SELECT ?1,
                            ?2,
                            ?3,
                            ?4,
                            lower(json_extract(posting.value, '$[0]')),
                            json_extract(posting.value, '$[1]')
                     FROM json_each(?5) AS posting",
                    (
                        project_root,
                        schema_id,
                        schema_version,
                        generation_id,
                        posting_rows_json.as_str(),
                    ),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to refresh relational Turso source-index postings",
    )
    .await?;
    let posting_count = posting_rows.len();
    if std::env::var_os("ASP_SOURCE_INDEX_TRACE").is_some() {
        eprintln!(
            "[source-index-db-trace] stage=posting-relational-written elapsedMs={} inserted={}",
            posting_projection_started.elapsed().as_millis(),
            posting_count,
        );
    }
    Ok(posting_count)
}
