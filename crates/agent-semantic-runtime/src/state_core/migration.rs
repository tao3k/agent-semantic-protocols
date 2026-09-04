//! State layout materialization and legacy-tree migration.

use super::identity::RepoId;
use super::identity::WorkspaceId;
use super::identity::stable_id;
use super::layout::STATE_LAYOUT_VERSION;
use super::layout::TURSO_BACKEND;
use super::migration_tree::merge_hook_event_logs;
use super::migration_tree::merge_immutable_tree;
use super::migration_tree::resolve_retired_tree_target;
use super::resolution::ResolvedState;
use crate::git::path_identity;
use serde_json::json;
use sha2::Digest;
use sha2::Sha256;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

impl ResolvedState {
    /// Materialize State Core from an async owner without blocking a Tokio worker.
    pub async fn ensure_minimal_layout_async(&self) -> Result<(), String> {
        let resolved = self.clone();
        tokio::task::spawn_blocking(move || resolved.ensure_minimal_layout())
            .await
            .map_err(|error| format!("State Core materialization task failed: {error}"))?
    }

    /// Create the minimal State Core v2 directory layout.
    pub fn ensure_minimal_layout(&self) -> Result<(), String> {
        if !self.repo.persistence.is_durable() {
            return Err(format!(
                "refusing to materialize ephemeral or standalone temporary checkout: {}",
                self.repo.checkout_root.display()
            ));
        }
        fs::create_dir_all(&self.paths.registry_dir).map_err(io_error("create registry dir"))?;
        fs::create_dir_all(&self.paths.aliases_by_display_name_dir)
            .map_err(io_error("create aliases dir"))?;
        self.ensure_project_identity_layout()?;
        self.ensure_workspace_identity_layout()?;
        self.migrate_legacy_state_tree()?;
        self.ensure_workspace_identity_layout()?;

        write_if_missing(
            &self.paths.version_file,
            format!("{STATE_LAYOUT_VERSION}\n"),
        )?;
        write_json_if_missing(
            &self.paths.state_json,
            &json!({
                "stateLayoutVersion": STATE_LAYOUT_VERSION,
                "stateHome": self.state_home,
                "registryEventsPath": self.paths.registry_events_jsonl,
                "aliasesByDisplayNamePath": self.paths.aliases_by_display_name_dir,
            }),
        )?;
        write_if_missing(&self.paths.registry_events_jsonl, String::new())?;
        Ok(())
    }

    fn ensure_project_identity_layout(&self) -> Result<(), String> {
        if self.paths.project_dir.exists() {
            if !self.paths.project_dir.is_dir() {
                return Err(format!(
                    "State Core project path is not a directory: {}",
                    self.paths.project_dir.display()
                ));
            }
            write_json_if_missing(&self.paths.project_json, &self.project_identity_json())?;
            return require_state_identity_file(&self.paths.project_json, "project");
        }
        materialize_directory_atomically(&self.paths.project_dir, |staging_dir| {
            write_json_atomically(
                &staging_dir.join("project.json"),
                &self.project_identity_json(),
            )?;
            let workspace_dir = staging_dir
                .join("workspaces")
                .join(self.workspace.workspace_id.as_str());
            fs::create_dir_all(&workspace_dir).map_err(io_error("create staged workspace dir"))?;
            self.populate_workspace_layout(&workspace_dir)
        })?;
        require_state_identity_file(&self.paths.project_json, "project")?;
        self.require_workspace_identity_layout()
    }

    fn ensure_workspace_identity_layout(&self) -> Result<(), String> {
        if self.paths.workspace_dir.exists() {
            if !self.paths.workspace_dir.is_dir() {
                return Err(format!(
                    "State Core workspace path is not a directory: {}",
                    self.paths.workspace_dir.display()
                ));
            }
            self.populate_workspace_layout(&self.paths.workspace_dir)?;
            return self.require_workspace_identity_layout();
        }
        materialize_directory_atomically(&self.paths.workspace_dir, |staging_dir| {
            self.populate_workspace_layout(staging_dir)
        })?;
        self.require_workspace_identity_layout()
    }

