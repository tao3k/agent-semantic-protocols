//! Provider-fact consumers for ASP Server-owned source-index publication.

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceIndexCollectionScope {
    CompleteGeneration,
}

pub(crate) struct SourceIndexCollectionReceipt {
    pub(crate) files: Vec<crate::ClientDbSourceIndexScopeFile>,
    pub(crate) project_resolutions: Vec<agent_semantic_content_identity::AdmittedProjectResolution>,
    pub(crate) candidate: crate::runtime_server_admission::WorkspaceGenerationCandidateIdentity,
}

pub(crate) fn collect_source_index_scope_from_inventory(
    project_root: &std::path::Path,
    provider_registry: &agent_semantic_client_core::RuntimeProviderProjection,
    scope: &SourceIndexCollectionScope,
    inventory: &[String],
    candidate: crate::runtime_server_admission::WorkspaceGenerationCandidateIdentity,
) -> Result<SourceIndexCollectionReceipt, String> {
    candidate.validate()?;
    let candidate_paths = inventory
        .iter()
        .map(std::path::PathBuf::from)
        .collect::<Vec<_>>();
    collect_source_index_scope_from_paths(
        project_root,
        provider_registry,
        scope,
        &candidate_paths,
        candidate,
    )
}

fn collect_source_index_scope_from_paths(
    project_root: &std::path::Path,
    provider_registry: &agent_semantic_client_core::RuntimeProviderProjection,
    scope: &SourceIndexCollectionScope,
    candidate_paths: &[std::path::PathBuf],
    candidate: crate::runtime_server_admission::WorkspaceGenerationCandidateIdentity,
) -> Result<SourceIndexCollectionReceipt, String> {
    let mut files = Vec::new();
    for provider in &provider_registry.providers {
        let SourceIndexCollectionScope::CompleteGeneration = scope;
        if provider.source_extensions.is_empty() {
            return Err(format!(
                "base generation requires declarative source extensions: languageId={} providerId={}",
                provider.language_id, provider.provider_id
            ));
        }
        append_provider_scope_files(
            &mut files,
            provider,
            declarative_source_scope_files(project_root, provider, candidate_paths),
        )?;
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
    Ok(SourceIndexCollectionReceipt {
        files,
        project_resolutions: Vec::new(),
        candidate,
    })
}

fn declarative_source_scope_files(
    project_root: &std::path::Path,
    provider: &agent_semantic_client_core::RuntimeProvider,
    candidate_paths: &[std::path::PathBuf],
) -> agent_semantic_client_server::ProviderProjectResolutionFiles {
    let files = collect_declarative_source_paths(
        project_root,
        candidate_paths.iter().map(std::path::PathBuf::as_path),
        &provider.source_extensions,
    )
    .into_iter()
    .map(
        |path| agent_semantic_client_server::ProviderProjectResolutionPathFile {
            path,
            language_id: provider.language_id.clone(),
            provider_id: provider.provider_id.clone(),
        },
    )
    .collect();
    agent_semantic_client_server::ProviderProjectResolutionFiles::Supported(files)
}

fn collect_declarative_source_paths<'a>(
    project_root: &std::path::Path,
    candidates: impl IntoIterator<Item = &'a std::path::Path>,
    source_extensions: &[String],
) -> Vec<std::path::PathBuf> {
    candidates
        .into_iter()
        .filter(|candidate| {
            let path = candidate.to_string_lossy();
            source_extensions
                .iter()
                .any(|extension| path.ends_with(extension.as_str()))
        })
        .filter_map(|candidate| {
            let candidate = candidate.to_str()?;
            let path = agent_semantic_client_core::scoped_child_path(project_root, candidate)?;
            path.is_file().then_some(path)
        })
        .collect()
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
