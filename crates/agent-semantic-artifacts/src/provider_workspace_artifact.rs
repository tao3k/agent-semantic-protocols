// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Content identity and immutable materialization for provider workspace artifacts.

use std::fs;
use std::path::Path;

pub fn provider_workspace_artifact_snapshot(root: &Path) -> Result<(String, usize), String> {
    let mut leaves = Vec::new();
    if root.is_file() {
        let leaf_name = root
            .file_name()
            .and_then(std::ffi::OsStr::to_str)
            .ok_or_else(|| {
                format!(
                    "provider workspace file artifact has no normalized UTF-8 leaf name: {}",
                    root.display()
                )
            })?;
        leaves.push((
            leaf_name.to_owned(),
            agent_semantic_content_identity::file_content_digest_v1(root)?,
        ));
    } else if root.is_dir() {
        collect_artifact_leaves(root, root, &mut leaves)?;
    } else {
        return Err(format!(
            "provider workspace artifact is not file or directory: {}",
            root.display()
        ));
    }
    if leaves.is_empty() {
        return Err(format!(
            "provider workspace artifact has no files: {}",
            root.display()
        ));
    }
    leaves.sort_by(|left, right| left.0.cmp(&right.0));
    let leaf_count = leaves.len();
    let snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes(leaves);
    Ok((snapshot.root_digest().to_string(), leaf_count))
}

pub fn materialize_provider_workspace_artifact(source: &Path, target: &Path) -> Result<(), String> {
    if source.is_file() {
        fs::copy(source, target)
            .map_err(|error| format!("copy provider artifact {}: {error}", source.display()))?;
        fs::set_permissions(
            target,
            fs::metadata(source)
                .map_err(|error| error.to_string())?
                .permissions(),
        )
        .map_err(|error| format!("copy provider artifact permissions: {error}"))?;
        return Ok(());
    }
    fs::create_dir(target).map_err(|error| {
        format!(
            "create provider artifact root {}: {error}",
            target.display()
        )
    })?;
    copy_artifact_directory(source, target)
}

fn collect_artifact_leaves(
    root: &Path,
    directory: &Path,
    leaves: &mut Vec<(String, String)>,
) -> Result<(), String> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| {
            format!(
                "read provider artifact directory {}: {error}",
                directory.display()
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("read provider artifact entry: {error}"))?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let link_metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("inspect provider artifact {}: {error}", path.display()))?;
        if link_metadata.file_type().is_symlink() {
            let target_metadata = fs::metadata(&path).map_err(|error| {
                format!(
                    "resolve provider artifact symlink {}: {error}",
                    path.display()
                )
            })?;
            if target_metadata.is_dir() {
                return Err(format!(
                    "provider workspace artifact contains directory symlink: {}",
                    path.display()
                ));
            }
        } else if link_metadata.is_dir() {
            collect_artifact_leaves(root, &path, leaves)?;
            continue;
        } else if !link_metadata.is_file() {
            return Err(format!(
                "provider workspace artifact contains unsupported entry: {}",
                path.display()
            ));
        }
        let relative = path
            .strip_prefix(root)
            .map_err(|error| format!("derive provider artifact relative path: {error}"))?
            .to_string_lossy()
            .into_owned();
        leaves.push((
            relative,
            agent_semantic_content_identity::file_content_digest_v1(&path)?,
        ));
    }
    Ok(())
}

fn copy_artifact_directory(source: &Path, target: &Path) -> Result<(), String> {
    let mut entries = fs::read_dir(source)
        .map_err(|error| {
            format!(
                "read provider artifact directory {}: {error}",
                source.display()
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("read provider artifact entry: {error}"))?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let link_metadata = fs::symlink_metadata(&source_path).map_err(|error| {
            format!(
                "inspect provider artifact {}: {error}",
                source_path.display()
            )
        })?;
        if link_metadata.file_type().is_symlink() {
            let target_metadata = fs::metadata(&source_path).map_err(|error| {
                format!(
                    "resolve provider artifact symlink {}: {error}",
                    source_path.display()
                )
            })?;
            if target_metadata.is_dir() {
                return Err(format!(
                    "provider workspace artifact contains directory symlink: {}",
                    source_path.display()
                ));
            }
            fs::copy(&source_path, &target_path)
                .map_err(|error| format!("copy provider artifact symlink target: {error}"))?;
            fs::set_permissions(&target_path, target_metadata.permissions())
                .map_err(|error| format!("copy provider artifact permissions: {error}"))?;
        } else if link_metadata.is_dir() {
            fs::create_dir(&target_path)
                .map_err(|error| format!("create provider artifact directory: {error}"))?;
            copy_artifact_directory(&source_path, &target_path)?;
        } else if link_metadata.is_file() {
            fs::copy(&source_path, &target_path)
                .map_err(|error| format!("copy provider artifact file: {error}"))?;
            fs::set_permissions(&target_path, link_metadata.permissions())
                .map_err(|error| format!("copy provider artifact permissions: {error}"))?;
        } else {
            return Err(format!(
                "provider workspace artifact contains unsupported entry: {}",
                source_path.display()
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/unit/provider_workspace_artifact.rs"]
mod tests;