    fn populate_workspace_layout(&self, workspace_dir: &Path) -> Result<(), String> {
        let hooks_dir = workspace_dir.join("hooks");
        let client_dir = workspace_dir.join("live/client");
        let artifacts_dir = workspace_dir.join("artifacts");
        write_json_if_missing(
            &workspace_dir.join("workspace.json"),
            &self.workspace_identity_json(),
        )?;
        fs::create_dir_all(&hooks_dir).map_err(io_error("create hooks dir"))?;
        fs::create_dir_all(&client_dir).map_err(io_error("create client dir"))?;
        fs::create_dir_all(&artifacts_dir).map_err(io_error("create artifacts dir"))?;
        write_json_if_missing(
            &client_dir.join("manifest.json"),
            &self.client_manifest_json(),
        )
    }

    fn require_workspace_identity_layout(&self) -> Result<(), String> {
        require_state_identity_file(&self.paths.workspace_json, "workspace")?;
        require_state_identity_file(&self.paths.client_manifest_json, "client manifest")
    }

    fn project_identity_json(&self) -> serde_json::Value {
        json!({
            "stateLayoutVersion": STATE_LAYOUT_VERSION,
            "repoId": self.repo.repo_id,
            "displayName": self.repo.display_name,
            "checkoutRoot": self.repo.checkout_root,
            "gitToplevel": self.repo.git_toplevel,
            "gitDir": self.repo.git_dir,
            "gitCommonDir": self.repo.git_common_dir,
            "remoteUrl": self.repo.remote_url,
            "identityBasis": self.repo.identity_basis,
            "persistence": self.repo.persistence,
        })
    }

    fn workspace_identity_json(&self) -> serde_json::Value {
        json!({
            "stateLayoutVersion": STATE_LAYOUT_VERSION,
            "repoId": self.repo.repo_id,
            "workspaceId": self.workspace.workspace_id,
            "scopeId": self.scope_id,
            "displayName": self.workspace.display_name,
            "root": self.workspace.root,
            "gitDir": self.workspace.git_dir,
            "identityBasis": self.workspace.identity_basis,
            "lifecycle": self.workspace.lifecycle,
        })
    }

    fn client_manifest_json(&self) -> serde_json::Value {
        json!({
            "stateLayoutVersion": STATE_LAYOUT_VERSION,
            "backend": TURSO_BACKEND,
            "repoId": self.repo.repo_id,
            "workspaceId": self.workspace.workspace_id,
            "scopeId": self.scope_id,
            "dbPath": self.paths.client_db_path,
            "artifactPath": self.paths.artifacts_dir,
            "generationManifestPath": self.paths.client_cache_manifest_path,
        })
    }

