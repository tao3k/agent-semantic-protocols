//! Provider-scope collection facade for source-index refresh.

use std::path::Path;

use agent_semantic_client_core::ProviderRegistrySnapshot;
use agent_semantic_client_local_cli::collect_provider_source_scope_files;

use super::config::SOURCE_INDEX_FILE_LIMIT;
use super::model::SourceIndexScopeFile;

/// Provider selection policy for one source-index collection pass.
#[derive(Clone, Debug)]
pub enum SourceIndexCollectionScopeV1 {
    TargetProvider {
        language_id: agent_semantic_client_core::LanguageId,
        provider_id: agent_semantic_client_core::ProviderId,
    },
    TargetProviderId {
        provider_id: agent_semantic_client_core::ProviderId,
    },
    CompleteGeneration,
}

pub(super) fn collect_source_index_files(
    project_root: &Path,
    snapshot: &ProviderRegistrySnapshot,
    scope: &SourceIndexCollectionScopeV1,
) -> Result<Vec<SourceIndexScopeFile>, String> {
    let provider_files = match scope {
        SourceIndexCollectionScopeV1::TargetProvider {
            language_id,
            provider_id,
        } => agent_semantic_client_local_cli::collect_target_provider_source_scope_files(
            project_root,
            snapshot,
            language_id,
            provider_id,
            SOURCE_INDEX_FILE_LIMIT,
        )?,
        SourceIndexCollectionScopeV1::TargetProviderId { provider_id } => {
            agent_semantic_client_local_cli::collect_target_provider_source_scope_files_by_provider_id(
                project_root,
                snapshot,
                provider_id,
                SOURCE_INDEX_FILE_LIMIT,
            )?
        }
        SourceIndexCollectionScopeV1::CompleteGeneration => {
            collect_provider_source_scope_files(project_root, snapshot, SOURCE_INDEX_FILE_LIMIT)?
        }
    };

    Ok(provider_files
        .into_iter()
        .map(|file| SourceIndexScopeFile {
            path: file.path,
            language_id: file.language_id,
            provider_id: file.provider_id,
            selector_receipts: Vec::new(),
        })
        .collect())
}

/// Collects the provider-owned source files required by one search-index scope.
pub fn collect_workspace_search_source_index_files(
    project_root: &Path,
    snapshot: &ProviderRegistrySnapshot,
    scope: &SourceIndexCollectionScopeV1,
) -> Result<Vec<SourceIndexScopeFile>, String> {
    let files = collect_source_index_files(project_root, snapshot, scope)?;
    let missing_provider_ids = snapshot
        .providers
        .iter()
        .filter(|provider| match scope {
            SourceIndexCollectionScopeV1::TargetProvider {
                language_id,
                provider_id,
            } => &provider.language_id == language_id && &provider.provider_id == provider_id,
            SourceIndexCollectionScopeV1::TargetProviderId { provider_id } => {
                &provider.provider_id == provider_id
            }
            SourceIndexCollectionScopeV1::CompleteGeneration => true,
        })
        .filter(|provider| {
            !files.iter().any(|file| {
                file.provider_id == provider.provider_id
                    && provider_matches_source_extension(provider, &file.path)
            })
        })
        .map(|provider| provider.provider_id.as_str())
        .collect::<Vec<_>>();
    if !missing_provider_ids.is_empty() {
        return Err(format!(
            "provider source envelope is incomplete: missingProviderIds={}",
            missing_provider_ids.join(",")
        ));
    }
    Ok(files)
}

#[cfg(test)]
#[path = "../../tests/unit/source_index_collection_scope.rs"]
mod source_index_collection_scope_tests;

fn provider_matches_source_extension(
    provider: &agent_semantic_client_core::ResolvedProvider,
    path: &Path,
) -> bool {
    let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
        return false;
    };
    provider
        .source_extensions
        .iter()
        .any(|candidate| extension_matches(candidate, extension))
}

fn extension_matches(candidate: &str, extension: &str) -> bool {
    candidate
        .trim_start_matches('.')
        .eq_ignore_ascii_case(extension)
}
