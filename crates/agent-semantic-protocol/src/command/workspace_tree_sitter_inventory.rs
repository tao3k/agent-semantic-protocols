use std::path::{Path, PathBuf};

use agent_semantic_hook::ActivatedProvider;

use super::workspace_tree_sitter_query::{InventoryOwner, registered_source_path};

#[derive(Debug)]
struct ProviderProjectScope {
    project_roots: Vec<PathBuf>,
    source_roots: Vec<String>,
}

impl ProviderProjectScope {
    fn discover(
        candidate_paths: &[PathBuf],
        config_files: &[String],
        package_roots: &[String],
        source_roots: &[String],
    ) -> Self {
        let mut project_roots = candidate_paths
            .iter()
            .filter(|path| {
                let path = normalized_path(path);
                config_files
                    .iter()
                    .any(|selector| selector_matches_path(selector, path.as_str()))
            })
            .filter_map(|path| path.parent().map(Path::to_path_buf))
            .collect::<Vec<_>>();

        if package_roots
            .iter()
            .any(|root| root.trim().trim_matches('/') == ".")
        {
            project_roots.push(PathBuf::new());
        }
        for path in candidate_paths {
            for ancestor in path.ancestors().skip(1) {
                let ancestor = normalized_path(ancestor);
                if package_roots
                    .iter()
                    .any(|selector| selector_matches_path(selector, ancestor.as_str()))
                {
                    project_roots.push(PathBuf::from(ancestor));
                }
            }
        }
        if project_roots.is_empty() {
            project_roots.push(PathBuf::new());
        }
        project_roots.sort();
        project_roots.dedup();

        Self {
            project_roots,
            source_roots: source_roots.to_vec(),
        }
    }

    fn contains_source(&self, relative_path: &Path) -> bool {
        self.project_roots.iter().any(|project_root| {
            let Ok(project_relative) = relative_path.strip_prefix(project_root) else {
                return false;
            };
            self.source_roots.is_empty()
                || self
                    .source_roots
                    .iter()
                    .any(|selector| selector_matches_directory(selector, project_relative))
        })
    }
}

pub(super) fn provider_path_is_ignored(owner_path: &str, ignored_path_prefixes: &[String]) -> bool {
    let owner_path = owner_path.trim_start_matches("./").trim_matches('/');
    ignored_path_prefixes.iter().any(|prefix| {
        let prefix = prefix.trim_start_matches("./").trim_matches('/');
        !prefix.is_empty()
            && (owner_path == prefix
                || owner_path
                    .strip_prefix(prefix)
                    .is_some_and(|suffix| suffix.starts_with('/')))
    })
}

pub(super) fn collect_provider_inventory(
    provider_workspace_root: &Path,
    provider: &ActivatedProvider,
) -> Result<Vec<InventoryOwner>, String> {
    let provider_workspace_root = std::fs::canonicalize(provider_workspace_root)
        .unwrap_or_else(|_| provider_workspace_root.to_path_buf());
    if let Some(repository_candidates) =
        agent_semantic_runtime::git::discover_repository_candidate_snapshot(
            &provider_workspace_root,
        )
        .map_err(|error| format!("failed to resolve repository candidate snapshot: {error}"))?
    {
        return collect_git_inventory(
            &provider_workspace_root,
            provider,
            &repository_candidates,
        );
    }

    Err(format!(
        "provider-only project resolution is required for non-Git workspace `{}`; unbounded workspace traversal is disabled",
        provider_workspace_root.display()
    ))
}

