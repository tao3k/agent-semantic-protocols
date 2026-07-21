use crate::ClientDbSourceIndexImport;
use crate::engine::turso_statement::{
    execute_turso_operation, execute_turso_statement, run_turso_operation,
};

pub(super) fn turso_source_index_import_membership(
    import: &ClientDbSourceIndexImport,
) -> Result<std::collections::HashMap<&str, &str>, String> {
    let mut file_hashes_by_path = std::collections::HashMap::new();
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

    let mut membership = std::collections::HashMap::new();
    for owner in &import.owners {
        let owner_path = owner.owner_path.as_str();
        let file_hash = file_hashes_by_path.get(owner_path).ok_or_else(|| {
            format!("source-index owner has no file hash: owner_path={owner_path}")
        })?;
        membership.insert(owner_path, *file_hash);
    }
    Ok(membership)
}

pub(super) async fn stage_turso_source_index_import_membership(
    connection: &turso::Connection,
    file_hashes_json: &str,
) -> Result<(), String> {
    execute_turso_statement(
        connection,
        "CREATE TEMP TABLE IF NOT EXISTS asp_source_index_incoming_membership_v1 (
            owner_path TEXT NOT NULL PRIMARY KEY,
            file_hash TEXT NOT NULL
        )",
        "failed to create Turso source-index incoming membership staging table",
    )
    .await?;
    execute_turso_statement(
        connection,
        "DELETE FROM asp_source_index_incoming_membership_v1",
        "failed to clear Turso source-index incoming membership staging table",
    )
    .await?;
    execute_turso_operation(
        || async {
            connection
                .execute(
                    "INSERT INTO asp_source_index_incoming_membership_v1 (
                        owner_path, file_hash
                     )
                     SELECT json_extract(value, '$.path'),
                            json_extract(value, '$.sha256')
                     FROM json_each(?1)",
                    (file_hashes_json,),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to stage Turso source-index incoming membership",
    )
    .await?;
    Ok(())
}

pub(super) async fn turso_source_index_membership_changes(
    connection: &turso::Connection,
    project_root: &str,
    schema_id: &str,
    schema_version: &str,
    projection_ready: bool,
) -> Result<(Vec<String>, Vec<String>), String> {
    let mut rows = run_turso_operation(
        || async {
            connection
                .query(
                    "SELECT incoming.owner_path, 0 AS removed
                     FROM asp_source_index_incoming_membership_v1 AS incoming
                     LEFT JOIN asp_source_index_owner_v1 AS owner
                      ON owner.project_root = ?1
                      AND owner.schema_id = ?2
                      AND owner.schema_version = ?3
                      AND owner.generation_id = (
                          SELECT generation_id FROM asp_source_index_scope_v1
                          WHERE project_root = ?1 AND schema_id = ?2 AND schema_version = ?3
                      )
                      AND owner.owner_path = incoming.owner_path
                     WHERE ?4 = 0
                        OR owner.owner_path IS NULL
                        OR owner.file_hash <> incoming.file_hash
                     UNION ALL
                     SELECT owner.owner_path, 1 AS removed
                     FROM asp_source_index_owner_v1 AS owner
                     LEFT JOIN asp_source_index_incoming_membership_v1 AS incoming
                       ON incoming.owner_path = owner.owner_path
                     WHERE owner.project_root = ?1
                       AND owner.schema_id = ?2
                       AND owner.schema_version = ?3
                       AND owner.generation_id = (
                           SELECT generation_id FROM asp_source_index_scope_v1
                           WHERE project_root = ?1 AND schema_id = ?2 AND schema_version = ?3
                       )
                       AND incoming.owner_path IS NULL",
                    (
                        project_root,
                        schema_id,
                        schema_version,
                        i64::from(projection_ready),
                    ),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to query Turso source-index membership changes",
    )
    .await?;
    let mut changed_owner_paths = Vec::new();
    let mut removed_owner_paths = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso source-index membership changes: {error}"))?
    {
        let owner_path = row.get::<String>(0).map_err(|error| {
            format!("failed to decode Turso source-index changed owner path: {error}")
        })?;
        let removed = row.get::<i64>(1).map_err(|error| {
            format!("failed to decode Turso source-index membership change kind: {error}")
        })?;
        if removed == 0 {
            changed_owner_paths.push(owner_path);
        } else {
            removed_owner_paths.push(owner_path);
        }
    }
    Ok((changed_owner_paths, removed_owner_paths))
}
