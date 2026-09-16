// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Isolated copy-on-write workspace fixture for Live Corpus qualification.

use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

pub(in crate::command::live_corpus) struct IsolatedBenchmarkWorkspace {
    pub(in crate::command::live_corpus) path: PathBuf,
    pub(super) workspace_identity: String,
    pub(super) materialization: &'static str,
    pub(super) materialization_elapsed_micros: u64,
}

impl IsolatedBenchmarkWorkspace {
    pub(in crate::command::live_corpus) fn materialize(
        state_home: &Path,
        source: &Path,
        resource_id: &str,
        artifact_digest: &str,
        remote: &str,
    ) -> Result<Self, String> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("read benchmark workspace clock: {error}"))?
            .as_nanos();
        let path = state_home
            .join("runtime")
            .join("live-corpus")
            .join("benchmark-workspaces")
            .join(resource_id)
            .join(artifact_digest)
            .join(format!("{}-{nonce}", std::process::id()));
        let parent = path
            .parent()
            .ok_or_else(|| "isolated benchmark workspace has no parent".to_owned())?;
        std::fs::create_dir_all(parent).map_err(|error| {
            format!(
                "create isolated benchmark workspace parent {}: {error}",
                parent.display()
            )
        })?;
        let started = std::time::Instant::now();
        let materialization = clone_immutable_tree(source, &path)?;
        write_benchmark_topology_manifest(&path, resource_id, remote)?;
        let materialization_elapsed_micros =
            started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64;
        let workspace_identity =
            agent_semantic_client_db::AgentSessionRegistry::workspace_id(&path)?;
        Ok(Self {
            path,
            workspace_identity,
            materialization,
            materialization_elapsed_micros,
        })
    }

    pub(in crate::command::live_corpus) fn cleanup(self) -> Result<u64, String> {
        let started = std::time::Instant::now();
        std::fs::remove_dir_all(&self.path).map_err(|error| {
            format!(
                "remove isolated benchmark workspace {}: {error}",
                self.path.display()
            )
        })?;
        Ok(started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64)
    }
}

fn write_benchmark_topology_manifest(
    workspace: &Path,
    resource_id: &str,
    remote: &str,
) -> Result<(), String> {
    let path = workspace.join(agent_semantic_topology::PROJECT_TOPOLOGY_MANIFEST_PATH);
    let parent = path
        .parent()
        .ok_or_else(|| "Live Corpus topology manifest has no parent".to_owned())?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("create Live Corpus topology manifest directory: {error}"))?;
    let source = format!(
        "#+TITLE: Live Corpus Project Workspace\n:PROPERTIES:\n:CONTRACT_ORG: [[../../../org/contracts/project.workspace-manifest.v1.org][project.workspace-manifest.v1]]\n:END:\n\n* Project Workspace\n:PROPERTIES:\n:PROJECT_WORKSPACE_ID: {resource_id}\n:PROJECT_WORKSPACE_IDENTITY: git+{remote}#workspace/root\n:WORKSPACE_ROOT_PATH: .\n:PORTABILITY: cross-machine\n:REPOSITORY_ALIASES: []\n:END:\n"
    );
    std::fs::write(&path, source)
        .map_err(|error| format!("write Live Corpus topology manifest: {error}"))?;
    agent_semantic_topology::ProjectTopologyManifest::load_from_project_root(workspace)
        .map_err(|error| format!("admit Live Corpus topology manifest: {error}"))?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn clone_immutable_tree(source: &Path, destination: &Path) -> Result<&'static str, String> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let source = CString::new(source.as_os_str().as_bytes())
        .map_err(|_| "Live Corpus source path contains NUL".to_owned())?;
    let destination = CString::new(destination.as_os_str().as_bytes())
        .map_err(|_| "Live Corpus benchmark path contains NUL".to_owned())?;
    // SAFETY: both strings remain alive for this copy-on-write clone call.
    let result = unsafe { libc::clonefile(source.as_ptr(), destination.as_ptr(), 0) };
    if result != 0 {
        return Err(format!(
            "clone immutable Live Corpus tree with clonefile: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok("apfs-clonefile")
}

#[cfg(not(target_os = "macos"))]
fn clone_immutable_tree(source: &Path, destination: &Path) -> Result<&'static str, String> {
    hardlink_tree(source, destination)?;
    Ok("hardlink-tree")
}

#[cfg(not(target_os = "macos"))]
fn hardlink_tree(source: &Path, destination: &Path) -> Result<(), String> {
    std::fs::create_dir(destination).map_err(|error| {
        format!(
            "create isolated benchmark directory {}: {error}",
            destination.display()
        )
    })?;
    for entry in std::fs::read_dir(source).map_err(|error| {
        format!(
            "read immutable Live Corpus tree {}: {error}",
            source.display()
        )
    })? {
        clone_entry(
            &entry.map_err(|error| format!("read Live Corpus directory entry: {error}"))?,
            destination,
        )?;
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn clone_entry(entry: &std::fs::DirEntry, destination: &Path) -> Result<(), String> {
    let source_path = entry.path();
    let destination_path = destination.join(entry.file_name());
    let metadata = std::fs::symlink_metadata(&source_path)
        .map_err(|error| format!("read Live Corpus entry {}: {error}", source_path.display()))?;
    if metadata.is_dir() {
        return hardlink_tree(&source_path, &destination_path);
    }
    if metadata.file_type().is_symlink() {
        return clone_symlink(&source_path, &destination_path);
    }
    if std::fs::hard_link(&source_path, &destination_path).is_err() {
        std::fs::copy(&source_path, &destination_path)
            .map_err(|error| format!("copy Live Corpus file {}: {error}", source_path.display()))?;
    }
    Ok(())
}

#[cfg(all(not(target_os = "macos"), unix))]
fn clone_symlink(source: &Path, destination: &Path) -> Result<(), String> {
    let target = std::fs::read_link(source)
        .map_err(|error| format!("read Live Corpus symlink {}: {error}", source.display()))?;
    std::os::unix::fs::symlink(target, destination)
        .map_err(|error| format!("clone Live Corpus symlink {}: {error}", source.display()))
}

#[cfg(all(not(target_os = "macos"), windows))]
fn clone_symlink(_source: &Path, _destination: &Path) -> Result<(), String> {
    Err("Live Corpus hardlink-tree does not support Windows symlinks".to_owned())
}