    fn migrate_legacy_state_tree(&self) -> Result<(), String> {
        let candidates = self.legacy_identity_candidates();
        let populated = candidates
            .iter()
            .filter(|(repo_id, workspace_id)| {
                self.legacy_workspace_dir(repo_id, workspace_id).exists()
                    || self.legacy_hooks_dir(repo_id, workspace_id).exists()
            })
            .collect::<Vec<_>>();
        if populated.len() > 1 {
            return Err(format!(
                "state migration conflict: multiple legacy identities contain state for checkout {}",
                self.repo.checkout_root.display()
            ));
        }
        let (legacy_repo_id, legacy_workspace_id) =
            if let Some((repo_id, workspace_id)) = populated.first().copied() {
                (repo_id.clone(), workspace_id.clone())
            } else {
                let current_legacy_hooks =
                    self.legacy_hooks_dir(&self.repo.repo_id, &self.workspace.workspace_id);
                if !current_legacy_hooks.exists() {
                    return Ok(());
                }
                (
                    self.repo.repo_id.clone(),
                    self.workspace.workspace_id.clone(),
                )
            };

        let legacy_workspace_dir = self.legacy_workspace_dir(&legacy_repo_id, &legacy_workspace_id);
        let identity_changed = legacy_repo_id != self.repo.repo_id
            || legacy_workspace_id != self.workspace.workspace_id;
        let canonical_envelope_root = self
            .paths
            .workspace_dir
            .join("live/client/source-snapshot-envelopes");
        let canonical_cas_root = self
            .paths
            .workspace_dir
            .join("live/client/source-blob-cas/v1");
        normalize_source_snapshot_envelope_tree(&canonical_envelope_root, &canonical_cas_root)?;
        if identity_changed && legacy_workspace_dir.exists() {
            normalize_source_snapshot_envelope_tree(
                &legacy_workspace_dir.join("live/client/source-snapshot-envelopes"),
                &canonical_cas_root,
            )?;
            if self.paths.workspace_dir.exists() {
                merge_immutable_tree(
                    &legacy_workspace_dir.join("live/client/source-blob-cas"),
                    &self.paths.workspace_dir.join("live/client/source-blob-cas"),
                )?;
                merge_immutable_tree(
                    &legacy_workspace_dir.join("live/client/source-snapshot-envelopes"),
                    &self
                        .paths
                        .workspace_dir
                        .join("live/client/source-snapshot-envelopes"),
                )?;
                let preferred_retired_workspace_dir = self
                    .paths
                    .workspace_dir
                    .join(".state")
                    .join("migrations")
                    .join("project-identity-v1")
                    .join("retired-workspaces")
                    .join(format!(
                        "{}--{}",
                        legacy_repo_id.as_str(),
                        legacy_workspace_id.as_str()
                    ));
                let retired_workspace_dir = resolve_retired_tree_target(
                    &legacy_workspace_dir,
                    &preferred_retired_workspace_dir,
                )?;
                if retired_workspace_dir.exists() {
                    merge_immutable_tree(&legacy_workspace_dir, &retired_workspace_dir)?;
                } else {
                    write_json_atomically(
                        &legacy_workspace_dir
                            .join(".state")
                            .join("migrations")
                            .join("project-identity-v1.json"),
                        &json!({
                            "migration": "project-identity-v1",
                            "legacyRepoId": legacy_repo_id,
                            "legacyWorkspaceId": legacy_workspace_id,
                            "repoId": self.repo.repo_id,
                            "workspaceId": self.workspace.workspace_id,
                            "legacyPath": legacy_workspace_dir,
                            "canonicalPath": self.paths.workspace_dir,
                            "retiredPath": retired_workspace_dir,
                            "activeDatabaseAuthority": self.paths.client_dir,
                            "status": "retired-read-disabled",
                        }),
                    )?;
                    let retired_parent = retired_workspace_dir.parent().ok_or_else(|| {
                        format!(
                            "retired legacy workspace has no parent: {}",
                            retired_workspace_dir.display()
                        )
                    })?;
                    fs::create_dir_all(retired_parent)
                        .map_err(io_error("create retired legacy workspace parent"))?;
                    fs::rename(&legacy_workspace_dir, &retired_workspace_dir)
                        .map_err(io_error("retire legacy project workspace"))?;
                }
            } else {
                write_json_atomically(
                    &legacy_workspace_dir
                        .join(".state")
                        .join("migrations")
                        .join("project-identity-v1.json"),
                    &json!({
                        "migration": "project-identity-v1",
                        "legacyRepoId": legacy_repo_id,
                        "legacyWorkspaceId": legacy_workspace_id,
                        "repoId": self.repo.repo_id,
                        "workspaceId": self.workspace.workspace_id,
                        "legacyPath": legacy_workspace_dir,
                        "canonicalPath": self.paths.workspace_dir,
                        "activeDatabaseAuthority": self.paths.client_dir,
                        "status": "committed-by-directory-rename",
                    }),
                )?;
                let workspaces_dir = self.paths.project_dir.join("workspaces");
                fs::create_dir_all(&workspaces_dir)
                    .map_err(io_error("create canonical workspaces dir"))?;
                fs::rename(&legacy_workspace_dir, &self.paths.workspace_dir)
                    .map_err(io_error("migrate legacy project workspace"))?;
                remove_file_if_present(&self.paths.workspace_json)?;
                remove_file_if_present(&self.paths.client_manifest_json)?;
            }
            self.prune_legacy_project_dir(&legacy_repo_id)?;
        }

        let legacy_hooks_dir = self.legacy_hooks_dir(&legacy_repo_id, &legacy_workspace_id);
        if legacy_hooks_dir.exists() {
            if self.paths.hooks_dir.exists() {
                let (merged_event_count, window_digest) = merge_hook_event_logs(
                    &legacy_hooks_dir.join("state/events.jsonl"),
                    &self.paths.hooks_dir.join("state/events.jsonl"),
                )?;
                let retired_legacy_hook_state =
                    if remove_directory_tree_if_empty(&legacy_hooks_dir)? {
                        None
                    } else {
                        let preferred_retired_hooks_dir = self
                            .paths
                            .hooks_dir
                            .join(".state")
                            .join("migrations")
                            .join("project-identity-v1")
                            .join("retired-hook-state")
                            .join(format!(
                                "{}--{}",
                                legacy_repo_id.as_str(),
                                legacy_workspace_id.as_str()
                            ));
                        let retired_hooks_dir = resolve_retired_tree_target(
                            &legacy_hooks_dir,
                            &preferred_retired_hooks_dir,
                        )?;
                        if retired_hooks_dir.exists() {
                            merge_immutable_tree(&legacy_hooks_dir, &retired_hooks_dir)?;
                        } else {
                            write_json_atomically(
                                &legacy_hooks_dir
                                    .join(".state")
                                    .join("migrations")
                                    .join("hook-project-tree-v1.json"),
                                &json!({
                                    "migration": "hook-project-tree-v1",
                                    "legacyRepoId": legacy_repo_id,
                                    "legacyWorkspaceId": legacy_workspace_id,
                                    "repoId": self.repo.repo_id,
                                    "workspaceId": self.workspace.workspace_id,
                                    "legacyPath": legacy_hooks_dir,
                                    "canonicalPath": self.paths.hooks_dir,
                                    "retiredPath": retired_hooks_dir,
                                    "status": "retired-read-disabled",
                                }),
                            )?;
                            let retired_parent = retired_hooks_dir.parent().ok_or_else(|| {
                                format!(
                                    "retired legacy hook state has no parent: {}",
                                    retired_hooks_dir.display()
                                )
                            })?;
                            fs::create_dir_all(retired_parent)
                                .map_err(io_error("create retired legacy hook state parent"))?;
                            fs::rename(&legacy_hooks_dir, &retired_hooks_dir)
                                .map_err(io_error("retire legacy hook state"))?;
                        }
                        Some(retired_hooks_dir)
                    };
                let migration_status = if retired_legacy_hook_state.is_some() {
                    "merged-events-and-retired-non-event-state"
                } else {
                    "merged-by-event-identity"
                };
                write_json_atomically(
                    &self
                        .paths
                        .hooks_dir
                        .join(".state")
                        .join("migrations")
                        .join(format!("hook-event-window-{}.json", &window_digest[..16])),
                    &json!({
                        "migration": "hook-event-window-v1",
                        "legacyRepoId": legacy_repo_id,
                        "legacyWorkspaceId": legacy_workspace_id,
                        "repoId": self.repo.repo_id,
                        "workspaceId": self.workspace.workspace_id,
                        "mergedEventCount": merged_event_count,
                        "windowDigest": format!("sha256:{window_digest}"),
                        "retiredLegacyHookState": retired_legacy_hook_state,
                        "status": migration_status,
                    }),
                )?;
                prune_empty_ancestors(&legacy_hooks_dir, &self.state_home.join("hooks"));
                return Ok(());
            }
            fs::create_dir_all(&self.paths.workspace_dir)
                .map_err(io_error("create canonical workspace for hooks"))?;
            write_json_atomically(
                &legacy_hooks_dir
                    .join(".state")
                    .join("migrations")
                    .join("hook-project-tree-v1.json"),
                &json!({
                    "migration": "hook-project-tree-v1",
                    "legacyRepoId": legacy_repo_id,
                    "legacyWorkspaceId": legacy_workspace_id,
                    "repoId": self.repo.repo_id,
                    "workspaceId": self.workspace.workspace_id,
                    "legacyPath": legacy_hooks_dir,
                    "canonicalPath": self.paths.hooks_dir,
                    "status": "committed-by-directory-rename",
                }),
            )?;
            fs::rename(&legacy_hooks_dir, &self.paths.hooks_dir)
                .map_err(io_error("migrate legacy hook project tree"))?;
            prune_empty_ancestors(&legacy_hooks_dir, &self.state_home.join("hooks"));
        }
        Ok(())
    }

