// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Cleanup and stable-launcher operations for Runtime artifact publication.

use std::path::Path;

use crate::runtime_artifact_publication::PreparedRuntimeArtifactBundleMember;
use crate::runtime_artifact_slots::discard_prepared_runtime_artifact;

pub(super) async fn discard_prepared_runtime_artifact_bundle_members(
    members: &[PreparedRuntimeArtifactBundleMember],
) -> Result<(), String> {
    for member in members {
        discard_prepared_runtime_artifact(&member.artifact).await?;
    }
    Ok(())
}

pub(super) fn repair_empty_artifact_selector_under_guard(slot: &Path) -> Result<(), String> {
    let metadata = match std::fs::symlink_metadata(slot) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(format!(
                "state=runtime-artifact-publication-failed reasonKind=artifact-selector-unreadable path={} error={error}",
                slot.display()
            ));
        }
    };
    if metadata.file_type().is_symlink() {
        return Ok(());
    }
    if !metadata.is_dir() {
        return Err(format!(
            "state=runtime-artifact-publication-failed reasonKind=artifact-selector-type-conflict path={}",
            slot.display()
        ));
    }
    let mut entries = std::fs::read_dir(slot).map_err(|error| {
        format!(
            "state=runtime-artifact-publication-failed reasonKind=artifact-selector-unreadable path={} error={error}",
            slot.display()
        )
    })?;
    if entries.next().is_some() {
        return Err(format!(
            "state=runtime-artifact-publication-failed reasonKind=artifact-selector-directory-conflict path={}",
            slot.display()
        ));
    }
    std::fs::remove_dir(slot).map_err(|error| {
        format!(
            "state=runtime-artifact-publication-failed reasonKind=artifact-selector-repair-failed path={} error={error}",
            slot.display()
        )
    })
}

pub(super) fn publish_runtime_bundle_member_launcher(
    target: &Path,
    candidate: &Path,
    publication_nonce: &str,
    artifact_kind: &str,
) -> Result<(), String> {
    let parent = target.parent().ok_or_else(|| {
        format!(
            "Runtime bundle launcher has no parent: {}",
            target.display()
        )
    })?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("create Runtime bundle launcher directory: {error}"))?;
    match std::fs::read_link(target) {
        Ok(current) if current == candidate => return Ok(()),
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error)
            if error.kind() == std::io::ErrorKind::InvalidInput
                && std::fs::symlink_metadata(target).is_ok_and(|metadata| metadata.is_file()) =>
        {
            // A direct executable is a retired pre-slot launcher. The staged
            // symlink rename below atomically replaces it; it is deliberately
            // not retained as rollback or fallback authority.
        }
        Err(error) => {
            return Err(format!(
                "read Runtime bundle launcher {}: {error}",
                target.display()
            ));
        }
    }
    let staged = parent.join(format!(".{artifact_kind}.{publication_nonce}.tmp"));
    match std::fs::remove_file(&staged) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("remove stale Runtime bundle launcher: {error}")),
    }
    stage_runtime_bundle_launcher(candidate, &staged)?;
    let observed = std::fs::read_link(&staged)
        .map_err(|error| format!("read staged Runtime bundle launcher: {error}"))?;
    if observed != candidate {
        let _ = std::fs::remove_file(&staged);
        return Err("reasonKind=runtime-bundle-launcher-candidate-mismatch".to_owned());
    }
    std::fs::rename(&staged, target).map_err(|error| {
        let _ = std::fs::remove_file(&staged);
        format!(
            "reasonKind=runtime-bundle-launcher-publication-failed target={} error={error}",
            target.display()
        )
    })
}

pub(super) fn validate_runtime_bundle_launcher(
    target: &Path,
    candidate: &Path,
) -> Result<(), String> {
    let expected = std::fs::canonicalize(candidate)
        .map_err(|error| format!("resolve active Runtime bundle member: {error}"))?;
    let observed = std::fs::canonicalize(target)
        .map_err(|error| format!("resolve Runtime bundle launcher: {error}"))?;
    if observed != expected {
        return Err(format!(
            "reasonKind=runtime-bundle-launcher-active-mismatch target={} candidate={}",
            target.display(),
            candidate.display()
        ));
    }
    Ok(())
}

#[cfg(unix)]
pub(super) fn stage_runtime_bundle_launcher(candidate: &Path, staged: &Path) -> Result<(), String> {
    std::os::unix::fs::symlink(candidate, staged)
        .map_err(|error| format!("stage Runtime client launcher: {error}"))
}

#[cfg(not(unix))]
pub(super) fn stage_runtime_bundle_launcher(candidate: &Path, staged: &Path) -> Result<(), String> {
    std::fs::copy(candidate, staged)
        .map(|_| ())
        .map_err(|error| format!("stage Runtime client launcher: {error}"))
}
