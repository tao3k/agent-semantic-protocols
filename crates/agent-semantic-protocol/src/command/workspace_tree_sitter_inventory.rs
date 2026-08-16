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

pub(super) async fn collect_runtime_inventory(
    session: &agent_semantic_client_db::WorkspaceDbIpcSession,
    provider_workspace_root: &Path,
    provider: &ActivatedProvider,
) -> Result<Vec<InventoryOwner>, String> {
    let lookup = session
        .read_tree_sitter_inventory(
            &agent_semantic_client_db::workspace_db_ipc::WorkspaceDbSourceIndexLookupRequest {
                project_root: provider_workspace_root.to_path_buf(),
                indexed_project_root: provider_workspace_root.to_path_buf(),
                query: provider.source_extensions.join(" "),
                language_id: Some(provider.language_id.clone().into()),
                limit: u32::MAX,
            },
        )
        .await?;
    let mut owners = Vec::new();
    for candidate in lookup.candidates {
        let relative_path = PathBuf::from(candidate.path.to_string());
        push_provider_owner(
            provider_workspace_root.join(&relative_path),
            relative_path,
            provider,
            &mut owners,
        )?;
    }
    owners.sort_by(|left, right| left.owner_path.cmp(&right.owner_path));
    owners.dedup_by(|left, right| left.owner_path == right.owner_path);
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
