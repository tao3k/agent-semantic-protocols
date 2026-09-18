// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

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

/// One physical entry that is outside the closed V1 State Home contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NonContractStateHomeEntry {
    pub relative_path: PathBuf,
    pub root: PathBuf,
    pub last_observed_at_ms: u64,
    pub byte_count: u64,
    is_directory: bool,
}

/// Physical trash removed by an explicit State Home convergence operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateHomeTrashCleanup {
    pub relative_paths: Vec<PathBuf>,
    pub byte_count: u64,
}

/// One State Home object moved out of the live namespace before Catalog CAS.
///
/// The staged directory remains on the same State Home filesystem, so staging
/// and rollback are atomic renames. Catalog is the logical authority; callers
/// commit only after its generation-checked removal succeeds.
#[derive(Debug)]
#[must_use = "a staged State Home removal must be committed or rolled back"]
pub struct StagedStateHomeRemoval {
    original_root: PathBuf,
    staged_root: PathBuf,
    is_directory: bool,
}

impl StateHomeLayout {
    const V1_ROOT_ENTRIES: &'static [&'static str] = &[
        "blobs",
        "cache",
        "catalog",
        "control",
        "receipts",
        "resources",
        "runtime",
        "trash",
        "workspaces",
    ];
    const V1_RUNTIME_ENTRIES: &'static [&'static str] = &["artifacts", "bin", "serving"];
    const V1_CONTROL_ENTRIES: &'static [&'static str] = &["config", "sessions"];
    const V1_CONFIG_ENTRIES: &'static [&'static str] = &["agents", "asp.toml", "hook-client.toml"];

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

    /// Resolve the one durable State Home selected at the process boundary.
    pub fn from_process_environment() -> Result<Self, String> {
        let root = match std::env::var_os("ASP_STATE_HOME") {
            Some(value) if value.is_empty() => {
                return Err("ASP_STATE_HOME is set but empty".to_owned());
            }
            Some(value) => PathBuf::from(value),
            None => {
                let home = std::env::var_os("HOME").ok_or_else(|| "HOME is not set".to_owned())?;
                if home.is_empty() {
                    return Err("HOME is set but empty".to_owned());
                }
                PathBuf::from(home).join(".agent-semantic-protocols")
            }
        };
        Ok(Self::new(root))
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

    /// Remove every previously staged object from the non-authoritative trash root.
    ///
    /// Trash never participates in identity, rollback selection, or Runtime
    /// admission. A successful explicit sync therefore leaves it empty instead
    /// of preserving a historical namespace indefinitely.
    pub fn purge_trash(&self) -> Result<StateHomeTrashCleanup, String> {
        let children = match fs::read_dir(&self.trash) {
            Ok(children) => children,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(StateHomeTrashCleanup {
                    relative_paths: Vec::new(),
                    byte_count: 0,
                });
            }
            Err(error) => return Err(format!("read State Home trash: {error}")),
        };
        let mut entries = Vec::new();
        for child in children {
            let child = child.map_err(io_error("read State Home trash entry"))?;
            let path = child.path();
            let metadata =
                fs::symlink_metadata(&path).map_err(io_error("inspect State Home trash entry"))?;
            let (byte_count, _) = tree_observation(&path)?;
            entries.push((child.file_name(), path, metadata, byte_count));
        }
        entries.sort_by(|left, right| left.0.cmp(&right.0));
        let mut relative_paths = Vec::with_capacity(entries.len());
        let mut byte_count = 0_u64;
        for (name, path, metadata, bytes) in entries {
            if metadata.is_dir() && !metadata.file_type().is_symlink() {
                remove_contract_staging_tree(&path)?;
            } else {
                fs::remove_file(&path).map_err(io_error("purge State Home trash entry"))?;
            }
            relative_paths.push(Path::new("trash").join(name));
            byte_count = byte_count.saturating_add(bytes);
        }
        Ok(StateHomeTrashCleanup {
            relative_paths,
            byte_count,
        })
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

    /// Discover physical entries outside the closed V1 contract.
    ///
    /// This is a positive allowlist check, not a migration or legacy decoder.
    pub fn non_contract_entries(&self) -> Result<Vec<NonContractStateHomeEntry>, String> {
        let mut entries =
            self.non_contract_children(&self.root, Path::new(""), Self::V1_ROOT_ENTRIES)?;
        let runtime = self.root.join("runtime");
        if runtime.is_dir() {
            entries.extend(self.non_contract_children(
                &runtime,
                Path::new("runtime"),
                Self::V1_RUNTIME_ENTRIES,
            )?);
        }
        let control = self.root.join("control");
        if control.is_dir() {
            entries.extend(self.non_contract_children(
                &control,
                Path::new("control"),
                Self::V1_CONTROL_ENTRIES,
            )?);
        }
        let config = control.join("config");
        if config.is_dir() {
            entries.extend(self.non_contract_children(
                &config,
                Path::new("control/config"),
                Self::V1_CONFIG_ENTRIES,
            )?);
        }
        entries.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        Ok(entries)
    }

    fn non_contract_children(
        &self,
        parent: &Path,
        relative_parent: &Path,
        allowed: &[&str],
    ) -> Result<Vec<NonContractStateHomeEntry>, String> {
        let children = match fs::read_dir(parent) {
            Ok(children) => children,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(format!("read State Home contract root: {error}")),
        };
        let mut entries = Vec::new();
        for child in children {
            let child = child.map_err(io_error("read State Home contract entry"))?;
            let name = child.file_name();
            let Some(name_str) = name.to_str() else {
                return Err("State Home contract entry name is not UTF-8".to_owned());
            };
            if allowed.contains(&name_str) {
                continue;
            }
            let root = child.path();
            let metadata = fs::symlink_metadata(&root)
                .map_err(io_error("inspect non-contract State Home entry"))?;
            let (byte_count, last_observed_at_ms) = tree_observation(&root)?;
            entries.push(NonContractStateHomeEntry {
                relative_path: relative_parent.join(name),
                root,
                last_observed_at_ms,
                byte_count,
                is_directory: metadata.is_dir() && !metadata.file_type().is_symlink(),
            });
        }
        Ok(entries)
    }

    /// Atomically stage one observed contract violation for removal.
    pub fn stage_non_contract_entry(
        &self,
        entry: &NonContractStateHomeEntry,
    ) -> Result<StagedStateHomeRemoval, String> {
        let expected_root = self.root.join(&entry.relative_path);
        if expected_root != entry.root || !self.path_is_outside_v1_contract(&entry.relative_path) {
            return Err(format!(
                "State Home contract cleanup target drift: {}",
                entry.relative_path.display()
            ));
        }
        let metadata = fs::symlink_metadata(&entry.root)
            .map_err(io_error("revalidate non-contract State Home entry"))?;
        let is_directory = metadata.is_dir() && !metadata.file_type().is_symlink();
        let (byte_count, last_observed_at_ms) = tree_observation(&entry.root)?;
        if is_directory != entry.is_directory
            || byte_count != entry.byte_count
            || last_observed_at_ms != entry.last_observed_at_ms
        {
            return Err(format!(
                "State Home contract entry changed before cleanup: {}",
                entry.relative_path.display()
            ));
        }
        let staging_root = self.trash.join("contract-convergence");
        fs::create_dir_all(&staging_root)
            .map_err(io_error("create State Home convergence staging root"))?;
        let staged_root = staging_root.join(format!(
            "entry-{}-{}",
            std::process::id(),
            WORKSPACE_BINDING_TRANSACTION_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::rename(&entry.root, &staged_root)
            .map_err(io_error("stage non-contract State Home entry"))?;
        Ok(StagedStateHomeRemoval {
            original_root: entry.root.clone(),
            staged_root,
            is_directory,
        })
    }

    fn path_is_outside_v1_contract(&self, relative_path: &Path) -> bool {
        let mut components = relative_path.components();
        let Some(first) = components
            .next()
            .and_then(|component| component.as_os_str().to_str())
        else {
            return false;
        };
        if !Self::V1_ROOT_ENTRIES.contains(&first) {
            return components.next().is_none();
        }
        let Some(second) = components
            .next()
            .and_then(|component| component.as_os_str().to_str())
        else {
            return false;
        };
        if first == "runtime" {
            return components.next().is_none() && !Self::V1_RUNTIME_ENTRIES.contains(&second);
        }
        if first != "control" {
            return false;
        }
        if !Self::V1_CONTROL_ENTRIES.contains(&second) {
            return components.next().is_none();
        }
        if second != "config" {
            return false;
        }
        let Some(third) = components
            .next()
            .and_then(|component| component.as_os_str().to_str())
        else {
            return false;
        };
        components.next().is_none() && !Self::V1_CONFIG_ENTRIES.contains(&third)
    }

    /// Atomically move one exact workspace out of the live namespace.
    pub fn stage_workspace_removal(
        &self,
        workspace_digest: &str,
    ) -> Result<StagedStateHomeRemoval, String> {
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
        let staging_root = self.trash.join("workspaces");
        fs::create_dir_all(&staging_root)
            .map_err(io_error("create workspace removal staging root"))?;
        let staged_root = staging_root.join(format!(
            "{}-{}-{}",
            digest_dir,
            std::process::id(),
            WORKSPACE_BINDING_TRANSACTION_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::rename(&paths.root, &staged_root)
            .map_err(io_error("stage canonical workspace removal"))?;
        Ok(StagedStateHomeRemoval {
            original_root: paths.root,
            staged_root,
            is_directory: true,
        })
    }
}

impl StateHomeControlLayout {
    pub fn catalog(&self) -> PathBuf {
        self.state_home.join("catalog/state.turso")
    }

    pub fn hook_client_config(&self) -> PathBuf {
        self.state_home.join("control/config/hook-client.toml")
    }

    pub fn asp_config(&self) -> PathBuf {
        self.state_home.join("control/config/asp.toml")
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

impl StagedStateHomeRemoval {
    pub fn staged_root(&self) -> &Path {
        &self.staged_root
    }

    /// Restore the live object when its authority CAS did not commit.
    pub fn rollback(self) -> Result<(), String> {
        if self.original_root.exists() {
            return Err(format!(
                "State Home removal rollback target already exists: {}",
                self.original_root.display()
            ));
        }
        fs::rename(&self.staged_root, &self.original_root)
            .map_err(io_error("rollback staged State Home removal"))
    }

    /// Reap physical bytes after the authority commit succeeded.
    pub fn commit(self) -> Result<(), String> {
        if self.is_directory {
            remove_contract_staging_tree(&self.staged_root)
        } else {
            fs::remove_file(&self.staged_root).map_err(io_error("reap staged State Home removal"))
        }
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

/// Delete one tree only after it has been atomically moved below State Home
/// trash. Old tool outputs may contain read-only directories; those mode bits
/// cannot be allowed to preserve an obsolete authority namespace forever.
/// Symlinks are never followed.
fn remove_contract_staging_tree(root: &Path) -> Result<(), String> {
    let metadata =
        fs::symlink_metadata(root).map_err(io_error("inspect staged State Home removal"))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return fs::remove_file(root).map_err(io_error("reap staged State Home removal"));
    }
    make_contract_staging_directory_removable(root, metadata.permissions())?;
    for entry in fs::read_dir(root).map_err(io_error("read staged State Home removal"))? {
        let path = entry
            .map_err(io_error("read staged State Home removal entry"))?
            .path();
        let metadata =
            fs::symlink_metadata(&path).map_err(io_error("inspect staged State Home entry"))?;
        if metadata.is_dir() && !metadata.file_type().is_symlink() {
            remove_contract_staging_tree(&path)?;
        } else {
            fs::remove_file(&path).map_err(io_error("reap staged State Home entry"))?;
        }
    }
    fs::remove_dir(root).map_err(io_error("reap staged State Home removal"))
}

fn make_contract_staging_directory_removable(
    path: &Path,
    mut permissions: fs::Permissions,
) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        permissions.set_mode(permissions.mode() | 0o700);
    }
    #[cfg(not(unix))]
    permissions.set_readonly(false);
    fs::set_permissions(path, permissions)
        .map_err(io_error("make staged State Home directory removable"))
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
