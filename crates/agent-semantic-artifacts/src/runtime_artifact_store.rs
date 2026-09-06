// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Immutable generation-member snapshots and stable artifact links.

use std::path::Path;
use std::path::PathBuf;

pub(crate) fn runtime_artifact_content_digest(
    path: &Path,
) -> Result<crate::blake3_content_digest::Blake3ContentDigest, String> {
    use std::io::Read as _;

    let mut file = std::fs::File::open(path).map_err(|error| {
        format!(
            "failed to open runtime artifact {}: {error}",
            path.display()
        )
    })?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|error| {
            format!(
                "failed to read runtime artifact {}: {error}",
                path.display()
            )
        })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let content = agent_semantic_content_identity::exact_selector_merkle::ContentDigestV1::parse(
        hasher.finalize().to_hex().as_str(),
    )
    .map_err(|error| format!("encode runtime artifact content digest: {error}"))?;
    Ok(crate::blake3_content_digest::Blake3ContentDigest::from_content_digest(content))
}

pub(crate) fn stage_runtime_artifact(source: &Path, staged: &Path) -> Result<(), String> {
    // The generation store is an immutable snapshot boundary. A hard
    // link would leave the published artifact on the source executable's inode,
    // so a later build-side chmod or in-place update could mutate the active
    // Runtime artifact and invalidate its identity receipt.
    std::fs::copy(source, staged).map_err(|error| {
        format!(
            "failed to snapshot runtime artifact {}: {error}",
            staged.display()
        )
    })?;
    let permissions = std::fs::metadata(source)
        .map_err(|error| format!("failed to inspect {}: {error}", source.display()))?
        .permissions();
    std::fs::set_permissions(staged, permissions)
        .map_err(|error| format!("failed to chmod {}: {error}", staged.display()))?;
    Ok(())
}

/// Qualifies the complete active Runtime bundle after verifying the named
/// member has the exact health-qualified content digest.
///
/// There is only one active/healthy pair. The binary name selects a member for
/// validation; it never selects an independent mutable slot.
pub async fn promote_active_runtime_artifact_to_healthy(
    state_home: &Path,
    binary: &str,
    qualified_digest: &crate::blake3_content_digest::Blake3ContentDigest,
) -> Result<crate::blake3_content_digest::Blake3ContentDigest, String> {
    if binary.is_empty()
        || Path::new(binary).components().count() != 1
        || Path::new(binary).file_name().and_then(|name| name.to_str()) != Some(binary)
    {
        return Err(format!(
            "invalid Runtime artifact binary identity: {binary:?}"
        ));
    }
    let state_home = state_home.to_path_buf();
    let binary = binary.to_owned();
    let qualified_digest = qualified_digest.clone();
    tokio::task::spawn_blocking(move || {
        let layout = crate::RuntimeArtifactStateLayout::new(&state_home);
        let _guard = crate::runtime_artifact_retention::RuntimeArtifactMutationGuard::try_acquire(
            layout.root(),
        )?;
        let active_bundle = std::fs::canonicalize(layout.active_slot()).map_err(|error| {
            format!("resolve active Runtime bundle: {error}")
        })?;
        let canonical_generation_store =
            std::fs::canonicalize(layout.generation_store()).map_err(|error| {
                format!("resolve Runtime generation store: {error}")
        })?;
        if !active_bundle.starts_with(&canonical_generation_store) {
            return Err(format!(
                "active Runtime bundle escapes generation store: {}",
                active_bundle.display()
            ));
        }
        let active_member = std::fs::canonicalize(active_bundle.join(&binary)).map_err(|error| {
            format!("resolve active Runtime bundle member `{binary}`: {error}")
        })?;
        let observed = runtime_artifact_content_digest(&active_member)?;
        if observed != qualified_digest {
            return Err(format!(
                "Runtime health identity does not qualify active bundle member: qualified={qualified_digest} active={observed}"
            ));
        }
        publish_runtime_artifact_link(&active_bundle, &layout.healthy_slot())?;
        crate::runtime_artifact_retention::prune_unreachable_runtime_artifacts_blocking(
            layout.root(),
        )?;
        Ok(observed)
    })
    .await
    .map_err(|error| format!("Runtime bundle health promotion task failed: {error}"))?
}

pub(crate) fn publish_runtime_artifact_link(artifact: &Path, target: &Path) -> Result<(), String> {
    let parent = target
        .parent()
        .ok_or_else(|| format!("Runtime artifact link has no parent: {}", target.display()))?;
    std::fs::create_dir_all(parent).map_err(|error| {
        format!(
            "create Runtime artifact link directory {}: {error}",
            parent.display()
        )
    })?;
    let staged = temporary_runtime_artifact_path(target);
    remove_stale_staged_artifact(&staged)?;
    stage_runtime_artifact_link(artifact, &staged)?;
    atomic_replace_runtime_artifact(&staged, target)
}

fn temporary_runtime_artifact_path(target: &Path) -> PathBuf {
    use std::sync::atomic::AtomicU64;
    use std::sync::atomic::Ordering;
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);

    let file_name = target
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("runtime-artifact");
    target.with_file_name(format!(
        ".{file_name}.{}.{}.tmp",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ))
}

fn remove_stale_staged_artifact(staged: &Path) -> Result<(), String> {
    if std::fs::symlink_metadata(staged).is_ok() {
        std::fs::remove_file(staged)
            .map_err(|error| format!("failed to remove stale {}: {error}", staged.display()))?;
    }
    Ok(())
}

#[cfg(unix)]
fn stage_runtime_artifact_link(artifact: &Path, staged: &Path) -> Result<(), String> {
    std::os::unix::fs::symlink(artifact, staged).map_err(|error| {
        format!(
            "failed to stage runtime artifact link {} -> {}: {error}",
            staged.display(),
            artifact.display()
        )
    })
}

#[cfg(windows)]
fn stage_runtime_artifact_link(artifact: &Path, staged: &Path) -> Result<(), String> {
    std::os::windows::fs::symlink_file(artifact, staged).map_err(|error| {
        format!(
            "failed to stage runtime artifact link {} -> {}: {error}",
            staged.display(),
            artifact.display()
        )
    })
}

#[cfg(not(any(unix, windows)))]
fn stage_runtime_artifact_link(artifact: &Path, staged: &Path) -> Result<(), String> {
    std::fs::copy(artifact, staged)
        .map(|_| ())
        .map_err(|error| {
            format!(
                "failed to stage runtime artifact {}: {error}",
                staged.display()
            )
        })
}

fn atomic_replace_runtime_artifact(staged: &Path, target: &Path) -> Result<(), String> {
    std::fs::rename(staged, target)
        .map_err(|error| format!("failed to atomically publish {}: {error}", target.display()))
}
