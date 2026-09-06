// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Canonical physical State Home layout owned by Artifacts.

use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::{fs, io};

use serde::Deserialize;
use serde::Serialize;

use crate::ProjectBinding;
use crate::RuntimeStateLayout;
use crate::WorkspaceIdentity;

pub const WORKSPACE_BINDING_FILE: &str = "project-binding.json";
pub const WORKSPACE_DB_MANIFEST_FILE: &str = "db-engine.json";
pub const WORKSPACE_CACHE_MANIFEST_FILE: &str = "cache-manifest.json";

static WORKSPACE_BINDING_TRANSACTION_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateHomeLayout {
    root: PathBuf,
    catalog: PathBuf,
    workspaces: PathBuf,
    blobs: PathBuf,
    runtime: PathBuf,
    receipts: PathBuf,
    trash: PathBuf,
}

/// Typed projection of global control-plane State Home objects.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateHomeControlLayout {
    state_home: PathBuf,
}

/// Typed projection of rebuildable, content-addressed global caches.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateHomeCacheLayout {
    root: PathBuf,
}

/// Typed projection of managed global resources.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateHomeResourceLayout {
    root: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceStatePaths {
    pub root: PathBuf,
    pub facts: PathBuf,
    pub artifacts: PathBuf,
    pub observations: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaterializedWorkspaceState {
    pub binding: ProjectBinding,
    pub paths: WorkspaceStatePaths,
    pub last_observed_at_ms: u64,
    pub byte_count: u64,
}

/// One object discovered in a retired physical State Home namespace.
///
/// The object id is migration evidence only. It is never admitted as a
/// workspace, cache, or Runtime selection identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetiredStateRoot {
    pub object_id: String,
    pub root: PathBuf,
    pub checkout_root: PathBuf,
    pub last_observed_at_ms: u64,
    pub byte_count: u64,
}

