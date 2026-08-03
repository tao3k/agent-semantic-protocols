use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct RuntimeServerWorkspaceStore {
    root: PathBuf,
    prepared: std::sync::Arc<tokio::sync::OnceCell<()>>,
}

impl RuntimeServerWorkspaceStore {
    pub fn for_runtime_base(runtime_base: &Path) -> Self {
        Self {
            root: runtime_base.join("workspaces"),
            prepared: std::sync::Arc::new(tokio::sync::OnceCell::new()),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub async fn prepare(&self) -> Result<(), String> {
        self.prepared
            .get_or_try_init(|| async {
                tokio::fs::create_dir_all(&self.root)
                    .await
                    .map_err(|error| {
                        format!(
                            "failed to prepare the Runtime Server workspace store {}: {error}",
                            self.root.display()
                        )
                    })
            })
            .await
            .map(|_| ())
    }
}

pub async fn prepare_runtime_server_workspace_store(
    runtime_base: &Path,
) -> Result<RuntimeServerWorkspaceStore, String> {
    let store = RuntimeServerWorkspaceStore::for_runtime_base(runtime_base);
    store.prepare().await?;
    Ok(store)
}

#[cfg(test)]
#[path = "../../tests/unit/runtime_server_workspace_store.rs"]
mod runtime_server_workspace_store_tests;
