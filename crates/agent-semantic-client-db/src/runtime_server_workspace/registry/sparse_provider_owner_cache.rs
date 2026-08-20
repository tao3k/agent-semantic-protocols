use std::{collections::HashMap, path::Path};

use parking_lot::RwLock;

use crate::runtime_server_workspace::WorkspaceOwnerSnapshot;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SparseProviderOwnerKey {
    workspace_identity: String,
    project_root: String,
    owner_path: String,
}

#[derive(Debug, Clone)]
struct SparseProviderOwnerEntry {
    revision: u64,
    cache_digest: String,
    root_digest: String,
    owner: WorkspaceOwnerSnapshot,
}

#[derive(Debug, Default)]
struct SparseProviderOwnerState {
    revision: u64,
    owners: HashMap<SparseProviderOwnerKey, SparseProviderOwnerEntry>,
}

#[derive(Debug)]
pub(super) struct SparseProviderOwnerCache {
    capacity: usize,
    state: RwLock<SparseProviderOwnerState>,
}

impl SparseProviderOwnerCache {
    pub(super) fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            state: RwLock::new(SparseProviderOwnerState::default()),
        }
    }

    pub(super) fn publish(
        &self,
        workspace_identity: &str,
        project_root: &Path,
        owner: WorkspaceOwnerSnapshot,
    ) -> Result<(), String> {
        let key = SparseProviderOwnerKey {
            workspace_identity: workspace_identity.to_owned(),
            project_root: project_root.display().to_string(),
            owner_path: owner.owner_path.clone(),
        };
        let encoded = serde_json::to_vec(&(
            workspace_identity,
            project_root.display().to_string(),
            &owner,
        ))
        .map_err(|error| format!("encode sparse provider owner cache identity: {error}"))?;
        let cache_digest = format!("blake3-256:{}", blake3::hash(&encoded).to_hex());
        let root_digest = owner.content_digest.clone();
        let mut state = self.state.write();
        state.revision = state.revision.saturating_add(1);
        let revision = state.revision;
        state.owners.insert(
            key,
            SparseProviderOwnerEntry {
                revision,
                cache_digest,
                root_digest,
                owner,
            },
        );
        while state.owners.len() > self.capacity {
            let Some(oldest) = state
                .owners
                .iter()
                .min_by_key(|(_, entry)| entry.revision)
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            state.owners.remove(&oldest);
        }
        Ok(())
    }

    pub(super) fn read_validated(
        &self,
        workspace_identity: &str,
        project_root: &Path,
        owner_path: &str,
        source_leaf_digest: &str,
    ) -> Option<(String, String, WorkspaceOwnerSnapshot)> {
        let key = SparseProviderOwnerKey {
            workspace_identity: workspace_identity.to_owned(),
            project_root: project_root.display().to_string(),
            owner_path: owner_path.to_owned(),
        };
        {
            let state = self.state.read();
            match state.owners.get(&key) {
                Some(entry) if entry.root_digest == source_leaf_digest => {
                    return Some((
                        entry.cache_digest.clone(),
                        entry.root_digest.clone(),
                        entry.owner.clone(),
                    ));
                }
                None => return None,
                Some(_) => {}
            }
        }
        let mut state = self.state.write();
        if state
            .owners
            .get(&key)
            .is_some_and(|entry| entry.root_digest != source_leaf_digest)
        {
            state.owners.remove(&key);
        }
        None
    }

    pub(super) fn evict_scope(&self, workspace_identity: &str, project_root: &Path) {
        let project_root = project_root.display().to_string();
        self.state.write().owners.retain(|key, _| {
            key.workspace_identity != workspace_identity || key.project_root != project_root
        });
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/sparse_provider_owner_cache.rs"]
mod tests;
