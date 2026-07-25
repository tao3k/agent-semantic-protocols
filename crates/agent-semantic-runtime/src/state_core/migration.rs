//! State layout materialization and legacy-tree migration.

use super::identity::{RepoId, WorkspaceId, stable_id};
use super::layout::{STATE_LAYOUT_VERSION, TURSO_BACKEND};
use super::resolution::ResolvedState;
use crate::git::path_identity;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};

impl ResolvedState {
    /// Create the minimal State Core v2 directory layout.
    pub fn ensure_minimal_layout(&self) -> Result<(), String> {
        if !self.repo.persistence.is_durable() {
            return Err(format!(
                "refusing to materialize ephemeral non-Git search root: {}",
                self.repo.checkout_root.display()
            ));
        }
        fs::create_dir_all(&self.paths.registry_dir).map_err(io_error("create registry dir"))?;
        fs::create_dir_all(&self.paths.aliases_by_display_name_dir)
            .map_err(io_error("create aliases dir"))?;
        self.migrate_legacy_state_tree()?;
        fs::create_dir_all(&self.paths.project_dir).map_err(io_error("create project dir"))?;
        fs::create_dir_all(&self.paths.workspace_dir).map_err(io_error("create workspace dir"))?;
        fs::create_dir_all(&self.paths.hooks_dir).map_err(io_error("create hooks dir"))?;
        fs::create_dir_all(&self.paths.client_dir).map_err(io_error("create client dir"))?;
        fs::create_dir_all(&self.paths.artifacts_dir).map_err(io_error("create artifacts dir"))?;

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
        write_json_if_missing(
            &self.paths.project_json,
            &json!({
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
            }),
        )?;
        write_json_if_missing(
            &self.paths.workspace_json,
            &json!({
                "stateLayoutVersion": STATE_LAYOUT_VERSION,
                "repoId": self.repo.repo_id,
                "workspaceId": self.workspace.workspace_id,
                "scopeId": self.scope_id,
                "displayName": self.workspace.display_name,
                "root": self.workspace.root,
                "gitDir": self.workspace.git_dir,
                "identityBasis": self.workspace.identity_basis,
            }),
        )?;
        write_json_if_missing(
            &self.paths.client_manifest_json,
            &json!({
                "stateLayoutVersion": STATE_LAYOUT_VERSION,
                "backend": TURSO_BACKEND,
                "repoId": self.repo.repo_id,
                "workspaceId": self.workspace.workspace_id,
                "scopeId": self.scope_id,
                "dbPath": self.paths.client_db_path,
                "artifactPath": self.paths.artifacts_dir,
                "generationManifestPath": self.paths.client_cache_manifest_path,
            }),
        )?;
        self.touch_registry_activity()?;

        Ok(())
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
        if identity_changed && legacy_workspace_dir.exists() {
            if self.paths.workspace_dir.exists() {
                merge_immutable_tree(
                    &self.paths.workspace_dir.join("live/client/source-blob-cas"),
                    &legacy_workspace_dir.join("live/client/source-blob-cas"),
                )?;
                merge_immutable_tree(
                    &self
                        .paths
                        .workspace_dir
                        .join("live/client/source-snapshot-envelopes"),
                    &legacy_workspace_dir.join("live/client/source-snapshot-envelopes"),
                )?;
            }
            if self.paths.workspace_dir.exists()
                && !remove_directory_tree_if_empty(&self.paths.workspace_dir)?
            {
                return Err(format!(
                    "state migration conflict: both legacy and canonical workspaces exist: legacy={} canonical={}",
                    legacy_workspace_dir.display(),
                    self.paths.workspace_dir.display()
                ));
            }
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
            self.prune_legacy_project_dir(&legacy_repo_id)?;
        }

        let legacy_hooks_dir = self.legacy_hooks_dir(&legacy_repo_id, &legacy_workspace_id);
        if legacy_hooks_dir.exists() {
            if self.paths.hooks_dir.exists() {
                let (merged_event_count, window_digest) = merge_hook_event_logs(
                    &legacy_hooks_dir.join("state/events.jsonl"),
                    &self.paths.hooks_dir.join("state/events.jsonl"),
                )?;
                if !remove_directory_tree_if_empty(&legacy_hooks_dir)? {
                    return Err(format!(
                        "state migration conflict: legacy hook directory contains non-event state after canonical migration: legacy={} canonical={}",
                        legacy_hooks_dir.display(),
                        self.paths.hooks_dir.display()
                    ));
                }
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
                        "status": "merged-by-event-identity",
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

fn remove_file_if_present(path: &Path) -> Result<(), String> {
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

fn merge_immutable_tree(source: &Path, target: &Path) -> Result<(), String> {
    if !source.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(source).map_err(io_error("read immutable migration source"))? {
        let entry = entry.map_err(io_error("read immutable migration entry"))?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let file_type = entry
            .file_type()
            .map_err(io_error("read immutable migration file type"))?;
        if file_type.is_dir() {
            merge_immutable_tree(&source_path, &target_path)?;
            continue;
        }
        if !file_type.is_file() {
            return Err(format!(
                "state migration conflict: immutable tree contains non-file entry: {}",
                source_path.display()
            ));
        }
        if target_path.exists() {
            if !files_are_identical(&source_path, &target_path)? {
                return Err(format!(
                    "state migration conflict: immutable artifact differs: source={} target={}",
                    source_path.display(),
                    target_path.display()
                ));
            }
            fs::remove_file(&source_path)
                .map_err(io_error("remove duplicate immutable artifact"))?;
        } else {
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(io_error("create immutable migration target"))?;
            }
            fs::rename(&source_path, &target_path)
                .map_err(io_error("migrate immutable artifact"))?;
        }
    }
    fs::remove_dir(source).map_err(io_error("remove immutable migration source dir"))
}

fn files_are_identical(left: &Path, right: &Path) -> Result<bool, String> {
    use std::io::Read;

    let left_metadata = fs::metadata(left).map_err(io_error("read left artifact metadata"))?;
    let right_metadata = fs::metadata(right).map_err(io_error("read right artifact metadata"))?;
    if left_metadata.len() != right_metadata.len() {
        return Ok(false);
    }
    let mut left_file = fs::File::open(left).map_err(io_error("open left immutable artifact"))?;
    let mut right_file =
        fs::File::open(right).map_err(io_error("open right immutable artifact"))?;
    let mut left_buffer = [0_u8; 64 * 1024];
    let mut right_buffer = [0_u8; 64 * 1024];
    loop {
        let left_read = left_file
            .read(&mut left_buffer)
            .map_err(io_error("read left immutable artifact"))?;
        let right_read = right_file
            .read(&mut right_buffer)
            .map_err(io_error("read right immutable artifact"))?;
        if left_read != right_read || left_buffer[..left_read] != right_buffer[..right_read] {
            return Ok(false);
        }
        if left_read == 0 {
            return Ok(true);
        }
    }
}

fn merge_hook_event_logs(legacy: &Path, canonical: &Path) -> Result<(usize, String), String> {
    use std::collections::BTreeMap;

    if !legacy.exists() {
        return Ok((0, sha256_hex(b"")));
    }
    let legacy_content = fs::read(legacy).map_err(io_error("read legacy hook event window"))?;
    let window_digest = sha256_hex(&legacy_content);
    let mut events = BTreeMap::new();
    if canonical.exists() {
        collect_hook_events(canonical, &mut events)?;
    }
    let canonical_count = events.len();
    collect_hook_events(legacy, &mut events)?;
    let merged_event_count = events.len().saturating_sub(canonical_count);
    let mut content = String::new();
    for event in events.values() {
        content.push_str(
            &serde_json::to_string(event)
                .map_err(|error| format!("serialize merged hook event: {error}"))?,
        );
        content.push('\n');
    }
    write_text_atomically(canonical, &content)?;
    remove_file_if_present(legacy)?;
    Ok((merged_event_count, window_digest))
}

fn collect_hook_events(
    path: &Path,
    events: &mut std::collections::BTreeMap<(u64, String, String), serde_json::Value>,
) -> Result<(), String> {
    let content = fs::read_to_string(path).map_err(io_error("read hook event log"))?;
    for (index, line) in content.lines().enumerate() {
        let event: serde_json::Value = serde_json::from_str(line).map_err(|error| {
            format!(
                "parse hook event log {} line {}: {error}",
                path.display(),
                index + 1
            )
        })?;
        let recorded_at = event
            .get("recordedAtUnixMs")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| {
                format!(
                    "hook event missing recordedAtUnixMs: {} line {}",
                    path.display(),
                    index + 1
                )
            })?;
        let event_kind = event
            .get("event")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                format!(
                    "hook event missing event kind: {} line {}",
                    path.display(),
                    index + 1
                )
            })?
            .to_string();
        let event_identity = if let Some(tool_use_id) = event
            .pointer("/fields/toolUseId")
            .and_then(serde_json::Value::as_str)
        {
            tool_use_id.to_string()
        } else {
            let encoded = serde_json::to_vec(&event)
                .map_err(|error| format!("serialize hook event identity: {error}"))?;
            let mut hasher = Sha256::new();
            hasher.update(encoded);
            format!("event-json:{:x}", hasher.finalize())
        };
        let key = (recorded_at, event_identity, event_kind);
        if let Some(existing) = events.get(&key) {
            if existing != &event {
                return Err(format!(
                    "hook event identity conflict: {} line {}",
                    path.display(),
                    index + 1
                ));
            }
        } else {
            events.insert(key, event);
        }
    }
    Ok(())
}

fn write_text_atomically(path: &Path, content: &str) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("state text path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent).map_err(io_error("create atomic state text parent"))?;
    let temp_path = parent.join(format!(
        ".{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("state text path has no file name: {}", path.display()))?
    ));
    commit_text_atomically(&temp_path, path, content)
}

fn commit_text_atomically(temp_path: &Path, path: &Path, content: &str) -> Result<(), String> {
    use std::io::Write;

    let mut file =
        fs::File::create(temp_path).map_err(io_error("create atomic state temp file"))?;
    file.write_all(content.as_bytes())
        .map_err(io_error("write atomic state temp file"))?;
    file.sync_all()
        .map_err(io_error("sync atomic state temp file"))?;
    fs::rename(temp_path, path).map_err(io_error("commit atomic state file"))
}

fn sha256_hex(content: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content);
    format!("{:x}", hasher.finalize())
}

fn io_error(action: &'static str) -> impl FnOnce(std::io::Error) -> String {
    move |error| format!("{action}: {error}")
}
