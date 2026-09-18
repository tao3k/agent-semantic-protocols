// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::path::Path;

use agent_semantic_client_core::{SemanticSchemaId, SemanticSchemaVersion};

use super::generation_snapshot::{
    ClientDbActiveGenerationSourceBlobs, ClientDbSourceIndexGenerationLoad,
    ClientDbSourceIndexGenerationSnapshot,
};

#[derive(Clone, Debug, Eq, PartialEq)]
/// Active generation facts and bytes loaded through one Turso connection.
pub struct ClientDbActiveSourceIndexGeneration {
    pub snapshot: ClientDbSourceIndexGenerationSnapshot,
    pub source_blobs: ClientDbActiveGenerationSourceBlobs,
}

pub(crate) enum ClientDbActiveSourceIndexGenerationCandidate {
    Missing,
    Ready(Box<ClientDbActiveSourceIndexGeneration>),
    ReuseRejected(String),
}

pub async fn active_turso_source_index_generation(
    db_path: &Path,
    project_root: &Path,
    schema_id: &SemanticSchemaId,
    schema_version: &SemanticSchemaVersion,
) -> Result<Option<ClientDbActiveSourceIndexGeneration>, String> {
    match active_turso_source_index_generation_candidate(
        db_path,
        project_root,
        schema_id,
        schema_version,
    )
    .await?
    {
        ClientDbActiveSourceIndexGenerationCandidate::Missing => Ok(None),
        ClientDbActiveSourceIndexGenerationCandidate::Ready(active) => Ok(Some(*active)),
        ClientDbActiveSourceIndexGenerationCandidate::ReuseRejected(reason) => Err(reason),
    }
}

pub(crate) async fn active_turso_source_index_generation_candidate(
    db_path: &Path,
    project_root: &Path,
    schema_id: &SemanticSchemaId,
    schema_version: &SemanticSchemaVersion,
) -> Result<ClientDbActiveSourceIndexGenerationCandidate, String> {
    if !db_path.exists() {
        return Ok(ClientDbActiveSourceIndexGenerationCandidate::Missing);
    }
    let normalized_project_root = crate::types::normalized_project_root(project_root)?;
    let connection = crate::engine::turso::connect_turso_client_db(db_path).await?;
    super::core::ensure_turso_source_index_schema(&connection).await?;
    let snapshot = match super::generation_snapshot::load_turso_source_index_generation_candidate(
        &connection,
        normalized_project_root.as_str(),
        schema_id.as_str(),
        schema_version.as_str(),
    )
    .await?
    {
        ClientDbSourceIndexGenerationLoad::Missing => {
            return Ok(ClientDbActiveSourceIndexGenerationCandidate::Missing);
        }
        ClientDbSourceIndexGenerationLoad::Ready(snapshot) => *snapshot,
        ClientDbSourceIndexGenerationLoad::ReuseRejected(error) => {
            return Ok(ClientDbActiveSourceIndexGenerationCandidate::ReuseRejected(
                format!("failed to materialize Turso source-index generation: {error}"),
            ));
        }
    };
    let source_blobs =
        super::generation_snapshot::load_turso_source_index_generation_blobs_on_connection(
            &connection,
            normalized_project_root.as_str(),
            schema_id.as_str(),
            schema_version.as_str(),
            &snapshot,
        )
        .await?;
    Ok(ClientDbActiveSourceIndexGenerationCandidate::Ready(
        Box::new(ClientDbActiveSourceIndexGeneration {
            snapshot,
            source_blobs,
        }),
    ))
}

pub(crate) async fn active_turso_workspace_generation_materialization(
    db_path: &Path,
    workspace_identity: &str,
    project_root: &Path,
) -> Result<Option<crate::runtime_server_workspace::WorkspaceCanonicalMaterialization>, String> {
    use crate::runtime_server_workspace::WorkspaceCanonicalMaterializationLoad;

    if !db_path.exists() {
        return Ok(None);
    }
    let normalized_project_root = crate::types::normalized_project_root(project_root)?;
    let connection = crate::engine::turso::connect_turso_client_db(db_path).await?;
    super::core::ensure_turso_source_index_schema(&connection).await?;
    match super::materialization::load_active_workspace_generation_materialization(
        &connection,
        workspace_identity,
        normalized_project_root.as_str(),
    )
    .await?
    {
        WorkspaceCanonicalMaterializationLoad::Ready(materialization) => Ok(Some(materialization)),
        WorkspaceCanonicalMaterializationLoad::Missing => Ok(None),
        WorkspaceCanonicalMaterializationLoad::Incompatible { reason } => Err(reason),
    }
}