    fn legacy_identity_candidates(&self) -> Vec<(RepoId, WorkspaceId)> {
        let mut repo_bases = Vec::new();
        if let Some(common_git_dir) = self.repo.git_common_dir.as_deref() {
            repo_bases.push(format!("git-common-dir:{}", path_identity(common_git_dir)));
        }
        if let Some(git_dir) = self.repo.git_dir.as_deref() {
            repo_bases.push(format!("git-dir:{}", path_identity(git_dir)));
        }
        repo_bases.push(format!("path:{}", path_identity(&self.repo.checkout_root)));

        let mut candidates = Vec::new();
        for repo_basis in repo_bases {
            let repo_id = RepoId(stable_id("repo", &repo_basis));
            let workspace_basis = format!(
                "repo:{}|checkout:{}|git-dir:{}",
                repo_id.as_str(),
                path_identity(&self.workspace.root),
                self.workspace
                    .git_dir
                    .as_deref()
                    .map(path_identity)
                    .unwrap_or_else(|| "none".to_string())
            );
            let workspace_id = WorkspaceId(stable_id("workspace", &workspace_basis));
            if repo_id != self.repo.repo_id
                && !candidates.contains(&(repo_id.clone(), workspace_id.clone()))
            {
                candidates.push((repo_id, workspace_id));
            }
        }
        candidates
    }

