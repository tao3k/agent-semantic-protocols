// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use crate::runtime_server_workspace::WorkspaceCanonicalMaterialization;

pub(super) async fn persist_workspace_generation_materialization(
    connection: &turso::Connection,
    project_root: &str,
    source_index_schema_id: &str,
    source_index_schema_version: &str,
    generation_id: &str,
    materialization: &WorkspaceCanonicalMaterialization,
) -> Result<(), String> {
    if let Some(existing) = load_workspace_generation_materialization(
        connection,
        materialization.workspace_identity.as_str(),
        project_root,
        source_index_schema_id,
        source_index_schema_version,
        generation_id,
    )
    .await?
    {
        if !existing.has_same_generation_identity(materialization) {
            return Err(format!(
                "immutable workspace generation materialization drift: workspaceIdentity={} generationId={generation_id}",
                materialization.workspace_identity
            ));
        }
        return Ok(());
    }

    let materialization_json = serde_json::to_vec(materialization).map_err(|error| {
        format!("failed to encode workspace generation materialization: {error}")
    })?;
    connection
        .execute(
            "INSERT INTO asp_workspace_generation_materialization_v1 (
                workspace_identity,
                project_root,
                materialization_schema_id,
                materialization_schema_version,
                source_index_schema_id,
                source_index_schema_version,
                generation_id,
                source_snapshot_root_digest,
                import_digest,
                materialization_json,
                updated_at_ms
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            (
                materialization.workspace_identity.as_str(),
                project_root,
                materialization.schema_id.as_str(),
                materialization.schema_version.as_str(),
                source_index_schema_id,
                source_index_schema_version,
                generation_id,
                materialization.source_snapshot.root_digest.as_str(),
                materialization.import_digest.as_str(),
                materialization_json,
                unix_time_ms(),
            ),
        )
        .await
        .map_err(|error| {
            format!("failed to persist workspace generation materialization: {error}")
        })?;
    Ok(())
}

pub(super) async fn load_workspace_generation_materialization(
    connection: &turso::Connection,
    workspace_identity: &str,
    project_root: &str,
    source_index_schema_id: &str,
    source_index_schema_version: &str,
    generation_id: &str,
) -> Result<Option<WorkspaceCanonicalMaterialization>, String> {
    let mut rows = connection
        .query(
            "SELECT materialization_json
             FROM asp_workspace_generation_materialization_v1
             WHERE workspace_identity = ?1
               AND project_root = ?2
               AND source_index_schema_id = ?3
               AND source_index_schema_version = ?4
               AND generation_id = ?5
             LIMIT 1",
            (
                workspace_identity,
                project_root,
                source_index_schema_id,
                source_index_schema_version,
                generation_id,
            ),
        )
        .await
        .map_err(|error| {
            format!("failed to query workspace generation materialization: {error}")
        })?;
    let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read workspace generation materialization: {error}"))?
    else {
        return Ok(None);
    };
    let bytes = row.get::<Vec<u8>>(0).map_err(|error| {
        format!("failed to decode workspace generation materialization bytes: {error}")
    })?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|error| format!("failed to decode workspace generation materialization: {error}"))
}

pub(super) async fn load_active_workspace_generation_materialization(
    connection: &turso::Connection,
    workspace_identity: &str,
    project_root: &str,
) -> Result<crate::runtime_server_workspace::WorkspaceCanonicalMaterializationLoad, String> {
    use crate::runtime_server_workspace::WorkspaceCanonicalMaterializationLoad;

    let mut rows = connection
        .query(
            "SELECT materialization.materialization_json, scope.source_snapshot_json
             FROM asp_workspace_generation_materialization_v1 AS materialization
             INNER JOIN asp_source_index_scope_v1 AS scope
               ON scope.project_root = materialization.project_root
              AND scope.schema_id = materialization.source_index_schema_id
              AND scope.schema_version = materialization.source_index_schema_version
              AND scope.generation_id = materialization.generation_id
             WHERE materialization.workspace_identity = ?1
               AND materialization.project_root = ?2
             ORDER BY scope.updated_at_ms DESC, scope.generation_id DESC
             LIMIT 1",
            (workspace_identity, project_root),
        )
        .await
        .map_err(|error| {
            format!("failed to query active workspace generation materialization: {error}")
        })?;
    let Some(row) = rows.next().await.map_err(|error| {
        format!("failed to read active workspace generation materialization: {error}")
    })?
    else {
        return Ok(WorkspaceCanonicalMaterializationLoad::Missing);
    };
    let bytes = row.get::<Vec<u8>>(0).map_err(|error| {
        format!("failed to decode active workspace generation materialization bytes: {error}")
    })?;
    let mut materialization: WorkspaceCanonicalMaterialization =
        match serde_json::from_slice(&bytes) {
            Ok(materialization) => materialization,
            Err(error) => {
                return Ok(WorkspaceCanonicalMaterializationLoad::Incompatible {
                    reason: format!(
                        "failed to decode active workspace generation materialization: {error}"
                    ),
                });
            }
        };
    let requested_project_root =
        WorkspaceCanonicalMaterialization::canonical_project_root(project_root)?;
    let materialized_project_root =
        WorkspaceCanonicalMaterialization::canonical_project_root(&materialization.project_root)?;
    if materialized_project_root != requested_project_root {
        return Err(format!(
            "active workspace generation materialization project root drift: requested={project_root} materialized={}",
            materialization.project_root
        ));
    }
    materialization.project_root = requested_project_root;
    let source_snapshot_json = row.get::<String>(1).map_err(|error| {
        format!("failed to decode active workspace generation source snapshot: {error}")
    })?;
    let source_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence =
        match serde_json::from_str(&source_snapshot_json) {
            Ok(source_snapshot) => source_snapshot,
            Err(error) => {
                return Ok(WorkspaceCanonicalMaterializationLoad::Incompatible {
                    reason: format!(
                        "failed to decode active workspace generation source snapshot: {error}"
                    ),
                });
            }
        };
    if !materialization
        .source_snapshot
        .has_same_content_identity(&source_snapshot)
    {
        return Err(format!(
            "active workspace generation materialization root drift: materialized={:?} durable={source_snapshot:?}",
            materialization.source_snapshot
        ));
    }
    if let Err(reason) = materialization.validate_persisted(workspace_identity) {
        return Ok(WorkspaceCanonicalMaterializationLoad::Incompatible { reason });
    }
    Ok(WorkspaceCanonicalMaterializationLoad::Ready(
        materialization,
    ))
}

fn unix_time_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}
