use std::path::Path;

use agent_semantic_content_identity::{
    DerivedArtifactAuthorityState, DerivedSourceArtifactEvidence, DerivedSourceArtifactKind,
    SourceSnapshotEvidence,
};

use super::TursoClientDbGraphOwnerReadModel;
use super::artifact::graph_artifact_digest_for_snapshot;
use super::entity_read::decode_turso_graph_entity_row;
use crate::engine::turso::{connect_turso_client_db_read_only, turso_client_db_exists};
use crate::engine::turso_statement::run_turso_operation_with_lock_retry;

fn empty_graph_owner_read_model(
    authority_state: DerivedArtifactAuthorityState,
    expected_artifact_digest: &str,
    source_snapshot: &SourceSnapshotEvidence,
) -> TursoClientDbGraphOwnerReadModel {
    let artifact_evidence = match authority_state {
        DerivedArtifactAuthorityState::Missing => DerivedSourceArtifactEvidence::missing(
            DerivedSourceArtifactKind::EvidenceGraph,
            expected_artifact_digest,
            source_snapshot.clone(),
        ),
        DerivedArtifactAuthorityState::Stale => DerivedSourceArtifactEvidence::stale(
            DerivedSourceArtifactKind::EvidenceGraph,
            expected_artifact_digest,
            source_snapshot.clone(),
        ),
        DerivedArtifactAuthorityState::Current => {
            unreachable!("an empty graph read model cannot be current")
        }
    };
    TursoClientDbGraphOwnerReadModel {
        artifact_evidence,
        owner_present: false,
        selector_nodes: Vec::new(),
    }
}

/// Read owner identity and parser-owned selector nodes from one pinned graph artifact.
pub async fn lookup_turso_graph_owner_read_model(
    db_path: &Path,
    source_snapshot: &SourceSnapshotEvidence,
    owner_path: &str,
    language_id: Option<&str>,
    limit: u32,
) -> Result<TursoClientDbGraphOwnerReadModel, String> {
    if limit == 0 {
        return Err("graph owner lookup limit must be greater than zero".to_owned());
    }
    if owner_path.trim().is_empty() {
        return Err("graph owner lookup path must not be empty".to_owned());
    }
    let graph_artifact_digest = graph_artifact_digest_for_snapshot(source_snapshot);
    if !turso_client_db_exists(db_path) {
        return Ok(empty_graph_owner_read_model(
            DerivedArtifactAuthorityState::Missing,
            &graph_artifact_digest,
            source_snapshot,
        ));
    }
    let connection = match connect_turso_client_db_read_only(db_path).await {
        Ok(connection) => connection,
        Err(error) if error.to_ascii_lowercase().contains("entity not found") => {
            return Ok(empty_graph_owner_read_model(
                DerivedArtifactAuthorityState::Missing,
                &graph_artifact_digest,
                source_snapshot,
            ));
        }
        Err(error) => return Err(error),
    };
    let mut artifact_rows = match run_turso_operation_with_lock_retry(
        || async {
            connection
                .query(
                    "SELECT source_snapshot_json, snapshot_root, provider_digest
                     FROM asp_graph_artifact
                     WHERE graph_artifact_digest = ?1
                     LIMIT 1",
                    [graph_artifact_digest.as_str()],
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to query Turso graph artifact authority",
    )
    .await
    {
        Ok(rows) => rows,
        Err(error) if error.to_ascii_lowercase().contains("no such table") => {
            return Ok(empty_graph_owner_read_model(
                DerivedArtifactAuthorityState::Missing,
                &graph_artifact_digest,
                source_snapshot,
            ));
        }
        Err(error) => return Err(error),
    };
    let Some(artifact_row) = artifact_rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso graph artifact row: {error}"))?
    else {
        let mut any_artifact_rows = run_turso_operation_with_lock_retry(
            || async {
                connection
                    .query(
                        "SELECT graph_artifact_digest FROM asp_graph_artifact LIMIT 1",
                        (),
                    )
                    .await
                    .map_err(|error| error.to_string())
            },
            "failed to inspect Turso graph artifact authority",
        )
        .await?;
        let state = if any_artifact_rows
            .next()
            .await
            .map_err(|error| format!("failed to read Turso graph artifact authority: {error}"))?
            .is_some()
        {
            DerivedArtifactAuthorityState::Stale
        } else {
            DerivedArtifactAuthorityState::Missing
        };
        return Ok(empty_graph_owner_read_model(
            state,
            &graph_artifact_digest,
            source_snapshot,
        ));
    };
    let source_snapshot_json = artifact_row
        .get::<String>(0)
        .map_err(|error| format!("failed to read Turso graph source snapshot: {error}"))?;
    let persisted_snapshot = serde_json::from_str::<SourceSnapshotEvidence>(&source_snapshot_json)
        .map_err(|error| format!("failed to decode Turso graph source snapshot: {error}"))?;
    let persisted_snapshot_root = artifact_row
        .get::<String>(1)
        .map_err(|error| format!("failed to read Turso graph snapshot root: {error}"))?;
    let persisted_provider_digest = artifact_row
        .get::<String>(2)
        .map_err(|error| format!("failed to read Turso graph provider digest: {error}"))?;
    if persisted_snapshot != *source_snapshot
        || persisted_snapshot_root != source_snapshot.root_digest
        || persisted_provider_digest != source_snapshot.provider_digest
    {
        return Ok(empty_graph_owner_read_model(
            DerivedArtifactAuthorityState::Stale,
            &graph_artifact_digest,
            source_snapshot,
        ));
    }
    let mut rows = run_turso_operation_with_lock_retry(
        || async {
            connection
                .query(
                    "SELECT id, kind, semantic_kind, label, selector, path, language_id, provider_id, query_keys_json
                     FROM asp_graph_artifact_entity
                     WHERE graph_artifact_digest = ?1
                       AND path = ?2
                       AND (?3 IS NULL OR language_id = ?3)
                     ORDER BY CASE WHEN kind = 'selector' THEN 1 ELSE 0 END, id
                     LIMIT ?4",
                    (
                        graph_artifact_digest.as_str(),
                        owner_path,
                        language_id,
                        limit,
                    ),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to query Turso graph owner selectors",
    )
    .await?;
    let mut owner_present = false;
    let mut selector_nodes = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso graph owner selector row: {error}"))?
    {
        let entity = decode_turso_graph_entity_row(&row)?;
        if entity.kind == "selector" {
            selector_nodes.push(entity);
        } else {
            owner_present = true;
        }
    }
    Ok(TursoClientDbGraphOwnerReadModel {
        artifact_evidence: DerivedSourceArtifactEvidence::current(
            DerivedSourceArtifactKind::EvidenceGraph,
            graph_artifact_digest,
            persisted_snapshot,
        ),
        owner_present,
        selector_nodes,
    })
}
