use crate::engine::turso_statement::{
    execute_turso_statement_with_lock_retry, run_turso_operation_with_lock_retry,
};

pub(super) async fn retire_noncanonical_posting_layout(
    connection: &turso::Connection,
) -> Result<(), String> {
    let mut rows = run_turso_operation_with_lock_retry(
        || async {
            connection
                .query(
                    "SELECT sql FROM sqlite_master
                     WHERE type = 'table' AND name = 'asp_source_index_token_owner_v1'",
                    (),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to inspect Turso source-index posting layout",
    )
    .await?;
    let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso source-index posting layout: {error}"))?
    else {
        return Ok(());
    };
    let sql = row
        .get::<String>(0)
        .map_err(|error| format!("failed to decode Turso source-index posting layout: {error}"))?;
    let normalized = sql
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    if normalized.contains(
        "primary key (project_root, schema_id, schema_version, generation_id, token, owner_path)",
    ) {
        return Ok(());
    }
    execute_turso_statement_with_lock_retry(
        connection,
        "DROP TABLE asp_source_index_token_owner_v1",
        "failed to retire noncanonical Turso source-index posting layout",
    )
    .await
}
