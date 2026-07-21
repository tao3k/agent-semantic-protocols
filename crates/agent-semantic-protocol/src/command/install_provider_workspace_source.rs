use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub(super) struct WorkspaceBuildSnapshot {
    pub(super) evidence: agent_semantic_content_identity::SourceSnapshotEvidence,
    pub(super) leaves: BTreeMap<String, String>,
}

impl WorkspaceBuildSnapshot {
    pub(super) fn changed_paths(&self, next: &Self) -> Vec<String> {
        self.leaves
            .keys()
            .chain(next.leaves.keys())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .filter(|path| self.leaves.get(*path) != next.leaves.get(*path))
            .take(32)
            .cloned()
            .collect()
    }
}

#[derive(Debug)]
pub(super) struct WorkspaceBuildSandbox {
    pub(super) root: PathBuf,
}

impl WorkspaceBuildSandbox {
    pub(super) fn persist(mut self) -> PathBuf {
        std::mem::take(&mut self.root)
    }
}

impl Drop for WorkspaceBuildSandbox {
    fn drop(&mut self) {
        if !self.root.as_os_str().is_empty() {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

pub(super) fn capture_workspace_build_snapshot(
    project_root: &Path,
    derived_paths: &[PathBuf],
    provider_digest: &str,
) -> Result<WorkspaceBuildSnapshot, String> {
    let mut walker = ignore::WalkBuilder::new(project_root);
    let derived_paths = derived_paths.to_vec();
    walker
        .hidden(false)
        .ignore(true)
        .git_ignore(true)
        .git_global(false)
        .git_exclude(true)
        .parents(true)
        .follow_links(false)
        .filter_entry(move |entry| {
            entry.file_name() != ".git"
                && !derived_paths
                    .iter()
                    .any(|derived| entry.path().starts_with(derived))
        });
    let mut file_hashes = Vec::new();
    for entry in walker.build() {
        let entry = entry.map_err(|error| {
            format!(
                "failed to walk workspace source snapshot under {}: {error}",
                project_root.display()
            )
        })?;
        if !entry
            .file_type()
            .is_some_and(|file_type| file_type.is_file())
        {
            continue;
        }
        let relative = entry.path().strip_prefix(project_root).map_err(|error| {
            format!(
                "failed to normalize workspace snapshot path {}: {error}",
                entry.path().display()
            )
        })?;
        let normalized = relative.to_string_lossy().replace('\\', "/");
        let bytes = fs::read(entry.path()).map_err(|error| {
            format!(
                "failed to read workspace snapshot leaf {}: {error}",
                entry.path().display()
            )
        })?;
        file_hashes.push((
            normalized,
            agent_semantic_content_identity::hash_blob(&bytes).value,
        ));
    }
    file_hashes.sort_by(|left, right| left.0.cmp(&right.0));
    let leaves = file_hashes.into_iter().collect::<BTreeMap<_, _>>();
    let snapshot =
        agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes(leaves.clone());
    Ok(WorkspaceBuildSnapshot {
        evidence: snapshot.evidence(
            agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
            provider_digest.to_string(),
        ),
        leaves,
    })
}

pub(super) fn remove_workspace_snapshot_tree(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(path)
            .map_err(|error| format!("failed to remove {}: {error}", path.display())),
        Ok(_) => fs::remove_file(path)
            .map_err(|error| format!("failed to remove {}: {error}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("failed to inspect {}: {error}", path.display())),
    }
}

pub(super) fn copy_workspace_snapshot_leaves(
    source_root: &Path,
    destination_root: &Path,
    snapshot: &WorkspaceBuildSnapshot,
) -> Result<(), String> {
    remove_workspace_snapshot_tree(destination_root)?;
    fs::create_dir_all(destination_root).map_err(|error| {
        format!(
            "failed to create workspace snapshot destination {}: {error}",
            destination_root.display()
        )
    })?;
    for (relative, expected_digest) in &snapshot.leaves {
        let source = source_root.join(relative);
        let destination = destination_root.join(relative);
        let parent = destination.parent().ok_or_else(|| {
            format!(
                "workspace snapshot destination has no parent: {}",
                destination.display()
            )
        })?;
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        fs::copy(&source, &destination).map_err(|error| {
            format!(
                "failed to materialize workspace snapshot leaf {} from {}: {error}",
                relative,
                source.display()
            )
        })?;
        let bytes = fs::read(&destination).map_err(|error| {
            format!(
                "failed to verify workspace snapshot leaf {}: {error}",
                destination.display()
            )
        })?;
        let actual_digest = agent_semantic_content_identity::hash_blob(&bytes).value;
        if &actual_digest != expected_digest {
            return Err(format!(
                "workspace source changed before snapshot materialization: path={relative} expectedDigest={expected_digest} actualDigest={actual_digest}; retry from a new WorkspaceSnapshot"
            ));
        }
    }
    Ok(())
}

pub(super) fn materialize_workspace_source_cas(
    state: &agent_semantic_runtime::ProjectRuntimeState,
    project_root: &Path,
    snapshot: &WorkspaceBuildSnapshot,
) -> Result<PathBuf, String> {
    let cas_parent = state
        .protocol_home
        .join("runtime")
        .join("provider-source-snapshots");
    fs::create_dir_all(&cas_parent)
        .map_err(|error| format!("failed to create {}: {error}", cas_parent.display()))?;
    let cas_root = cas_parent.join(&snapshot.evidence.root_digest);
    if cas_root.is_dir() {
        let cached =
            capture_workspace_build_snapshot(&cas_root, &[], &snapshot.evidence.provider_digest)?;
        if cached.evidence.root_digest == snapshot.evidence.root_digest
            && cached.evidence.leaf_count == snapshot.evidence.leaf_count
        {
            return Ok(cas_root);
        }
        remove_workspace_snapshot_tree(&cas_root)?;
    }

    let staging = cas_parent.join(format!(
        ".{}.tmp-{}-{}",
        snapshot.evidence.root_digest,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| format!("system time before unix epoch: {error}"))?
            .as_nanos()
    ));
    copy_workspace_snapshot_leaves(project_root, &staging, snapshot)?;
    let staged =
        capture_workspace_build_snapshot(&staging, &[], &snapshot.evidence.provider_digest)?;
    if staged.evidence.root_digest != snapshot.evidence.root_digest
        || staged.evidence.leaf_count != snapshot.evidence.leaf_count
    {
        let _ = remove_workspace_snapshot_tree(&staging);
        return Err(format!(
            "workspace source CAS verification failed: expectedRoot={} actualRoot={} expectedLeafCount={} actualLeafCount={}",
            snapshot.evidence.root_digest,
            staged.evidence.root_digest,
            snapshot.evidence.leaf_count,
            staged.evidence.leaf_count
        ));
    }
    match fs::rename(&staging, &cas_root) {
        Ok(()) => {}
        Err(_error) if cas_root.is_dir() => {
            remove_workspace_snapshot_tree(&staging)?;
        }
        Err(error) => {
            let _ = remove_workspace_snapshot_tree(&staging);
            return Err(format!(
                "failed to publish workspace source CAS {}: {error}",
                cas_root.display()
            ));
        }
    }
    Ok(cas_root)
}

pub(super) fn materialize_workspace_build_sandbox(
    state: &agent_semantic_runtime::ProjectRuntimeState,
    provider_id: &str,
    build_recipe_digest: &str,
    source_cas_root: &Path,
    snapshot: &WorkspaceBuildSnapshot,
) -> Result<WorkspaceBuildSandbox, String> {
    let parent = state
        .protocol_home
        .join("runtime")
        .join("provider-builds")
        .join(provider_id);
    fs::create_dir_all(&parent)
        .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    let root = parent.join(format!(
        "{}-{}-{}-{}",
        snapshot.evidence.root_digest,
        build_recipe_digest,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| format!("system time before unix epoch: {error}"))?
            .as_nanos()
    ));
    copy_workspace_snapshot_leaves(source_cas_root, &root, snapshot)?;
    Ok(WorkspaceBuildSandbox { root })
}