/// One canonical workspace moved out of the live namespace before Catalog CAS.
///
/// The staged directory remains on the same State Home filesystem, so staging
/// and rollback are atomic renames. Catalog is the logical authority; callers
/// commit only after its generation-checked retirement succeeds.
#[derive(Debug)]
#[must_use = "a staged workspace retirement must be committed or rolled back"]
pub struct StagedWorkspaceRetirement {
    original_root: PathBuf,
    staged_root: PathBuf,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RetiredProjectEnvelope {
    repo_id: String,
    checkout_root: PathBuf,
}

impl StateHomeLayout {
    pub fn new(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref().to_path_buf();
        Self {
            catalog: root.join("catalog").join("state.turso"),
            workspaces: root.join("workspaces"),
            blobs: root.join("blobs").join("blake3-256"),
            runtime: root.join("runtime"),
            receipts: root.join("receipts"),
            trash: root.join("trash"),
            root,
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn catalog(&self) -> &Path {
        &self.catalog
    }

    pub fn workspaces(&self) -> &Path {
        &self.workspaces
    }

    pub fn blobs(&self) -> &Path {
        &self.blobs
    }

    pub fn receipts(&self) -> &Path {
        &self.receipts
    }

    pub fn trash(&self) -> &Path {
        &self.trash
    }

    pub fn control(&self) -> StateHomeControlLayout {
        StateHomeControlLayout {
            state_home: self.root.clone(),
        }
    }

    pub fn cache(&self) -> StateHomeCacheLayout {
        StateHomeCacheLayout {
            root: self.root.join("cache"),
        }
    }

    pub fn resources(&self) -> StateHomeResourceLayout {
        StateHomeResourceLayout {
            root: self.root.join("resources"),
        }
    }

    pub fn workspace(&self, identity: &WorkspaceIdentity) -> Result<WorkspaceStatePaths, String> {
        let digest = identity
            .digest
            .as_str()
            .strip_prefix("blake3-256:")
            .ok_or_else(|| "workspace digest must use canonical blake3-256".to_string())?;
        if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("workspace digest must contain 64 hexadecimal digits".to_string());
        }
        let root = self.workspaces.join(digest);
        Ok(WorkspaceStatePaths {
            facts: root.join("facts.turso"),
            artifacts: root.join("artifacts"),
            observations: root.join("observations"),
            root,
        })
    }

    pub fn runtime_state(&self) -> RuntimeStateLayout {
        RuntimeStateLayout::new(&self.root)
    }

    /// Atomically materialize the canonical workspace envelope owned by Artifacts.
    ///
    /// The binding digest, rather than a host project id or a remote URL, selects
    /// the physical workspace directory. Existing metadata must validate exactly;
    /// callers cannot silently rebind a populated directory.
    pub fn materialize_workspace(
        &self,
        binding: &ProjectBinding,
    ) -> Result<WorkspaceStatePaths, String> {
        binding.validate()?;
        let paths = self.workspace(&binding.workspace)?;
        fs::create_dir_all(&paths.artifacts).map_err(io_error("create workspace artifacts"))?;
        fs::create_dir_all(&paths.observations)
            .map_err(io_error("create workspace observations"))?;
        fs::create_dir_all(
            self.catalog
                .parent()
                .ok_or_else(|| "State Home catalog has no parent".to_string())?,
        )
        .map_err(io_error("create State Home catalog directory"))?;

        let binding_path = paths.binding_path();
        if binding_path.exists() {
            let bytes = fs::read(&binding_path).map_err(io_error("read workspace binding"))?;
            let existing = serde_json::from_slice::<ProjectBinding>(&bytes).map_err(|error| {
                format!(
                    "decode workspace binding {}: {error}",
                    binding_path.display()
                )
            })?;
            existing.validate()?;
            if existing != *binding {
                return Err(format!(
                    "workspace binding mismatch at {}",
                    binding_path.display()
                ));
            }
            return Ok(paths);
        }

        let encoded = serde_json::to_vec_pretty(binding)
            .map_err(|error| format!("encode workspace binding: {error}"))?;
        let temporary = binding_path.with_extension(format!(
            "json.tmp-{}-{}",
            std::process::id(),
            WORKSPACE_BINDING_TRANSACTION_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&temporary, encoded).map_err(io_error("write staged workspace binding"))?;
        match fs::rename(&temporary, &binding_path) {
            Ok(()) => Ok(paths),
            Err(error) if binding_path.exists() => {
                let _ = fs::remove_file(&temporary);
                let bytes =
                    fs::read(&binding_path).map_err(io_error("read raced workspace binding"))?;
                let existing =
                    serde_json::from_slice::<ProjectBinding>(&bytes).map_err(|decode| {
                        format!(
                            "decode raced workspace binding {}: {decode}",
                            binding_path.display()
                        )
                    })?;
                existing.validate()?;
                if existing == *binding {
                    Ok(paths)
                } else {
                    Err(format!(
                        "workspace binding race at {}: {error}",
                        binding_path.display()
                    ))
                }
            }
            Err(error) => {
                let _ = fs::remove_file(&temporary);
                Err(format!(
                    "publish workspace binding {}: {error}",
                    binding_path.display()
                ))
            }
        }
    }

    /// Enumerate only valid content-addressed workspace envelopes.
    pub fn materialized_workspaces(&self) -> Result<Vec<MaterializedWorkspaceState>, String> {
        let entries = match fs::read_dir(&self.workspaces) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(format!("read canonical workspaces: {error}")),
        };
        let mut workspaces = Vec::new();
        for entry in entries {
            let entry = entry.map_err(io_error("read canonical workspace entry"))?;
            if !entry
                .file_type()
                .map_err(io_error("inspect canonical workspace entry"))?
                .is_dir()
            {
                continue;
            }
            let root = entry.path();
            let digest = entry.file_name().to_string_lossy().into_owned();
            if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err(format!(
                    "non-canonical workspace directory: {}",
                    root.display()
                ));
            }
            let paths = WorkspaceStatePaths {
                facts: root.join("facts.turso"),
                artifacts: root.join("artifacts"),
                observations: root.join("observations"),
                root,
            };
            let binding_bytes = fs::read(paths.binding_path())
                .map_err(io_error("read materialized workspace binding"))?;
            let binding = serde_json::from_slice::<ProjectBinding>(&binding_bytes)
                .map_err(|error| format!("decode materialized workspace binding: {error}"))?;
            binding.validate()?;
            let expected = binding
                .workspace
                .digest
                .as_str()
                .strip_prefix("blake3-256:")
                .ok_or_else(|| "workspace binding digest is not canonical".to_string())?;
            if expected != digest {
                return Err(format!(
                    "workspace directory identity mismatch: directory={digest} binding={expected}"
                ));
            }
            let (byte_count, last_observed_at_ms) = tree_observation(&paths.root)?;
            workspaces.push(MaterializedWorkspaceState {
                binding,
                paths,
                last_observed_at_ms,
                byte_count,
            });
        }
        workspaces.sort_by(|left, right| left.paths.root.cmp(&right.paths.root));
        Ok(workspaces)
    }

