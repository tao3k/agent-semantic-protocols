//! Provider-fact consumers for ASP Server-owned source-index publication.

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceIndexCollectionScope {
    CompleteGeneration,
    ExplicitOwners {
        owner_paths: Vec<String>,
    },
    TargetProvider {
        language_id: agent_semantic_client_core::LanguageId,
        provider_id: agent_semantic_client_core::ProviderId,
    },
    TargetProviderId {
        provider_id: agent_semantic_client_core::ProviderId,
    },
}

impl SourceIndexCollectionScope {
    pub(super) fn explicit_owner_paths(&self) -> Option<&[String]> {
        match self {
            Self::ExplicitOwners { owner_paths } => Some(owner_paths),
            Self::CompleteGeneration
            | Self::TargetProvider { .. }
            | Self::TargetProviderId { .. } => None,
        }
    }
}

fn explicit_owner_paths(
    scope: &SourceIndexCollectionScope,
) -> Result<Option<std::collections::BTreeSet<std::path::PathBuf>>, String> {
    let SourceIndexCollectionScope::ExplicitOwners { owner_paths } = scope else {
        return Ok(None);
    };
    if owner_paths.is_empty() {
        return Err("explicit owner collection requires at least one owner path".to_owned());
    }
    let mut requested = std::collections::BTreeSet::new();
    for owner_path in owner_paths {
        let owner_path = std::path::PathBuf::from(owner_path);
        if owner_path.is_absolute()
            || owner_path.components().any(|component| {
                matches!(
                    component,
                    std::path::Component::CurDir
                        | std::path::Component::ParentDir
                        | std::path::Component::RootDir
                        | std::path::Component::Prefix(_)
                )
            })
        {
            return Err(format!(
                "explicit owner collection path must be normalized and workspace-relative: {}",
                owner_path.display()
            ));
        }
        requested.insert(owner_path);
    }
    Ok(Some(requested))
}

fn retain_explicit_owner_files(
    project_root: &std::path::Path,
    scope: &SourceIndexCollectionScope,
    files: &mut Vec<crate::ClientDbSourceIndexScopeFile>,
) -> Result<(), String> {
    let Some(requested) = explicit_owner_paths(scope)? else {
        return Ok(());
    };
    files.retain(|file| {
        let relative = file
            .path
            .strip_prefix(project_root)
            .unwrap_or(file.path.as_path());
        requested.contains(relative)
    });
    Ok(())
}

fn provider_project_resolution_collection_scope(
    scope: &SourceIndexCollectionScope,
) -> agent_semantic_client_server::ProviderProjectResolutionCollectionScope {
    match scope {
        SourceIndexCollectionScope::ExplicitOwners { owner_paths } =>
            agent_semantic_client_server::ProviderProjectResolutionCollectionScope::ExplicitOwners {
                owner_paths: owner_paths.clone(),
            },
        SourceIndexCollectionScope::CompleteGeneration
        | SourceIndexCollectionScope::TargetProvider { .. }
        | SourceIndexCollectionScope::TargetProviderId { .. } =>
            agent_semantic_client_server::ProviderProjectResolutionCollectionScope::CompleteGeneration,
    }
}

fn provider_applicability_is_required(scope: &SourceIndexCollectionScope) -> bool {
    matches!(
        scope,
        SourceIndexCollectionScope::TargetProvider { .. }
            | SourceIndexCollectionScope::TargetProviderId { .. }
    )
}