    fn legacy_workspace_dir(&self, repo_id: &RepoId, workspace_id: &WorkspaceId) -> PathBuf {
        self.state_home
            .join("projects")
            .join("by-id")
            .join(repo_id.as_str())
            .join("workspaces")
            .join(workspace_id.as_str())
    }

    fn legacy_hooks_dir(&self, repo_id: &RepoId, workspace_id: &WorkspaceId) -> PathBuf {
        self.state_home
            .join("hooks")
            .join("projects")
            .join(repo_id.as_str())
            .join("workspaces")
            .join(workspace_id.as_str())
    }

    fn prune_legacy_project_dir(&self, legacy_repo_id: &RepoId) -> Result<(), String> {
        let project_dir = self
            .state_home
            .join("projects")
            .join("by-id")
            .join(legacy_repo_id.as_str());
        let workspaces_dir = project_dir.join("workspaces");
        if workspaces_dir.exists()
            && fs::read_dir(&workspaces_dir)
                .map_err(io_error("read legacy workspaces dir"))?
                .next()
                .is_none()
        {
            fs::remove_dir(&workspaces_dir).map_err(io_error("remove legacy workspaces dir"))?;
            remove_file_if_present(&project_dir.join("project.json"))?;
            let _ = fs::remove_dir(&project_dir);
        }
        Ok(())
    }
}

fn write_if_missing(path: &Path, content: String) -> Result<(), String> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(io_error("create parent dir"))?;
    }
    fs::write(path, content).map_err(io_error("write state file"))
}

fn require_state_identity_file(path: &Path, kind: &str) -> Result<(), String> {
    if path.is_file() {
        return Ok(());
    }
    Err(format!(
        "State Core {kind} identity was not committed: {}",
        path.display()
    ))
}

fn materialize_directory_atomically(
    target: &Path,
    populate: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<(), String> {
    let parent = target
        .parent()
        .ok_or_else(|| format!("State Core target has no parent: {}", target.display()))?;
    fs::create_dir_all(parent).map_err(io_error("create State Core target parent"))?;
    let leaf = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("State Core target has no UTF-8 leaf: {}", target.display()))?;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| format!("system clock is before Unix epoch: {error}"))?
        .as_nanos();
    static NEXT_STAGING_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let staging_id = NEXT_STAGING_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let staging = parent.join(format!(
        ".{leaf}.staging-{}-{nonce}-{staging_id}",
        std::process::id()
    ));
    fs::create_dir(&staging).map_err(io_error("create State Core staging dir"))?;
    if let Err(error) = populate(&staging) {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    match fs::rename(&staging, target) {
        Ok(()) => Ok(()),
        Err(_) if target.is_dir() => {
            let _ = fs::remove_dir_all(&staging);
            Ok(())
        }
        Err(error) => {
            let _ = fs::remove_dir_all(&staging);
            Err(format!(
                "commit State Core directory {}: {error}",
                target.display()
            ))
        }
    }
}

