use std::path::{Path, PathBuf};

use agent_semantic_hook::ActivatedProvider;

use super::workspace_tree_sitter_query::{InventoryOwner, registered_source_path};

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
    profiles: &agent_semantic_hook::RuntimeProfiles,
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
            profiles,
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
    profiles: &agent_semantic_hook::RuntimeProfiles,
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
        .collect::<std::collections::BTreeSet<_>>();
    let resolved_paths = super::provider_owner_native::run_provider_project_resolution(
        super::provider_owner_native::ProviderOwnerNativeTransportContext {
            language_id: provider.language_id.as_str(),
            provider,
            profiles,
            project_root: provider_workspace_root,
        },
        repository_candidates,
    )?;
    let mut owners = Vec::new();
    for relative_path in resolved_paths {
        if !candidates.contains(&relative_path) {
            return Err(format!(
                "provider project-resolution escaped repository candidates: languageId={} providerId={} path={}",
                provider.language_id,
                provider.provider_id,
                relative_path.display()
            ));
        }
        let absolute_path = provider_workspace_root.join(&relative_path);
        push_provider_owner(absolute_path, relative_path, provider, &mut owners)?;
    }
    owners.sort_by(|left, right| left.owner_path.cmp(&right.owner_path));
    Ok(owners)
}

fn push_provider_owner(
    absolute_path: PathBuf,
    relative_path: PathBuf,
    provider: &ActivatedProvider,
    owners: &mut Vec<InventoryOwner>,
) -> Result<(), String> {
    let owner_path = relative_path.to_string_lossy().replace('\\', "/");
    if provider_path_is_ignored(&owner_path, &[])
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