pub(crate) struct SourceIndexCollectionReceipt {
    pub(crate) files: Vec<crate::ClientDbSourceIndexScopeFile>,
    pub(crate) project_resolutions: Vec<agent_semantic_content_identity::AdmittedProjectResolution>,
    pub(crate) candidate: crate::runtime_server_admission::WorkspaceGenerationCandidateIdentity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProviderSourceInventorySelection {
    ProjectResolution,
    GitDocumentCandidates,
    NotApplicable { reason_kind: &'static str },
}

fn provider_source_inventory_selection(
    provider: &agent_semantic_client_core::RuntimeProvider,
    candidates: &agent_semantic_runtime::git::RepositoryCandidateSnapshot,
) -> ProviderSourceInventorySelection {
    select_provider_source_inventory_adapter(
        &provider.source_inventory_capabilities,
        |entry_marker| {
            candidates
                .candidates
                .iter()
                .any(|candidate| candidate.path == std::path::Path::new(entry_marker))
        },
    )
}

fn select_provider_source_inventory_adapter(
    capabilities: &agent_semantic_client_core::ProviderSourceInventoryCapabilities,
    has_candidate_path: impl Fn(&str) -> bool,
) -> ProviderSourceInventorySelection {
    if let Some(project_resolution) = capabilities.project_resolution.as_ref() {
        if project_resolution
            .entry_markers
            .iter()
            .any(|entry_marker| has_candidate_path(entry_marker))
        {
            return ProviderSourceInventorySelection::ProjectResolution;
        }
    }
    if let Some(document_resolution) = capabilities.document_resolution.as_ref() {
        if document_resolution.supports_git_candidates {
            return ProviderSourceInventorySelection::GitDocumentCandidates;
        }
        return ProviderSourceInventorySelection::NotApplicable {
            reason_kind: "provider-document-resolution-git-candidates-unsupported",
        };
    }
    ProviderSourceInventorySelection::NotApplicable {
        reason_kind: "provider-source-inventory-capability-unavailable",
    }
}

pub(crate) async fn collect_source_index_scope_with_runtime_service_async(
    runtime: &crate::runtime_search_service::RuntimeSearchServiceHandle,
    project_root: &std::path::Path,
    provider_registry: &agent_semantic_client_core::RuntimeProviderProjection,
    scope: &SourceIndexCollectionScope,
    cancellation: crate::runtime_generation_cancellation::GenerationCancellation,
) -> Result<SourceIndexCollectionReceipt, String> {
    collect_source_index_scope_with_executor_async(
        ProviderScopeExecutor::RuntimeService(runtime.clone(), cancellation),
        project_root,
        provider_registry,
        scope,
    )
    .await
}

#[derive(Clone)]
enum ProviderScopeExecutor {
    RuntimeService(
        crate::runtime_search_service::RuntimeSearchServiceHandle,
        crate::runtime_generation_cancellation::GenerationCancellation,
    ),
}

async fn collect_source_index_scope_with_executor_async(
    executor: ProviderScopeExecutor,
    project_root: &std::path::Path,
    provider_registry: &agent_semantic_client_core::RuntimeProviderProjection,
    scope: &SourceIndexCollectionScope,
) -> Result<SourceIndexCollectionReceipt, String> {
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
            SourceIndexCollectionScope::CompleteGeneration
            | SourceIndexCollectionScope::ExplicitOwners { .. } => true,
            SourceIndexCollectionScope::TargetProvider {
                language_id,
                provider_id,
            } => language_id == &provider.language_id && provider_id == &provider.provider_id,
            SourceIndexCollectionScope::TargetProviderId { provider_id } => {
                provider_id == &provider.provider_id
            }
        };
        if !selected {
            continue;
        }
        let project_root = project_root.to_path_buf();
        let provider = provider.clone();
        let repository_candidates = repository_candidates.clone();
        let collection_scope = provider_project_resolution_collection_scope(scope);
        let provider_is_required = provider_applicability_is_required(scope);
        let executor = executor.clone();
        providers.spawn(async move {
            let selection =
                provider_source_inventory_selection(&provider, &repository_candidates);
            if let ProviderSourceInventorySelection::NotApplicable { reason_kind } = selection {
                if provider_is_required {
                    return Err(format!(
                        "reasonKind={} languageId={} providerId={} detail=no declared capability satisfies the Runtime candidate snapshot",
                        reason_kind, provider.language_id, provider.provider_id
                    ));
                }
                return Ok::<_, String>((provider, None, None));
            }
            let (receipt, project_resolution) =
                match selection {
                ProviderSourceInventorySelection::ProjectResolution => {
                    let package_root_path = std::path::PathBuf::from(&provider.binary);
                        let resolution = match executor {
ProviderScopeExecutor::RuntimeService(runtime, cancellation) => {
                                let (request, candidates) =
                                    agent_semantic_client_server::encode_provider_project_resolution_request(
                                        &project_root,
                                        &provider.language_id,
                                        &provider.provider_id,
                                        &collection_scope,
                                        &repository_candidates,
                                    )?;
                                runtime
                                    .provider_runtime(
                                        project_root.clone(),
                                        provider.language_id.as_str().to_owned(),
                                    )
                                    .await?;
                                runtime
        .provider_runtime_await_ready(
            project_root.clone(),
            provider.language_id.as_str().to_owned(),
            cancellation.clone(),
        )
                                    .await?;
                                let response = runtime
                                    .provider_operation(
                                        project_root.clone(),
                                        provider.language_id.as_str().to_owned(),
            "project-resolution".to_owned(),
            request,
            cancellation,
        )
                                    .await?;
                                agent_semantic_client_server::project_resolution_from_stdout(
                                    &response,
                                    &provider.language_id,
                                    &provider.provider_id,
                                    &candidates,
                                )?
                            }
                        };
                    let agent_semantic_client_server::ProviderProjectResolution::Supported(
                        packet,
                    ) = resolution
                    else {
                        return Ok::<_, String>((
                            provider,
                            Some(
                                agent_semantic_client_server::ProviderProjectResolutionFiles::Unsupported,
                            ),
                            None,
                        ));
                    };
                    let admitted = agent_semantic_content_identity::AdmittedProjectResolution::new(
                        ".",
                        packet.resolution.clone(),
                    )?;
                    let files = agent_semantic_client_server::provider_project_resolution_files_from_packet_async(
                        &project_root,
                        &package_root_path,
                        packet,
                    )
                    .await?;
                    (
                        agent_semantic_client_server::ProviderProjectResolutionFiles::Supported(
                            files,
                        ),
                        Some(admitted),
                    )
                }
                ProviderSourceInventorySelection::GitDocumentCandidates => {
                    (
                        document_scope_files(&project_root, &provider, &repository_candidates),
                        None,
                    )
                }
                ProviderSourceInventorySelection::NotApplicable { .. } => unreachable!(
                    "not-applicable provider inventory is handled before provider dispatch"
                ),
            };
            Ok::<_, String>((provider, Some(receipt), project_resolution))
        });
    }
    let mut files = Vec::new();
    let mut project_resolutions = Vec::new();
    while let Some(result) = providers.join_next().await {
        let (provider, receipt, project_resolution) =
            result.map_err(|error| format!("provider scope task failed: {error}"))??;
        if let Some(receipt) = receipt {
            append_provider_scope_files(&mut files, &provider, receipt)?;
        }
        if let Some(project_resolution) = project_resolution {
            project_resolutions.push(project_resolution);
        }
    }
    retain_explicit_owner_files(project_root, scope, &mut files)?;
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
    project_resolutions.sort_by(|left, right| {
        (
            &left.candidate_base,
            &left.resolution.provider_id,
            &left.resolution.project_entry,
        )
            .cmp(&(
                &right.candidate_base,
                &right.resolution.provider_id,
                &right.resolution.project_entry,
            ))
    });
    Ok(SourceIndexCollectionReceipt {
        files,
        project_resolutions,
        candidate:
            crate::runtime_server_admission::WorkspaceGenerationCandidateIdentity::from_snapshot(
                &repository_candidates,
            ),
    })
}