    /// Enumerate the one retired project-id namespace for transactional cleanup.
    ///
    /// This is deliberately not a compatibility resolver: callers may only
    /// retire these objects. Missing or malformed observation evidence is kept
    /// fail-closed and cannot be converted into a canonical workspace binding.
    pub fn retired_state_roots(&self) -> Result<Vec<RetiredStateRoot>, String> {
        let retired_root = self.root.join("projects").join("by-id");
        let entries = match fs::read_dir(&retired_root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(format!("read retired State Home roots: {error}")),
        };
        let mut roots = Vec::new();
        for entry in entries {
            let entry = entry.map_err(io_error("read retired State Home entry"))?;
            let root = entry.path();
            let metadata = fs::symlink_metadata(&root)
                .map_err(io_error("inspect retired State Home entry"))?;
            if !metadata.is_dir() || metadata.file_type().is_symlink() {
                return Err(format!(
                    "retired State Home entry is not a directory: {}",
                    root.display()
                ));
            }
            let object_id = entry.file_name().to_string_lossy().into_owned();
            let Some(suffix) = object_id.strip_prefix("repo-") else {
                return Err(format!(
                    "retired State Home object id is invalid: {object_id}"
                ));
            };
            if suffix.len() != 16 || !suffix.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err(format!(
                    "retired State Home object id is invalid: {object_id}"
                ));
            }
            let envelope_bytes = fs::read(root.join("project.json"))
                .map_err(io_error("read retired State Home envelope"))?;
            let envelope = serde_json::from_slice::<RetiredProjectEnvelope>(&envelope_bytes)
                .map_err(|error| format!("decode retired State Home envelope: {error}"))?;
            if envelope.repo_id != object_id || !envelope.checkout_root.is_absolute() {
                return Err(format!(
                    "retired State Home envelope identity mismatch: objectId={object_id}"
                ));
            }
            let marker = match fs::read_to_string(root.join(".last-seen-ms")) {
                Ok(marker) => marker,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(format!("read retired State Home observation: {error}")),
            };
            let last_observed_at_ms = marker
                .trim()
                .parse::<u64>()
                .map_err(|error| format!("decode retired State Home observation: {error}"))?;
            let (byte_count, _) = tree_observation(&root)?;
            roots.push(RetiredStateRoot {
                object_id,
                root,
                checkout_root: envelope.checkout_root,
                last_observed_at_ms,
                byte_count,
            });
        }
        roots.sort_by(|left, right| left.object_id.cmp(&right.object_id));
        Ok(roots)
    }

