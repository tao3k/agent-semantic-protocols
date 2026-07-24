//! Shared lifecycle import for query-free language-harness projections.

use std::path::Path;

use agent_semantic_client_core::{SemanticSchemaId, SemanticSchemaVersion};
use agent_semantic_client_db::{
    CLIENT_DB_SOURCE_INDEX_SCHEMA_ID, CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION, ClientDbEngine,
    ClientDbLanguageProjection, ClientDbLanguageProjectionImportRequest,
    source_index_import_from_language_projection,
};

/// Result of validating and importing one parser-owned language projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LanguageProjectionImportReport {
    pub reused: bool,
    pub node_locator_count: usize,
}

/// Import one query-free projection through the shared source-index lifecycle.
pub fn import_language_projection(
    project_root: &Path,
    projection: ClientDbLanguageProjection,
) -> Result<LanguageProjectionImportReport, String> {
    projection.validate()?;
    let db_engine = ClientDbEngine::resolve(project_root)?;
    let client_dir = db_engine.client_dir().to_path_buf();
    let db_session = ClientDbEngine::open_write_session_client_dir(&client_dir)?;
    let schema_id = SemanticSchemaId::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_ID);
    let schema_version = SemanticSchemaVersion::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION);
    let registry_fingerprint = language_projection_registry_fingerprint(&projection);
    let source_blobs = agent_semantic_client_db::ClientDbSourceIndexSourceBlobs::from_normalized(
        projection
            .sources()
            .iter()
            .map(|source| {
                let path =
                    agent_semantic_client_db::ClientDbSourceIndexPath::new(source.path.clone());
                let bytes = std::fs::read(project_root.join(&source.path)).map_err(|error| {
                    format!(
                        "failed to capture language projection source {}: {error}",
                        source.path
                    )
                })?;
                Ok((path, bytes))
            })
            .collect::<Result<Vec<_>, String>>()?,
    );
    let prepared =
        source_index_import_from_language_projection(ClientDbLanguageProjectionImportRequest {
            project_root: project_root.to_path_buf(),
            source_blobs,
            registry_fingerprint: registry_fingerprint.clone(),
            projection: projection.clone(),
        })?;
    let import = prepared.source_index;
    let source_snapshot = prepared.source_snapshot;
    let membership_change_set = prepared.membership_change_set;
    if db_session
        .reusable_source_index_generation(
            project_root,
            &schema_id,
            &schema_version,
            &import.file_hashes,
        )?
        .is_some()
    {
        return Ok(LanguageProjectionImportReport {
            reused: true,
            node_locator_count: 0,
        });
    }
    let report = ClientDbEngine::persist_language_projection_read_model_from_client_dir(
        client_dir,
        &import,
        &projection,
        &source_snapshot,
        &membership_change_set,
    )?;
    Ok(LanguageProjectionImportReport {
        reused: false,
        node_locator_count: report.node_locator_count,
    })
}

fn language_projection_registry_fingerprint(projection: &ClientDbLanguageProjection) -> String {
    format!(
        "language-projection:{}:{}:{}:{}",
        projection.language_id(),
        projection.harness().harness_id(),
        projection.harness().parser_abi(),
        projection.harness().selector_dialect(),
    )
}
