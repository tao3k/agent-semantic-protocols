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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProviderScopeCollectionRoute {
    ProjectResolver,
    GitDocumentCandidates,
}

fn provider_scope_collection_route(
    authority: agent_semantic_client_core::ProviderScopeAuthority,
) -> ProviderScopeCollectionRoute {
    match authority {
        agent_semantic_client_core::ProviderScopeAuthority::ProjectResolution => {
            ProviderScopeCollectionRoute::ProjectResolver
        }
        agent_semantic_client_core::ProviderScopeAuthority::DocumentResolution => {
            ProviderScopeCollectionRoute::GitDocumentCandidates
        }
    }
}

pub(crate) fn collect_source_index_files(
    project_root: &std::path::Path,
    provider_registry: &agent_semantic_client_core::ProviderRegistrySnapshot,
    scope: &SourceIndexCollectionScope,
) -> Result<Vec<agent_semantic_client_db::ClientDbSourceIndexScopeFile>, String> {
    let mut files = Vec::new();
    let mut repository_candidates = None;
    for provider in &provider_registry.providers {
        let provider_is_selected = match scope {
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
        let receipt = match provider_scope_collection_route(provider.scope_authority) {
            ProviderScopeCollectionRoute::ProjectResolver => {
                agent_semantic_client_local_cli::provider_project_scope_files(
                    project_root,
                    provider,
                    provider.language_id.as_str(),
                    std::path::Path::new(&provider.binary),
                )
                .map_err(|error| error.to_string())?
            }
            ProviderScopeCollectionRoute::GitDocumentCandidates => {
                let candidates = match repository_candidates.as_ref() {
                    Some(candidates) => candidates,
                    None => {
                        repository_candidates =
                            agent_semantic_runtime::git::discover_repository_candidate_snapshot(
                                project_root,
                            )
                            .map_err(|error| {
                                format!("discover document repository candidates: {error}")
                            })?;
                        repository_candidates.as_ref().ok_or_else(|| {
                            format!(
                                "document source-index generation requires a Git candidate snapshot: workspace={}",
                                project_root.display()
                            )
                        })?
                    }
                };
                document_scope_files(project_root, provider, candidates)
            }
        };
        append_provider_scope_files(&mut files, provider, receipt)?;
    }
    Ok(files)
}

pub(crate) async fn collect_source_index_files_async(
    project_root: &std::path::Path,
    provider_registry: &agent_semantic_client_core::ProviderRegistrySnapshot,
    scope: &SourceIndexCollectionScope,
) -> Result<Vec<agent_semantic_client_db::ClientDbSourceIndexScopeFile>, String> {
    let project_root_owned = project_root.to_path_buf();
    let repository_candidates = tokio::task::spawn_blocking(move || {
        agent_semantic_runtime::git::discover_repository_candidate_snapshot(&project_root_owned)
    })
    .await
    .map_err(|error| format!("workspace Git candidate task failed: {error}"))?
    .map_err(|error| format!("discover workspace repository candidates: {error}"))?
    .ok_or_else(|| {
        format!(
            "source-index generation requires a Git candidate snapshot: workspace={}",
            project_root.display()
        )
    })?;
    let mut providers = tokio::task::JoinSet::new();
    for provider in &provider_registry.providers {
        let selected = match scope {
            SourceIndexCollectionScope::CompleteGeneration => true,
            SourceIndexCollectionScope::TargetProvider {
                language_id,
                provider_id,
            } => language_id == &provider.language_id && provider_id == &provider.provider_id,
            SourceIndexCollectionScope::TargetProviderId { provider_id } => {
                provider_id == &provider.provider_id
            }
        };
        if !selected
            || !agent_semantic_hook::registered_provider_matches_candidate_paths(
                provider.language_id.as_str(),
                provider.provider_id.as_str(),
                repository_candidates
                    .candidates
                    .iter()
                    .map(|candidate| candidate.path.as_path()),
            )?
        {
            continue;
        }
        let project_root = project_root.to_path_buf();
        let provider = provider.clone();
        let repository_candidates = repository_candidates.clone();
        providers.spawn(async move {
            let receipt = match provider_scope_collection_route(provider.scope_authority) {
                ProviderScopeCollectionRoute::ProjectResolver => {
                    let package_root_path = std::path::PathBuf::from(&provider.binary);
                    agent_semantic_client_local_cli::provider_project_scope_files_with_candidates_async(
                        &project_root,
                        &provider,
                        &package_root_path,
                        repository_candidates,
                    )
                    .await
                    .map_err(|error| error.to_string())?
                }
                ProviderScopeCollectionRoute::GitDocumentCandidates => {
                    document_scope_files(&project_root, &provider, &repository_candidates)
                }
            };
            Ok::<_, String>((provider, receipt))
        });
    }
    let mut files = Vec::new();
    while let Some(result) = providers.join_next().await {
        let (provider, receipt) =
            result.map_err(|error| format!("provider scope task failed: {error}"))??;
        append_provider_scope_files(&mut files, &provider, receipt)?;
    }
    files.sort_by(|left, right| {
        (&left.path, &left.language_id, &left.provider_id).cmp(&(
            &right.path,
            &right.language_id,
            &right.provider_id,
        ))
    });
    files.dedup_by(|left, right| {
        left.path == right.path
            && left.language_id == right.language_id
            && left.provider_id == right.provider_id
    });
    Ok(files)
}

fn document_scope_files(
    project_root: &std::path::Path,
    provider: &agent_semantic_client_core::ResolvedProvider,
    repository_candidates: &agent_semantic_runtime::git::RepositoryCandidateSnapshot,
) -> agent_semantic_client_local_cli::ProviderProjectScopeFiles {
    let files = repository_candidates
        .candidates
        .iter()
        .filter(|candidate| {
            provider.source_extensions.iter().any(|extension| {
                candidate
                    .path
                    .to_string_lossy()
                    .ends_with(extension.as_str())
            })
        })
        .filter_map(|candidate| {
            let candidate_path = candidate.path.to_str()?;
            let path =
                agent_semantic_client_core::scoped_child_path(project_root, candidate_path)?;
            path.is_file().then(|| {
                agent_semantic_client_local_cli::ProviderProjectScopePathFile {
                    path,
                    language_id: provider.language_id.clone(),
                    provider_id: provider.provider_id.clone(),
                }
            })
        })
        .collect();
    agent_semantic_client_local_cli::ProviderProjectScopeFiles::Supported(files)
}

fn append_provider_scope_files(
    files: &mut Vec<agent_semantic_client_db::ClientDbSourceIndexScopeFile>,
    provider: &agent_semantic_client_core::ResolvedProvider,
    receipt: agent_semantic_client_local_cli::ProviderProjectScopeFiles,
) -> Result<(), String> {
    match receipt {
        agent_semantic_client_local_cli::ProviderProjectScopeFiles::Supported(provider_files) => {
            for provider_file in provider_files {
                let agent_semantic_client_local_cli::ProviderProjectScopePathFile {
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
            Ok(())
        }
        agent_semantic_client_local_cli::ProviderProjectScopeFiles::Unsupported => Err(format!(
            "provider workspace scope is unsupported: languageId={} providerId={}",
            provider.language_id, provider.provider_id
        )),
    }
}

pub(crate) fn collect_workspace_search_source_index_files(
    project_root: &std::path::Path,
    provider_registry: &agent_semantic_client_core::ProviderRegistrySnapshot,
    scope: &SourceIndexCollectionScope,
) -> Result<Vec<agent_semantic_client_db::ClientDbSourceIndexScopeFile>, String> {
    collect_source_index_files(project_root, provider_registry, scope)
}

#[cfg(test)]
#[path = "../../tests/unit/source_index_collect.rs"]
mod tests;
