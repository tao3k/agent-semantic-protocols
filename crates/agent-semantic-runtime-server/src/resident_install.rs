// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::path::Path;
use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResidentRuntimeInstallReceipt {
    pub path: PathBuf,
    pub status: &'static str,
    pub artifact_digest: agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest,
    pub bundle_digest: agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest,
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
    )
    .await?;
    Ok(ResidentRuntimeInstallReceipt {
        path: receipt.path,
        status: receipt.status,
        artifact_digest: receipt.artifact_digest,
        bundle_digest: receipt.bundle_digest,
        lock_acquisition_count: receipt.lock_acquisition_count,
        quiescence_operation: receipt.quiescence_operation,
        quiescence_lease_nonce: receipt.quiescence_lease_nonce,
        lease_producer_process_id: receipt.lease_producer_process_id,
        lease_consumer_process_id: receipt.lease_consumer_process_id,
    })
}

/// Publishes the ASP client and Hook evaluator into one candidate directory.
/// The Runtime activation actor switches the shared active/healthy directory
/// authority, so both executables become visible as one build cohort.
pub async fn install_resident_runtime_bundle(
    state_home: &Path,
    source: &Path,
    target: &Path,
    hook_source: &Path,
    artifact_mode: &str,
    qualified_source: Option<
        agent_semantic_artifacts::runtime_artifact_catalog::QualifiedRuntimeArtifactSource,
    >,
) -> Result<ResidentRuntimeInstallReceipt, String> {
    let members = [
        agent_semantic_artifacts::runtime_artifact_publication::RuntimeArtifactBundleMemberSource {
            name: "asp-hook",
            source: hook_source,
        },
    ];
    install_resident_runtime_bundle_members(
        state_home,
        source,
        target,
        &members,
        artifact_mode,
        qualified_source,
    )
    .await
}

pub async fn install_resident_runtime_bundle_members(
    state_home: &Path,
    source: &Path,
    target: &Path,
    members: &[agent_semantic_artifacts::runtime_artifact_publication::RuntimeArtifactBundleMemberSource<'_>],
    artifact_mode: &str,
    qualified_source: Option<
        agent_semantic_artifacts::runtime_artifact_catalog::QualifiedRuntimeArtifactSource,
    >,
) -> Result<ResidentRuntimeInstallReceipt, String> {
    if let Some(authority) = qualified_source.as_ref() {
        authority.validate_source(state_home, source, "asp")?;
        for member in members {
            authority.validate_source(state_home, member.source, member.name)?;
        }
    }
    let receipt = agent_semantic_artifacts::runtime_artifact_publication::publish_runtime_artifact_bundle_members(
            state_home,
            source,
            target,
            artifact_mode,
            members,
        )
        .await?;
    Ok(ResidentRuntimeInstallReceipt {
        path: receipt.path,
        status: receipt.status,
        artifact_digest: receipt.artifact_digest,
        bundle_digest: receipt.bundle_digest,
        lock_acquisition_count: receipt.lock_acquisition_count,
        quiescence_operation: receipt.quiescence_operation,
        quiescence_lease_nonce: receipt.quiescence_lease_nonce,
        lease_producer_process_id: receipt.lease_producer_process_id,
        lease_consumer_process_id: receipt.lease_consumer_process_id,
    })
}

/// Publishes one executable cohort together with its complete V2 execution
/// closure.  The binding is part of the immutable bundle identity; it is not a
/// mutable Runtime-side catalog and cannot be refreshed independently from the
/// executable bytes.
pub async fn install_resident_runtime_bound_bundle_members(
    state_home: &Path,
    source: &Path,
    target: &Path,
    members: &[agent_semantic_artifacts::runtime_artifact_publication::RuntimeArtifactBundleMemberSource<'_>],
    binding: &agent_semantic_artifacts::runtime_artifact_slots::RuntimeArtifactBundleBinding,
    artifact_mode: &str,
    qualified_source: Option<
        agent_semantic_artifacts::runtime_artifact_catalog::QualifiedRuntimeArtifactSource,
    >,
) -> Result<ResidentRuntimeInstallReceipt, String> {
    if let Some(authority) = qualified_source.as_ref() {
        authority.validate_source(state_home, source, "asp")?;
        for member in members {
            authority.validate_source(state_home, member.source, member.name)?;
        }
    }
    let receipt = agent_semantic_artifacts::runtime_artifact_publication::publish_runtime_artifact_bound_bundle_members(
        state_home,
        source,
        target,
        artifact_mode,
        members,
        binding,
    )
    .await?;
    Ok(ResidentRuntimeInstallReceipt {
        path: receipt.path,
        status: receipt.status,
        artifact_digest: receipt.artifact_digest,
        bundle_digest: receipt.bundle_digest,
        lock_acquisition_count: receipt.lock_acquisition_count,
        quiescence_operation: receipt.quiescence_operation,
        quiescence_lease_nonce: receipt.quiescence_lease_nonce,
        lease_producer_process_id: receipt.lease_producer_process_id,
        lease_consumer_process_id: receipt.lease_consumer_process_id,
    })
}

#[cfg(all(test, unix))]
#[path = "../tests/unit/resident_install.rs"]
mod tests;
