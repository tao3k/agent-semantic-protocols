use std::path::Path;

use agent_semantic_client_core::{SemanticSchemaId, SemanticSchemaVersion};

use super::generation_snapshot::{
    ClientDbActiveGenerationSourceBlobs, ClientDbSourceIndexGenerationSnapshot,
};

#[derive(Clone, Debug, Eq, PartialEq)]
/// Active generation facts and bytes loaded through one Turso connection.
pub struct ClientDbActiveSourceIndexGeneration {
    pub snapshot: ClientDbSourceIndexGenerationSnapshot,
    pub source_blobs: ClientDbActiveGenerationSourceBlobs,
}

pub async fn active_turso_source_index_generation(
    db_path: &Path,
    project_root: &Path,
    schema_id: &SemanticSchemaId,
    schema_version: &SemanticSchemaVersion,
) -> Result<Option<ClientDbActiveSourceIndexGeneration>, String> {
    if !db_path.exists() {
        return Ok(None);
    }
    let normalized_project_root = crate::types::normalized_project_root(project_root)?;
    let connection = crate::engine::turso::connect_turso_client_db(db_path).await?;
    super::core::ensure_turso_source_index_schema(&connection).await?;
    let Some(snapshot) = super::generation_snapshot::load_turso_source_index_generation_snapshot(
        &connection,
        normalized_project_root.as_str(),
        schema_id.as_str(),
        schema_version.as_str(),
    )
    .await?
    else {
        return Ok(None);
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
    Ok(Some(ClientDbActiveSourceIndexGeneration {
        snapshot,
        source_blobs,
    }))
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
