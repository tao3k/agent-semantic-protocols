// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Atomic symlink operations for Runtime artifact slots.

use std::path::Path;
use std::path::PathBuf;

use crate::runtime_artifact_store::publish_runtime_artifact_link;

pub(super) async fn publish_runtime_artifact_slot(
    target: &Path,
    slot: &Path,
) -> Result<(), String> {
    let target = target.to_path_buf();
    let slot = slot.to_path_buf();
    tokio::task::spawn_blocking(move || publish_runtime_artifact_link(&target, &slot))
        .await
        .map_err(|error| format!("publish Runtime artifact slot task failed: {error}"))?
}

pub(crate) fn publish_runtime_artifact_slot_under_guard(
    target: &Path,
    slot: &Path,
) -> Result<(), String> {
    publish_runtime_artifact_link(target, slot)
}

pub(super) async fn restore_runtime_artifact_slot(
    target: Option<&Path>,
    slot: &Path,
) -> Result<(), String> {
    match target {
        Some(target) => publish_runtime_artifact_slot(target, slot).await,
        None => match tokio::fs::remove_file(slot).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!(
                "remove Runtime artifact slot {} during rollback: {error}",
                slot.display()
            )),
        },
    }
}

pub(crate) fn restore_runtime_artifact_slot_under_guard(
    target: Option<&Path>,
    slot: &Path,
) -> Result<(), String> {
    match target {
        Some(target) => publish_runtime_artifact_slot_under_guard(target, slot),
        None => match std::fs::remove_file(slot) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!(
                "remove Runtime artifact slot {} during rollback: {error}",
                slot.display()
            )),
        },
    }
}

pub(super) async fn read_runtime_artifact_slot(path: &Path) -> Result<Option<PathBuf>, String> {
    match tokio::fs::read_link(path).await {
        Ok(target) => Ok(Some(target)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!(
            "read Runtime artifact slot {}: {error}",
            path.display()
        )),
    }
}

pub(crate) fn read_runtime_artifact_slot_under_guard(
    path: &Path,
) -> Result<Option<PathBuf>, String> {
    match std::fs::read_link(path) {
        Ok(target) => Ok(Some(target)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!(
            "read Runtime artifact slot {}: {error}",
            path.display()
        )),
    }
}
