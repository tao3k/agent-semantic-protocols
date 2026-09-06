//! State Home identity and Runtime Server endpoint path derivation.

use std::path::{Path, PathBuf};

use agent_semantic_artifacts::RuntimeServingStateLayout;
use agent_semantic_artifacts::StateHomeLayout;

pub fn runtime_server_runtime_base(state_home: &Path) -> Result<PathBuf, String> {
    let canonical_state_home = std::fs::canonicalize(state_home).map_err(|error| {
        format!(
            "failed to canonicalize ASP State Home {} for Runtime Server identity: {error}",
            state_home.display()
        )
    })?;
    Ok(runtime_server_runtime_base_for_identity(
        &canonical_state_home,
    ))
}

pub async fn runtime_server_runtime_base_async(state_home: &Path) -> Result<PathBuf, String> {
    let canonical_state_home = tokio::fs::canonicalize(state_home).await.map_err(|error| {
        format!(
            "failed to canonicalize ASP State Home {} for Runtime Server identity: {error}",
            state_home.display()
        )
    })?;
    Ok(runtime_server_runtime_base_for_identity(
        &canonical_state_home,
    ))
}

pub fn runtime_server_endpoint_path(state_home: &Path) -> Result<PathBuf, String> {
    if let Some(publication_dir) = std::env::var_os("ASP_RUNTIME_SERVER_PUBLICATION_DIR") {
        return Ok(
            RuntimeServingStateLayout::from_root(publication_dir).injected_endpoint_receipt()
        );
    }
    Ok(
        RuntimeServingStateLayout::from_root(runtime_server_runtime_base(state_home)?)
            .endpoint_receipt(),
    )
}

pub(super) fn runtime_server_status_memory_path_for_identity(
    runtime_base: &Path,
    owner_epoch: u64,
    binding_token: &str,
    runtime_binary_content_digest: impl std::fmt::Display,
) -> PathBuf {
    RuntimeServingStateLayout::from_root(runtime_base.to_path_buf()).status_memory(
        owner_epoch,
        binding_token,
        runtime_binary_content_digest,
    )
}

pub async fn runtime_server_endpoint_path_async(state_home: &Path) -> Result<PathBuf, String> {
    if let Some(publication_dir) = std::env::var_os("ASP_RUNTIME_SERVER_PUBLICATION_DIR") {
        return Ok(
            RuntimeServingStateLayout::from_root(publication_dir).injected_endpoint_receipt()
        );
    }
    Ok(
        RuntimeServingStateLayout::from_root(runtime_server_runtime_base_async(state_home).await?)
            .endpoint_receipt(),
    )
}

fn runtime_server_runtime_base_for_identity(canonical_state_home: &Path) -> PathBuf {
    StateHomeLayout::new(canonical_state_home)
        .runtime_state()
        .serving()
        .root()
        .to_path_buf()
}