fn write_json_if_missing(path: &Path, value: &serde_json::Value) -> Result<(), String> {
    let content = serde_json::to_string_pretty(value)
        .map_err(|error| format!("serialize state json: {error}"))?;
    write_if_missing(path, format!("{content}\n"))
}

fn write_json_atomically(path: &Path, value: &serde_json::Value) -> Result<(), String> {
    let content = serde_json::to_string_pretty(value)
        .map_err(|error| format!("serialize state json: {error}"))?;
    let content = format!("{content}\n");
    if path.exists() {
        let existing =
            fs::read_to_string(path).map_err(io_error("read existing atomic state file"))?;
        if existing == content {
            return Ok(());
        }
        return Err(format!(
            "atomic state file conflict: existing content differs: {}",
            path.display()
        ));
    }
    let parent = path
        .parent()
        .ok_or_else(|| format!("state json path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent).map_err(io_error("create atomic state parent dir"))?;
    let temp_path = parent.join(format!(
        ".{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("state json path has no file name: {}", path.display()))?
    ));
    commit_text_atomically(&temp_path, path, &content)
}

pub(super) fn remove_file_if_present(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("remove legacy state file: {error}")),
    }
}

fn prune_empty_ancestors(start: &Path, root: &Path) {
    let mut current = start.parent().map(Path::to_path_buf);
    while let Some(path) = current {
        let parent = path.parent().map(Path::to_path_buf);
        if fs::remove_dir(&path).is_err() {
            break;
        }
        if path == root {
            break;
        }
        current = parent;
    }
}

fn remove_directory_tree_if_empty(path: &Path) -> Result<bool, String> {
    if !path.exists() {
        return Ok(true);
    }
    let entries = fs::read_dir(path).map_err(io_error("read canonical placeholder dir"))?;
    for entry in entries {
        let entry = entry.map_err(io_error("read canonical placeholder entry"))?;
        let file_type = entry
            .file_type()
            .map_err(io_error("read canonical placeholder file type"))?;
        if !file_type.is_dir() || !remove_directory_tree_if_empty(&entry.path())? {
            return Ok(false);
        }
    }
    fs::remove_dir(path).map_err(io_error("remove canonical placeholder dir"))?;
    Ok(true)
}

