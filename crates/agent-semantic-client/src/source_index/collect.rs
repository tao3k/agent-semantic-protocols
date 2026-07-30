//! Provider-owned source-scope receipt consumers for source-index publication.

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceIndexCollectionScope {
    CompleteGeneration,
    TargetProvider {
        language_id: agent_semantic_client_core::LanguageId,
        provider_id: agent_semantic_client_core::ProviderId,
    },
    TargetProviderId {
        provider_id: agent_semantic_client_core::ProviderId,
    },
}

pub(crate) fn collect_source_index_files(
    _project_root: &std::path::Path,
    _provider_registry: &agent_semantic_client_core::ProviderRegistrySnapshot,
    _scope: &SourceIndexCollectionScope,
) -> Result<Vec<agent_semantic_client_db::ClientDbSourceIndexScopeFile>, String> {
    let mut files = Vec::new();
    for provider in &_provider_registry.providers {
        let provider_is_selected = match _scope {
            SourceIndexCollectionScope::CompleteGeneration => true,
            SourceIndexCollectionScope::TargetProvider {
                language_id,
                provider_id,
            } => language_id == &provider.language_id && provider_id == &provider.provider_id,
            SourceIndexCollectionScope::TargetProviderId { provider_id } => {
                provider_id == &provider.provider_id
            }
        };
        if !provider_is_selected {
            continue;
        }
        let receipt = agent_semantic_client_local_cli::provider_workspace_scope_files(
            _project_root,
            provider,
            provider.language_id.as_str(),
            std::path::Path::new(&provider.binary),
        )
        .map_err(|error| error.to_string())?;
        match receipt {
            agent_semantic_client_local_cli::ProviderWorkspaceScopeFiles::Supported(
                provider_files,
            ) => {
                for provider_file in provider_files {
                    let agent_semantic_client_local_cli::ProviderWorkspaceScopePathFile {
                        path,
                        language_id,
                        provider_id,
                    } = provider_file;
                    files.push(agent_semantic_client_db::ClientDbSourceIndexScopeFile {
                        path,
                        language_id,
                        provider_id,
                        selector_receipts: Vec::new(),
                    });
                }
            }
            agent_semantic_client_local_cli::ProviderWorkspaceScopeFiles::Unsupported => {
                return Err(format!(
                    "provider workspace scope is unsupported: languageId={} providerId={}",
                    provider.language_id, provider.provider_id
                ));
            }
        }
    }
    Ok(files)
}

pub(crate) fn collect_workspace_search_source_index_files(
    project_root: &std::path::Path,
    provider_registry: &agent_semantic_client_core::ProviderRegistrySnapshot,
    scope: &SourceIndexCollectionScope,
) -> Result<Vec<agent_semantic_client_db::ClientDbSourceIndexScopeFile>, String> {
    collect_source_index_files(project_root, provider_registry, scope)
}
