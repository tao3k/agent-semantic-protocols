//! Provider-scope collection facade for source-index refresh.

use std::path::Path;

use agent_semantic_client_core::ProviderRegistrySnapshot;
use agent_semantic_client_local_cli::collect_provider_source_scope_files;

use super::config::SOURCE_INDEX_FILE_LIMIT;
use super::model::SourceIndexScopeFile;

pub(super) fn collect_source_index_files(
    project_root: &Path,
    snapshot: &ProviderRegistrySnapshot,
) -> Result<Vec<SourceIndexScopeFile>, String> {
    Ok(
        collect_provider_source_scope_files(project_root, snapshot, SOURCE_INDEX_FILE_LIMIT)?
            .into_iter()
            .map(|file| SourceIndexScopeFile {
                path: file.path,
                language_id: file.language_id,
                provider_id: file.provider_id,
                selector_receipts: Vec::new(),
            })
            .collect(),
    )
}