fn normalize_source_snapshot_envelope_tree(
    envelope_root: &Path,
    canonical_cas_root: &Path,
) -> Result<(), String> {
    let version_root = envelope_root.join("v1");
    if !version_root.exists() {
        return Ok(());
    }
    for snapshot_entry in
        fs::read_dir(&version_root).map_err(io_error("read snapshot envelope version root"))?
    {
        let snapshot_entry =
            snapshot_entry.map_err(io_error("read snapshot envelope root entry"))?;
        let snapshot_path = snapshot_entry.path();
        if !snapshot_entry
            .file_type()
            .map_err(io_error("read snapshot envelope root file type"))?
            .is_dir()
        {
            return Err(format!(
                "state migration conflict: snapshot envelope root contains non-directory entry: {}",
                snapshot_path.display()
            ));
        }
        let expected_snapshot_root = snapshot_entry.file_name().to_string_lossy().to_string();
        let envelope_entries = fs::read_dir(&snapshot_path)
            .map_err(io_error("read provider snapshot envelope directory"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(io_error("read provider snapshot envelope entry"))?;
        for envelope_entry in envelope_entries {
            let source_path = envelope_entry.path();
            if !envelope_entry
                .file_type()
                .map_err(io_error("read provider snapshot envelope file type"))?
                .is_file()
            {
                return Err(format!(
                    "state migration conflict: provider snapshot envelope contains non-file entry: {}",
                    source_path.display()
                ));
            }
            let mut envelope = serde_json::from_slice::<serde_json::Value>(
                &fs::read(&source_path).map_err(io_error("read provider snapshot envelope"))?,
            )
            .map_err(|error| {
                format!(
                    "parse provider snapshot envelope {}: {error}",
                    source_path.display()
                )
            })?;
            let provider_id = envelope
                .get("providerId")
                .and_then(serde_json::Value::as_str)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    format!(
                        "provider snapshot envelope lacks providerId: {}",
                        source_path.display()
                    )
                })?;
            let source_snapshot = envelope
                .get("sourceSnapshot")
                .and_then(serde_json::Value::as_object)
                .ok_or_else(|| {
                    format!(
                        "provider snapshot envelope lacks sourceSnapshot: {}",
                        source_path.display()
                    )
                })?;
            let snapshot_root = source_snapshot
                .get("rootDigest")
                .and_then(serde_json::Value::as_str)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    format!(
                        "provider snapshot envelope lacks rootDigest: {}",
                        source_path.display()
                    )
                })?;
            if snapshot_root != expected_snapshot_root {
                return Err(format!(
                    "provider snapshot envelope root mismatch: path={} expected={} actual={}",
                    source_path.display(),
                    expected_snapshot_root,
                    snapshot_root
                ));
            }
            let provider_digest = source_snapshot
                .get("providerDigest")
                .and_then(serde_json::Value::as_str)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    format!(
                        "provider snapshot envelope lacks providerDigest: {}",
                        source_path.display()
                    )
                })?;
            let target_path = snapshot_path.join(source_snapshot_envelope_file_name(
                provider_id,
                provider_digest,
            ));
            envelope["casRoot"] =
                serde_json::Value::String(canonical_cas_root.display().to_string());
            if source_path == target_path {
                replace_json_atomically(&target_path, &envelope)?;
                continue;
            }
            if target_path.exists() {
                let existing = serde_json::from_slice::<serde_json::Value>(
                    &fs::read(&target_path)
                        .map_err(io_error("read digest-qualified snapshot envelope"))?,
                )
                .map_err(|error| {
                    format!(
                        "parse digest-qualified snapshot envelope {}: {error}",
                        target_path.display()
                    )
                })?;
                if existing != envelope {
                    return Err(format!(
                        "state migration conflict: digest-qualified snapshot envelope differs: source={} target={}",
                        source_path.display(),
                        target_path.display()
                    ));
                }
            } else {
                replace_json_atomically(&target_path, &envelope)?;
            }
            fs::remove_file(&source_path)
                .map_err(io_error("remove flat provider snapshot envelope"))?;
        }
    }
    Ok(())
}

fn source_snapshot_envelope_file_name(provider_id: &str, provider_digest: &str) -> String {
    format!(
        "{}--{}.json",
        source_snapshot_envelope_component(provider_id),
        source_snapshot_envelope_component(provider_digest)
    )
}

fn source_snapshot_envelope_component(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn replace_json_atomically(path: &Path, value: &serde_json::Value) -> Result<(), String> {
    let content = serde_json::to_string_pretty(value)
        .map_err(|error| format!("serialize migrated snapshot envelope: {error}"))?;
    let parent = path
        .parent()
        .ok_or_else(|| format!("snapshot envelope path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent).map_err(io_error("create snapshot envelope parent"))?;
    let temporary = parent.join(format!(
        ".{}.migration-{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!(
                "snapshot envelope path has no file name: {}",
                path.display()
            ))?,
        std::process::id()
    ));
    fs::write(&temporary, format!("{content}\n"))
        .map_err(io_error("write migrated snapshot envelope"))?;
    fs::rename(&temporary, path).map_err(io_error("commit migrated snapshot envelope"))
}

pub(super) fn commit_text_atomically(
    temp_path: &Path,
    path: &Path,
    content: &str,
) -> Result<(), String> {
    use std::io::Write;

    let mut file =
        fs::File::create(temp_path).map_err(io_error("create atomic state temp file"))?;
    file.write_all(content.as_bytes())
        .map_err(io_error("write atomic state temp file"))?;
    file.sync_all()
        .map_err(io_error("sync atomic state temp file"))?;
    fs::rename(temp_path, path).map_err(io_error("commit atomic state file"))
}

pub(super) fn sha256_hex(content: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content);
    format!("{:x}", hasher.finalize())
}

pub(super) fn io_error(action: &'static str) -> impl FnOnce(std::io::Error) -> String {
    move |error| format!("{action}: {error}")
}

#[cfg(test)]
#[path = "../../tests/unit/state_core_migration_source_snapshot.rs"]
mod tests;
