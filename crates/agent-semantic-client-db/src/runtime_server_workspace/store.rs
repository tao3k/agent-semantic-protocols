use std::path::Path;

pub async fn prepare_runtime_server_workspace_store() -> Result<(), String> {
    prepare_runtime_server_workspace_store_at(&crate::runtime_server_runtime_base()).await
}

async fn prepare_runtime_server_workspace_store_at(runtime_base: &Path) -> Result<(), String> {
    let workspaces = runtime_base.join("workspaces");
    std::fs::create_dir_all(&workspaces).map_err(|error| {
        format!(
            "failed to prepare the Runtime Server workspace store {}: {error}",
            workspaces.display()
        )
    })
}

#[cfg(test)]
#[path = "../../tests/unit/runtime_server_workspace_store.rs"]
mod runtime_server_workspace_store_tests;
