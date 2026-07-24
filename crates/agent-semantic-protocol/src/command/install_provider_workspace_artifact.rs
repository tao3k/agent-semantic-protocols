use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::Path};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WorkspaceArtifactSpec {
    pub(super) root: String,
    pub(super) entrypoint: String,
    #[serde(default)]
    pub(super) launch: Option<WorkspaceArtifactLaunchSpec>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WorkspaceArtifactLaunchSpec {
    pub(super) program: String,
    #[serde(default)]
    pub(super) args: Vec<String>,
    #[serde(default)]
    pub(super) program_relative_to_artifact: bool,
    #[serde(default)]
    pub(super) args_relative_to_artifact: bool,
}

#[derive(Debug)]
pub(super) struct WorkspaceArtifactSnapshot {
    pub(super) root_digest: String,
    pub(super) leaf_count: usize,
}

pub(super) fn capture_workspace_artifact_snapshot(
    root: &Path,
) -> Result<WorkspaceArtifactSnapshot, String> {
    if !root.exists() && fs::symlink_metadata(root).is_err() {
        return Err(format!(
            "workspace artifact root is missing at {}",
            root.display()
        ));
    }
    let mut leaves = BTreeMap::new();
    collect_workspace_artifact_leaves(root, root, &mut leaves)?;
    let snapshot =
        agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes(leaves.clone());
    let root_digest = snapshot
        .evidence(
            agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
            "workspace-provider-artifact".to_string(),
        )
        .root_digest;
    Ok(WorkspaceArtifactSnapshot {
        root_digest,
        leaf_count: leaves.len(),
    })
}

fn collect_workspace_artifact_leaves(
    artifact_root: &Path,
    path: &Path,
    leaves: &mut BTreeMap<String, String>,
) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        format!(
            "failed to inspect workspace artifact leaf {}: {error}",
            path.display()
        )
    })?;
    let relative = path.strip_prefix(artifact_root).map_err(|error| {
        format!(
            "failed to normalize workspace artifact leaf {}: {error}",
            path.display()
        )
    })?;
    let key = if relative.as_os_str().is_empty() {
        ".".to_string()
    } else {
        relative.to_string_lossy().replace('\\', "/")
    };
    leaves.insert(key, workspace_artifact_leaf_digest(path, &metadata)?);
    if metadata.file_type().is_dir() {
        let mut children = fs::read_dir(path)
            .map_err(|error| {
                format!(
                    "failed to read workspace artifact directory {}: {error}",
                    path.display()
                )
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| {
                format!(
                    "failed to enumerate workspace artifact directory {}: {error}",
                    path.display()
                )
            })?;
        children.sort_by_key(std::fs::DirEntry::file_name);
        for child in children {
            collect_workspace_artifact_leaves(artifact_root, &child.path(), leaves)?;
        }
    }
    Ok(())
}

fn workspace_artifact_leaf_digest(path: &Path, metadata: &fs::Metadata) -> Result<String, String> {
    let mut payload = b"asp.workspace-provider-artifact-leaf.v1\0".to_vec();
    if metadata.file_type().is_symlink() {
        payload.extend_from_slice(b"symlink\0");
        let target = fs::read_link(path).map_err(|error| {
            format!(
                "failed to read workspace artifact symlink {}: {error}",
                path.display()
            )
        })?;
        payload.extend_from_slice(target.to_string_lossy().as_bytes());
    } else if metadata.file_type().is_dir() {
        payload.extend_from_slice(b"directory\0");
        append_workspace_artifact_permissions(&mut payload, metadata);
    } else if metadata.file_type().is_file() {
        payload.extend_from_slice(b"file\0");
        append_workspace_artifact_permissions(&mut payload, metadata);
        payload.extend_from_slice(&fs::read(path).map_err(|error| {
            format!(
                "failed to read workspace artifact file {}: {error}",
                path.display()
            )
        })?);
    } else {
        return Err(format!(
            "workspace artifact contains unsupported leaf type at {}",
            path.display()
        ));
    }
    Ok(agent_semantic_content_identity::hash_blob(&payload).value)
}

fn append_workspace_artifact_permissions(payload: &mut Vec<u8>, metadata: &fs::Metadata) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        payload.extend_from_slice(format!("{:o}\0", metadata.permissions().mode()).as_bytes());
    }
    #[cfg(not(unix))]
    {
        payload.extend_from_slice(if metadata.permissions().readonly() {
            b"readonly\0"
        } else {
            b"writable\0"
        });
    }
}

pub(super) fn copy_workspace_artifact_tree(source: &Path, target: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(source).map_err(|error| {
        format!(
            "failed to inspect workspace artifact {}: {error}",
            source.display()
        )
    })?;
    if metadata.file_type().is_symlink() {
        let link_target = fs::read_link(source).map_err(|error| {
            format!(
                "failed to read workspace artifact symlink {}: {error}",
                source.display()
            )
        })?;
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        }
        create_workspace_artifact_symlink(&link_target, target, source)?;
        return Ok(());
    }
    if metadata.file_type().is_dir() {
        fs::create_dir(target)
            .map_err(|error| format!("failed to create {}: {error}", target.display()))?;
        let mut children = fs::read_dir(source)
            .map_err(|error| format!("failed to read {}: {error}", source.display()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("failed to enumerate {}: {error}", source.display()))?;
        children.sort_by_key(std::fs::DirEntry::file_name);
        for child in children {
            copy_workspace_artifact_tree(&child.path(), &target.join(child.file_name()))?;
        }
        fs::set_permissions(target, metadata.permissions()).map_err(|error| {
            format!("failed to set permissions on {}: {error}", target.display())
        })?;
        return Ok(());
    }
    if metadata.file_type().is_file() {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        }
        fs::copy(source, target).map_err(|error| {
            format!(
                "failed to copy workspace artifact {} to {}: {error}",
                source.display(),
                target.display()
            )
        })?;
        fs::set_permissions(target, metadata.permissions()).map_err(|error| {
            format!("failed to set permissions on {}: {error}", target.display())
        })?;
        return Ok(());
    }
    Err(format!(
        "workspace artifact contains unsupported leaf type at {}",
        source.display()
    ))
}

#[cfg(unix)]
fn create_workspace_artifact_symlink(
    link_target: &Path,
    target: &Path,
    _source: &Path,
) -> Result<(), String> {
    std::os::unix::fs::symlink(link_target, target).map_err(|error| {
        format!(
            "failed to copy workspace artifact symlink {} -> {}: {error}",
            target.display(),
            link_target.display()
        )
    })
}

#[cfg(windows)]
fn create_workspace_artifact_symlink(
    link_target: &Path,
    target: &Path,
    source: &Path,
) -> Result<(), String> {
    let resolved = source
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(link_target);
    let result = if resolved.is_dir() {
        std::os::windows::fs::symlink_dir(link_target, target)
    } else {
        std::os::windows::fs::symlink_file(link_target, target)
    };
    result.map_err(|error| {
        format!(
            "failed to copy workspace artifact symlink {} -> {}: {error}",
            target.display(),
            link_target.display()
        )
    })
}

pub(super) fn remove_workspace_artifact_path(path: &Path) -> Result<(), String> {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return Ok(());
    };
    if metadata.file_type().is_dir() && !metadata.file_type().is_symlink() {
        fs::remove_dir_all(path)
            .map_err(|error| format!("failed to remove {}: {error}", path.display()))
    } else {
        fs::remove_file(path)
            .map_err(|error| format!("failed to remove {}: {error}", path.display()))
    }
}
