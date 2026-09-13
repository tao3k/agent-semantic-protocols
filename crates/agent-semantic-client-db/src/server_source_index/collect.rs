// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

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
        files.extend(declarative_source_scope_files(
            project_root,
            provider,
            candidate_paths,
        ));
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
) -> Vec<crate::ClientDbSourceIndexScopeFile> {
    collect_declarative_source_paths(
        project_root,
        candidate_paths.iter().map(std::path::PathBuf::as_path),
        &provider.source_extensions,
    )
    .into_iter()
    .map(|path| crate::ClientDbSourceIndexScopeFile {
        path,
        language_id: provider.language_id.clone(),
        provider_id: provider.provider_id.clone(),
        projection_coverage: crate::ClientDbSourceIndexProjectionCoverage::NotDeclared,
        projection_diagnostic: None,
        selector_receipts: Vec::new(),
        relations: Vec::new(),
    })
    .collect()
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

#[cfg(test)]
#[path = "../../tests/unit/source_index_collect.rs"]
mod tests;
