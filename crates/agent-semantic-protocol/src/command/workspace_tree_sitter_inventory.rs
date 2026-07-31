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
) -> Result<Vec<InventoryOwner>, String> {
    let mut owners = Vec::new();
    for source_path in &provider.source_paths {
        let relative_path = PathBuf::from(source_path);
        if relative_path.is_absolute()
            || relative_path
                .components()
                .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            return Err(format!(
                "activated ProjectResolution contains a non-relative source path: languageId={} providerId={} path={}",
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