fn document_scope_files(
    project_root: &std::path::Path,
    provider: &agent_semantic_client_core::RuntimeProvider,
    repository_candidates: &agent_semantic_runtime::git::RepositoryCandidateSnapshot,
) -> agent_semantic_client_server::ProviderProjectResolutionFiles {
    let files = repository_candidates
        .candidates
        .iter()
        .filter(|candidate| {
            provider
                .source_inventory_capabilities
                .document_resolution
                .as_ref()
                .is_some_and(|document_resolution| {
                    document_resolution.extensions.iter().any(|extension| {
                        candidate
                            .path
                            .to_string_lossy()
                            .ends_with(extension.as_str())
                    })
                })
        })
        .filter_map(|candidate| {
            let candidate_path = candidate.path.to_str()?;
            let path = agent_semantic_client_core::scoped_child_path(project_root, candidate_path)?;
            path.is_file().then(|| {
                agent_semantic_client_server::ProviderProjectResolutionPathFile {
                    path,
                    language_id: provider.language_id.clone(),
                    provider_id: provider.provider_id.clone(),
                }
            })
        })
        .collect();
    agent_semantic_client_server::ProviderProjectResolutionFiles::Supported(files)
}

fn append_provider_scope_files(
    files: &mut Vec<crate::ClientDbSourceIndexScopeFile>,
    provider: &agent_semantic_client_core::RuntimeProvider,
    receipt: agent_semantic_client_server::ProviderProjectResolutionFiles,
) -> Result<(), String> {
    match receipt {
        agent_semantic_client_server::ProviderProjectResolutionFiles::Supported(provider_files) => {
            for provider_file in provider_files {
                let agent_semantic_client_server::ProviderProjectResolutionPathFile {
                    path,
                    language_id,
                    provider_id: reported_provider_id,
                } = provider_file;
                if language_id != provider.language_id {
                    return Err(format!(
                        "provider workspace scope language identity mismatch: admitted={} reported={} providerId={}",
                        provider.language_id, language_id, provider.provider_id
                    ));
                }
                if reported_provider_id != provider.provider_id {
                    return Err(format!(
                        "provider workspace scope provider identity mismatch: admitted={} reported={}",
                        provider.provider_id, reported_provider_id
                    ));
                }
                files.push(crate::ClientDbSourceIndexScopeFile {
                    path,
                    language_id,
                    provider_id: reported_provider_id,
                    projection_coverage: crate::ClientDbSourceIndexProjectionCoverage::NotDeclared,
                    selector_receipts: Vec::new(),
                    relations: Vec::new(),
                });
            }
            Ok(())
        }
        agent_semantic_client_server::ProviderProjectResolutionFiles::Unsupported => Err(format!(
            "provider workspace scope is unsupported: languageId={} providerId={}",
            provider.language_id, provider.provider_id
        )),
    }
}

#[cfg(test)]
#[path = "../../tests/unit/source_index_collect.rs"]
mod tests;
