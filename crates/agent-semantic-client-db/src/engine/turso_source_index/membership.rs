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

pub(super) fn validate_source_index_membership_change_set(
    request: &crate::source_index::ClientDbSourceIndexRefreshRequest,
) -> Result<(), String> {
    let crate::source_index::ClientDbSourceIndexMembershipChangeSet::MerkleOverlay {
        changed_owner_paths,
        removed_owner_paths,
    } = &request.membership_change_set
    else {
        return Ok(());
    };
    if request.source_snapshot.base_root_digest.is_none()
        || request.source_snapshot.dirty_paths_digest.is_none()
    {
        return Err(
            "source-index Merkle overlay requires baseRootDigest and dirtyPathsDigest evidence"
                .to_string(),
        );
    }
    if changed_owner_paths.is_empty() && removed_owner_paths.is_empty() {
        return Err("source-index Merkle overlay requires at least one changed owner".to_string());
    }

    let changed = changed_owner_paths
        .iter()
        .map(|path| path.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let removed = removed_owner_paths
        .iter()
        .map(|path| path.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    if changed.len() != changed_owner_paths.len() || removed.len() != removed_owner_paths.len() {
        return Err("source-index Merkle overlay contains duplicate owner paths".to_string());
    }
    if changed.iter().any(|path| removed.contains(path)) {
        return Err(
            "source-index Merkle overlay owner cannot be both changed and removed".to_string(),
        );
    }

    let imported_file_paths = request
        .import
        .file_hashes
        .iter()
        .map(|file| file.path.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let imported_owner_paths = request
        .import
        .owners
        .iter()
        .map(|owner| owner.owner_path.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    if let Some(path) = changed.iter().find(|path| {
        !imported_file_paths.contains(**path) || !imported_owner_paths.contains(**path)
    }) {
        return Err(format!(
            "source-index Merkle overlay changed owner is absent from snapshot import: path={path}"
        ));
    }
    if let Some(path) = removed
        .iter()
        .find(|path| imported_file_paths.contains(**path) || imported_owner_paths.contains(**path))
    {
        return Err(format!(
            "source-index Merkle overlay removed owner remains in snapshot import: path={path}"
        ));
    }
    Ok(())
}

pub(super) async fn validate_turso_source_index_overlay_base(
    connection: &turso::Connection,
    project_root: &str,
    schema_id: &str,
    schema_version: &str,
    expected_base_root_digest: &str,
) -> Result<(), String> {
    let mut rows = run_turso_operation(
        || async {
            connection
                .query(
                    "SELECT source_snapshot_json
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
        "failed to query Turso source-index overlay base snapshot",
    )
    .await?;
    let Some(row) = rows.next().await.map_err(|error| {
        format!("failed to read Turso source-index overlay base snapshot: {error}")
    })?
    else {
        return Err("source-index Merkle overlay base projection is missing".to_string());
    };
    let source_snapshot_json = row.get::<String>(0).map_err(|error| {
        format!("failed to decode Turso source-index overlay base snapshot: {error}")
    })?;
    let source_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence =
        serde_json::from_str(&source_snapshot_json).map_err(|error| {
            format!("failed to parse Turso source-index overlay base snapshot: {error}")
        })?;
    if source_snapshot.root_digest != expected_base_root_digest {
        return Err(format!(
            "source-index Merkle overlay base root mismatch: expected={} actual={}",
            expected_base_root_digest, source_snapshot.root_digest
        ));
    }
    Ok(())
}
