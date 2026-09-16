// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_content_identity::ProviderRelationEndpointKindV1;
use agent_semantic_content_identity::provider_projection_relation::{
    ProviderProjectedRelation, ProviderProjectedRelationEndpoint,
};

use crate::ClientDbSourceIndexImport;
use crate::engine::turso_statement::execute_turso_operation;

pub(super) async fn refresh_turso_source_index_relation_projection(
    connection: &turso::Connection,
    import: &ClientDbSourceIndexImport,
    project_root: &str,
    schema_id: &str,
    schema_version: &str,
    generation_id: &str,
    owner_paths: &[String],
) -> Result<(), String> {
    if owner_paths.is_empty() {
        if import.relations.is_empty() {
            return Ok(());
        }
        return Err("source-index relations require changed owner membership".to_string());
    }
    let owner_paths_json = serde_json::to_string(owner_paths)
        .map_err(|error| format!("failed to encode source-index relation owners: {error}"))?;
    execute_turso_operation(
        || async {
            connection
                .execute(
                    "DELETE FROM asp_source_index_relation_v1
                     WHERE project_root = ?1
                       AND schema_id = ?2
                       AND schema_version = ?3
                       AND generation_id = ?4
                       AND owner_path IN (SELECT value FROM json_each(?5))",
                    (
                        project_root,
                        schema_id,
                        schema_version,
                        generation_id,
                        owner_paths_json.as_str(),
                    ),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to clear Turso source-index relation projection",
    )
    .await?;

    let changed_owner_paths = owner_paths
        .iter()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    let mut rows = Vec::with_capacity(import.relations.len());
    for owned in &import.relations {
        let relation = &owned.relation;
        relation.validate()?;
        let owner_path = owned.owner_path.as_str();
        if !changed_owner_paths.contains(owner_path) {
            return Err(format!(
                "source-index relation belongs outside changed owner membership: ownerPath={owner_path}"
            ));
        }
        rows.push((
            owner_path,
            &relation.from.kind,
            &relation.from.id,
            &relation.kind,
            &relation.to.kind,
            &relation.to.id,
        ));
    }
    rows.sort();
    rows.dedup();
    let rows_json = serde_json::to_string(&rows)
        .map_err(|error| format!("failed to encode source-index relation rows: {error}"))?;
    execute_turso_operation(
        || async {
            connection
                .execute(
                    "INSERT INTO asp_source_index_relation_v1 (
                        project_root, schema_id, schema_version, generation_id,
                        owner_path, from_kind, from_id, relation_kind, to_kind, to_id
                     )
                     SELECT ?1, ?2, ?3, ?4,
                            json_extract(row.value, '$[0]'),
                            json_extract(row.value, '$[1]'),
                            json_extract(row.value, '$[2]'),
                            json_extract(row.value, '$[3]'),
                            json_extract(row.value, '$[4]'),
                            json_extract(row.value, '$[5]')
                     FROM json_each(?5) AS row",
                    (
                        project_root,
                        schema_id,
                        schema_version,
                        generation_id,
                        rows_json.as_str(),
                    ),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to write Turso source-index relation projection",
    )
    .await?;
    Ok(())
}

pub(super) async fn load_turso_source_index_relations(
    connection: &turso::Connection,
    project_root: &str,
    schema_id: &str,
    schema_version: &str,
    generation_id: &str,
) -> Result<Vec<super::generation_snapshot::ClientDbSourceIndexGenerationRelation>, String> {
    let mut rows = connection
        .query(
            "SELECT owner_path, from_kind, from_id, relation_kind, to_kind, to_id
             FROM asp_source_index_relation_v1
             WHERE project_root = ?1 AND schema_id = ?2
               AND schema_version = ?3 AND generation_id = ?4
             ORDER BY owner_path, from_kind, from_id, relation_kind, to_kind, to_id",
            (project_root, schema_id, schema_version, generation_id),
        )
        .await
        .map_err(|error| format!("failed to load source-index relations: {error}"))?;
    let mut relations = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read source-index relation: {error}"))?
    {
        relations.push(
            super::generation_snapshot::ClientDbSourceIndexGenerationRelation {
                owner_path: row.get::<String>(0).map_err(|error| {
                    format!("failed to decode source-index relation owner: {error}")
                })?,
                relation: ProviderProjectedRelation {
                    from: ProviderProjectedRelationEndpoint {
                        kind: ProviderRelationEndpointKindV1::try_from(
                            row.get::<String>(1)
                                .map_err(|error| {
                                    format!(
                                        "failed to decode source-index relation from kind: {error}"
                                    )
                                })?
                                .as_str(),
                        )?,
                        id: row.get::<String>(2).map_err(|error| {
                            format!("failed to decode source-index relation from id: {error}")
                        })?,
                    },
                    kind: row
                        .get::<String>(3)
                        .map_err(|error| {
                            format!("failed to decode source-index relation kind: {error}")
                        })?
                        .into(),
                    to: ProviderProjectedRelationEndpoint {
                        kind: ProviderRelationEndpointKindV1::try_from(
                            row.get::<String>(4)
                                .map_err(|error| {
                                    format!(
                                        "failed to decode source-index relation to kind: {error}"
                                    )
                                })?
                                .as_str(),
                        )?,
                        id: row.get::<String>(5).map_err(|error| {
                            format!("failed to decode source-index relation to id: {error}")
                        })?,
                    },
                },
            },
        );
    }
    let mut materialization_rows = connection
        .query(
            "SELECT materialization_json
             FROM asp_workspace_generation_materialization_v1
             WHERE project_root = ?1
               AND source_index_schema_id = ?2
               AND source_index_schema_version = ?3
               AND generation_id = ?4
             LIMIT 1",
            (project_root, schema_id, schema_version, generation_id),
        )
        .await
        .map_err(|error| format!("failed to load relation materialization authority: {error}"))?;
    let materialization_row = materialization_rows
        .next()
        .await
        .map_err(|error| format!("failed to read relation materialization authority: {error}"))?
        .ok_or_else(|| {
            format!(
                "source-index relation generation has no canonical materialization: generationId={generation_id}"
            )
        })?;
    let materialization_json = materialization_row
        .get::<Vec<u8>>(0)
        .map_err(|error| format!("failed to decode relation materialization bytes: {error}"))?;
    let materialization = serde_json::from_slice::<
        crate::runtime_server_workspace::WorkspaceCanonicalMaterialization,
    >(&materialization_json)
    .map_err(|error| format!("failed to decode relation materialization: {error}"))?;
    let mut persisted_relations = relations
        .iter()
        .map(|attributed| crate::ClientDbSourceIndexOwnedRelation {
            owner_path: crate::ClientDbSourceIndexPath::new(attributed.owner_path.clone()),
            relation: attributed.relation.clone(),
        })
        .collect::<Vec<_>>();
    persisted_relations.sort();
    let mut materialized_relations = materialization.relations;
    materialized_relations.sort();
    if persisted_relations != materialized_relations {
        return Err(format!(
            "source-index relation generation is incomplete: generationId={generation_id} persisted={} materialized={}",
            persisted_relations.len(),
            materialized_relations.len()
        ));
    }
    Ok(relations)
}
