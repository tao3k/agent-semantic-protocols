use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResidentRuntimeInstallReceipt {
    pub path: PathBuf,
    pub status: &'static str,
    pub artifact_digest: agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest,
    pub lock_acquisition_count: u8,
    pub quiescence_operation: String,
    pub quiescence_lease_nonce: String,
    pub lease_producer_process_id: u32,
    pub lease_consumer_process_id: u32,
}

/// Publishes an immutable ASP artifact and a typed activation event.
///
/// Runtime process activation is owned by the resident Tokio actor. Binary
/// installation never launches a child, binds readiness sockets, waits for a
/// generation, or drains a server while holding the artifact mutation lock.
pub async fn install_resident_runtime(
    state_home: &Path,
    source: &Path,
    target: &Path,
    artifact_mode: &str,
    previous_artifact_digest: Option<
        &agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest,
    >,
    qualified_source: Option<
        agent_semantic_artifacts::runtime_artifact_catalog::QualifiedRuntimeArtifactSource,
    >,
) -> Result<ResidentRuntimeInstallReceipt, String> {
    if let Some(authority) = qualified_source.as_ref() {
        let artifact_kind = target
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| "qualified Runtime artifact target has no binary name".to_owned())?;
        authority.validate_source(state_home, source, artifact_kind)?;
    }
    let receipt = agent_semantic_artifacts::runtime_artifact_publication::publish_runtime_artifact(
        state_home,
        source,
        target,
        artifact_mode,
        previous_artifact_digest,
    )
    .await?;
    Ok(ResidentRuntimeInstallReceipt {
        path: receipt.path,
        status: receipt.status,
        artifact_digest: receipt.artifact_digest,
        lock_acquisition_count: receipt.lock_acquisition_count,
        quiescence_operation: receipt.quiescence_operation,
        quiescence_lease_nonce: receipt.quiescence_lease_nonce,
        lease_producer_process_id: receipt.lease_producer_process_id,
        lease_consumer_process_id: receipt.lease_consumer_process_id,
    })
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use agent_semantic_artifacts::runtime_artifact_retention::RuntimeArtifactMutationGuard;

    use super::*;

    #[tokio::test]
    async fn install_publishes_without_a_runtime_child_and_releases_the_lock() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let state_home = temporary.path().join("state");
        let source = temporary.path().join("target-debug-asp");
        let target = temporary.path().join("bin/asp");
        tokio::fs::write(&source, b"#!/bin/sh\nexit 2\n")
            .await
            .expect("write candidate");
        std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o755))
            .expect("candidate permissions");

        let receipt = install_resident_runtime(&state_home, &source, &target, "dev", None, None)
            .await
            .expect("publication must not depend on candidate readiness");

        assert_eq!(receipt.status, "published-activation-pending");
        let artifact_root = state_home.join("runtime/artifacts");
        let guard = RuntimeArtifactMutationGuard::try_acquire(&artifact_root)
            .expect("publication must release the artifact lock before return");
        drop(guard);
    }
}