fn collect_git_inventory(
    provider_workspace_root: &Path,
    provider: &ActivatedProvider,
    repository_candidates: &agent_semantic_runtime::git::RepositoryCandidateSnapshot,
) -> Result<Vec<InventoryOwner>, String> {
    let candidates = repository_candidates
        .candidates
        .iter()
        .filter_map(|candidate| {
            let absolute_path = repository_candidates
                .worktree_identity
                .worktree_root
                .join(&candidate.path);
            absolute_path
                .strip_prefix(provider_workspace_root)
                .ok()
                .map(Path::to_path_buf)
        })
        .collect::<Vec<_>>();
    let project_scope = ProviderProjectScope::discover(
        &candidates,
        &provider.config_files,
        &provider.package_roots,
        &provider.source_roots,
    );
    let mut owners = Vec::new();
    for relative_path in candidates {
        if !project_scope.contains_source(&relative_path) {
            continue;
        }
        let absolute_path = provider_workspace_root.join(&relative_path);
        push_provider_owner(absolute_path, relative_path, provider, &mut owners)?;
    }
    owners.sort_by(|left, right| left.owner_path.cmp(&right.owner_path));
    Ok(owners)
}

fn normalized_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .trim_start_matches("./")
        .trim_matches('/')
        .to_string()
}

fn selector_matches_directory(selector: &str, project_relative: &Path) -> bool {
    let selector = selector.trim().trim_start_matches("./").trim_matches('/');
    if selector.is_empty() || selector == "." {
        return true;
    }
    let mut directory = project_relative.parent();
    while let Some(candidate) = directory {
        if selector_matches_path(selector, normalized_path(candidate).as_str()) {
            return true;
        }
        directory = candidate.parent();
    }
    false
}

fn selector_matches_path(selector: &str, relative_path: &str) -> bool {
    let selector = selector.trim().trim_start_matches("./").trim_matches('/');
    let relative_path = relative_path.trim_start_matches("./").trim_matches('/');
    if selector.is_empty() {
        return false;
    }
    if selector.contains('/') {
        wildcard_matches(selector.as_bytes(), relative_path.as_bytes())
    } else {
        let basename = relative_path.rsplit('/').next().unwrap_or(relative_path);
        wildcard_matches(selector.as_bytes(), basename.as_bytes())
    }
}

fn wildcard_matches(pattern: &[u8], value: &[u8]) -> bool {
    let (mut pattern_index, mut value_index) = (0, 0);
    let (mut star_index, mut star_value_index) = (None, 0);
    while value_index < value.len() {
        if pattern_index < pattern.len()
            && (pattern[pattern_index] == b'?' || pattern[pattern_index] == value[value_index])
        {
            pattern_index += 1;
            value_index += 1;
        } else if pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
            star_index = Some(pattern_index);
            pattern_index += 1;
            star_value_index = value_index;
        } else if let Some(star) = star_index {
            pattern_index = star + 1;
            star_value_index += 1;
            value_index = star_value_index;
        } else {
            return false;
        }
    }
    while pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
        pattern_index += 1;
    }
    pattern_index == pattern.len()
}

fn push_provider_owner(
    absolute_path: PathBuf,
    relative_path: PathBuf,
    provider: &ActivatedProvider,
    owners: &mut Vec<InventoryOwner>,
) -> Result<(), String> {
    let owner_path = relative_path.to_string_lossy().replace('\\', "/");
    if provider_path_is_ignored(&owner_path, &provider.ignored_path_prefixes)
        || !registered_source_path(owner_path.as_str(), &provider.source_extensions)
    {
        return Ok(());
    }
    owners.push(InventoryOwner {
        metadata: super::provider_owner_native::provider_owner_metadata(absolute_path.as_path())?,
        absolute_path,
        owner_path,
        probe: agent_semantic_client_db::ProviderOwnerProbe {
            decision: agent_semantic_client_db::ProviderOwnerDecision::New,
            generation_before: None,
            content_digest: None,
        },
        entry: agent_semantic_client_db::ProviderOwnerInventoryEntry {
            owner_path: String::new(),
            owner_content_digest: None,
            state: agent_semantic_client_db::ProviderOwnerInventoryEntryState::Unindexed,
        },
    });
    Ok(())
}

#[cfg(test)]
#[path = "../../tests/unit/command/workspace_tree_sitter_inventory.rs"]
mod tests;