    /// Atomically remove an exact retired root from the live State Home tree.
    pub fn stage_retired_state_root(
        &self,
        object: &RetiredStateRoot,
    ) -> Result<StagedWorkspaceRetirement, String> {
        let expected_root = self
            .root
            .join("projects")
            .join("by-id")
            .join(&object.object_id);
        if object.root != expected_root {
            return Err(format!(
                "retired State Home cleanup target drift: objectId={}",
                object.object_id
            ));
        }
        let observed = fs::read_to_string(object.root.join(".last-seen-ms"))
            .map_err(io_error("revalidate retired State Home observation"))?;
        if observed.trim().parse::<u64>().ok() != Some(object.last_observed_at_ms) {
            return Err(format!(
                "retired State Home observation changed: objectId={}",
                object.object_id
            ));
        }
        let retirement_root = self.trash.join("retired-state-roots");
        fs::create_dir_all(&retirement_root)
            .map_err(io_error("create retired State Home retirement root"))?;
        let staged_root = retirement_root.join(format!(
            "{}-{}-{}",
            object.object_id,
            std::process::id(),
            WORKSPACE_BINDING_TRANSACTION_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::rename(&object.root, &staged_root)
            .map_err(io_error("stage retired State Home retirement"))?;
        Ok(StagedWorkspaceRetirement {
            original_root: object.root.clone(),
            staged_root,
        })
    }

    /// Atomically move one exact workspace out of the live namespace.
    pub fn stage_workspace_retirement(
        &self,
        workspace_digest: &str,
    ) -> Result<StagedWorkspaceRetirement, String> {
        let digest = crate::blake3_content_digest::Blake3ContentDigest::parse(workspace_digest)?;
        let digest_dir = digest
            .as_str()
            .strip_prefix("blake3-256:")
            .ok_or_else(|| "workspace cleanup digest is not canonical".to_string())?;
        let root = self.workspaces.join(digest_dir);
        let paths = WorkspaceStatePaths {
            facts: root.join("facts.turso"),
            artifacts: root.join("artifacts"),
            observations: root.join("observations"),
            root,
        };
        let metadata = fs::symlink_metadata(&paths.root)
            .map_err(io_error("inspect workspace cleanup target"))?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(format!(
                "workspace cleanup target is not a canonical directory: {workspace_digest}"
            ));
        }
        let bytes = fs::read(paths.binding_path()).map_err(io_error("read cleanup binding"))?;
        let binding = serde_json::from_slice::<ProjectBinding>(&bytes)
            .map_err(|error| format!("decode cleanup binding: {error}"))?;
        binding.validate()?;
        if binding.workspace.digest.as_str() != workspace_digest {
            return Err("workspace cleanup binding mismatch".to_string());
        }
        let retirement_root = self.trash.join("workspaces");
        fs::create_dir_all(&retirement_root)
            .map_err(io_error("create workspace retirement root"))?;
        let staged_root = retirement_root.join(format!(
            "{}-{}-{}",
            digest_dir,
            std::process::id(),
            WORKSPACE_BINDING_TRANSACTION_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::rename(&paths.root, &staged_root)
            .map_err(io_error("stage canonical workspace retirement"))?;
        Ok(StagedWorkspaceRetirement {
            original_root: paths.root,
            staged_root,
        })
    }

    /// Compatibility convenience for callers that need immediate retirement.
    pub fn retire_workspace(&self, workspace_digest: &str) -> Result<(), String> {
        self.stage_workspace_retirement(workspace_digest)?.commit()
    }
}

impl StateHomeControlLayout {
    pub fn catalog(&self) -> PathBuf {
        self.state_home.join("catalog/state.turso")
    }

    pub fn hook_client_config(&self) -> PathBuf {
        self.state_home.join("control/config/hook-client.toml")
    }

    pub fn agent_registry_root(&self) -> PathBuf {
        self.state_home.join("control/config/agents")
    }

    pub fn agent_registry(&self) -> PathBuf {
        self.agent_registry_root().join("config.toml")
    }

    /// Root of the Runtime Server-owned session registry publication.
    ///
    /// Artifacts owns the physical namespace. The session-registry package
    /// owns the database filename, publication receipt, schema, and lifecycle.
    pub fn session_registry_root(&self) -> PathBuf {
        self.state_home.join("control/sessions")
    }
}

impl StateHomeCacheLayout {
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn reader_behavior(&self) -> PathBuf {
        self.root.join("reader-behavior/blake3-256")
    }
}

impl StateHomeResourceLayout {
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn org(&self) -> PathBuf {
        self.root.join("org")
    }

    pub fn live_corpus(&self) -> PathBuf {
        self.root.join("live-corpus")
    }
}

impl StagedWorkspaceRetirement {
    pub fn staged_root(&self) -> &Path {
        &self.staged_root
    }

    /// Restore the live workspace when the Catalog CAS did not commit.
    pub fn rollback(self) -> Result<(), String> {
        if self.original_root.exists() {
            return Err(format!(
                "workspace retirement rollback target already exists: {}",
                self.original_root.display()
            ));
        }
        fs::rename(&self.staged_root, &self.original_root)
            .map_err(io_error("rollback canonical workspace retirement"))
    }

    /// Reap physical bytes after the Catalog retirement committed.
    pub fn commit(self) -> Result<(), String> {
        fs::remove_dir_all(&self.staged_root)
            .map_err(io_error("reap staged canonical workspace retirement"))
    }
}

impl WorkspaceStatePaths {
    pub fn binding_path(&self) -> PathBuf {
        self.observations.join(WORKSPACE_BINDING_FILE)
    }

    pub fn db_manifest_path(&self) -> PathBuf {
        self.observations.join(WORKSPACE_DB_MANIFEST_FILE)
    }

    pub fn cache_manifest_path(&self) -> PathBuf {
        self.observations.join(WORKSPACE_CACHE_MANIFEST_FILE)
    }

    pub fn hook_root(&self) -> PathBuf {
        self.observations.join("hooks")
    }
}

fn io_error(operation: &'static str) -> impl FnOnce(io::Error) -> String {
    move |error| format!("{operation}: {error}")
}

fn tree_observation(root: &Path) -> Result<(u64, u64), String> {
    let mut pending = vec![root.to_path_buf()];
    let mut bytes = 0_u64;
    let mut latest = 0_u64;
    while let Some(path) = pending.pop() {
        let metadata = fs::symlink_metadata(&path).map_err(io_error("inspect workspace object"))?;
        if let Ok(modified) = metadata.modified()
            && let Ok(duration) = modified.duration_since(std::time::UNIX_EPOCH)
        {
            latest = latest.max(duration.as_millis().try_into().unwrap_or(u64::MAX));
        }
        if metadata.is_file() {
            bytes = bytes.saturating_add(metadata.len());
        } else if metadata.is_dir() {
            for entry in fs::read_dir(&path).map_err(io_error("walk workspace object"))? {
                pending.push(
                    entry
                        .map_err(io_error("read workspace object entry"))?
                        .path(),
                );
            }
        }
    }
    Ok((bytes, latest))
}
