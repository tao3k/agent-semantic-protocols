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
    let provider_files = match collect_provider_source_scope_files(
        project_root,
        snapshot,
        SOURCE_INDEX_FILE_LIMIT,
    ) {
        Ok(files) => files,
        Err(err)
            if err.starts_with("missing provider source-scope facts:")
                && snapshot_has_document_provider(snapshot) =>
        {
            collect_activation_scope_fallback_files(
                project_root,
                snapshot,
                SOURCE_INDEX_FILE_LIMIT,
            )?
        }
        Err(err) => return Err(err),
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

fn snapshot_has_document_provider(snapshot: &ProviderRegistrySnapshot) -> bool {
    snapshot.providers.iter().any(|provider| {
        matches!(
            provider.language_id.to_string().as_str(),
            "org" | "md" | "markdown"
        )
    })
}

fn collect_activation_scope_fallback_files(
    project_root: &Path,
    snapshot: &ProviderRegistrySnapshot,
    limit: usize,
) -> Result<Vec<agent_semantic_client_local_cli::ProviderWorkspaceScopePathFile>, String> {
    let mut files = std::collections::BTreeMap::new();
    for provider in &snapshot.providers {
        let scope_roots = provider_scope_roots(project_root, provider);
        for scope_root in scope_roots {
            collect_activation_scope_fallback_files_for_provider(
                project_root,
                &scope_root,
                provider,
                limit,
                &mut files,
            )?;
            if files.len() >= limit {
                break;
            }
        }
        for config_file in &provider.config_files {
            let path = workspace_path(project_root, config_file);
            if path.is_file() && !provider_ignores_path(project_root, provider, &path) {
                insert_activation_scope_file(project_root, provider, path, &mut files);
            }
            if files.len() >= limit {
                break;
            }
        }
        if files.len() >= limit {
            break;
        }
    }
    if files.is_empty() {
        return Err(format!(
            "missing provider source-scope facts: activated providers exposed no source/config files for {}; activation metadata fallback also found no matching files",
            project_root.display()
        ));
    }
    Ok(files.into_values().take(limit).collect())
}

fn provider_scope_roots(
    project_root: &Path,
    provider: &agent_semantic_client_core::ResolvedProvider,
) -> Vec<std::path::PathBuf> {
    let mut roots = Vec::new();
    if provider.source_roots.is_empty() && provider.package_roots.is_empty() {
        roots.push(project_root.to_path_buf());
    }
    for root in &provider.package_roots {
        roots.push(workspace_path(project_root, root));
    }
    for root in &provider.source_roots {
        roots.push(workspace_path(project_root, root));
    }
    roots.sort();
    roots.dedup();
    roots
}

fn collect_activation_scope_fallback_files_for_provider(
    project_root: &Path,
    scope_root: &Path,
    provider: &agent_semantic_client_core::ResolvedProvider,
    limit: usize,
    files: &mut std::collections::BTreeMap<
        std::path::PathBuf,
        agent_semantic_client_local_cli::ProviderWorkspaceScopePathFile,
    >,
) -> Result<(), String> {
    if files.len() >= limit || !scope_root.exists() {
        return Ok(());
    }
    let entries = std::fs::read_dir(scope_root).map_err(|err| {
        format!(
            "failed to read provider fallback scope {}: {err}",
            scope_root.display()
        )
    })?;
    for entry in entries {
        let entry = entry.map_err(|err| {
            format!(
                "failed to read provider fallback entry under {}: {err}",
                scope_root.display()
            )
        })?;
        let path = entry.path();
        if provider_ignores_path(project_root, provider, &path) {
            continue;
        }
        if path.is_dir() {
            collect_activation_scope_fallback_files_for_provider(
                project_root,
                &path,
                provider,
                limit,
                files,
            )?;
        } else if provider_matches_source_extension(provider, &path) {
            insert_activation_scope_file(project_root, provider, path, files);
        }
        if files.len() >= limit {
            break;
        }
    }
    Ok(())
}

fn insert_activation_scope_file(
    project_root: &Path,
    provider: &agent_semantic_client_core::ResolvedProvider,
    path: std::path::PathBuf,
    files: &mut std::collections::BTreeMap<
        std::path::PathBuf,
        agent_semantic_client_local_cli::ProviderWorkspaceScopePathFile,
    >,
) {
    let display_path = path
        .strip_prefix(project_root)
        .unwrap_or(&path)
        .to_path_buf();
    files.entry(display_path.clone()).or_insert_with(|| {
        agent_semantic_client_local_cli::ProviderWorkspaceScopePathFile {
            path: display_path,
            language_id: provider.language_id.clone(),
            provider_id: provider.provider_id.clone(),
        }
    });
}

fn provider_matches_source_extension(
    provider: &agent_semantic_client_core::ResolvedProvider,
    path: &Path,
) -> bool {
    let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
        return false;
    };
    if provider
        .source_extensions
        .iter()
        .any(|candidate| extension_matches(candidate, extension))
    {
        return true;
    }
    match provider.language_id.to_string().as_str() {
        "org" => extension == "org",
        "md" | "markdown" => matches!(extension, "md" | "markdown"),
        "rust" => extension == "rs",
        "python" => extension == "py",
        "typescript" => matches!(extension, "ts" | "tsx"),
        _ => false,
    }
}

fn extension_matches(candidate: &str, extension: &str) -> bool {
    candidate
        .trim_start_matches('.')
        .eq_ignore_ascii_case(extension)
}

fn provider_ignores_path(
    project_root: &Path,
    provider: &agent_semantic_client_core::ResolvedProvider,
    path: &Path,
) -> bool {
    let relative = path.strip_prefix(project_root).unwrap_or(path);
    provider.ignored_path_prefixes.iter().any(|prefix| {
        let prefix_path = std::path::Path::new(prefix);
        relative.starts_with(prefix_path) || path.starts_with(prefix_path)
    })
}

fn workspace_path(project_root: &Path, path: &str) -> std::path::PathBuf {
    let path = std::path::Path::new(path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        project_root.join(path)
    }
}
