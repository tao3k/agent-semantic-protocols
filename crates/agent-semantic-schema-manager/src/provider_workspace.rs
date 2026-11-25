// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Pure-Rust embeddable `providers.toml` workspace library API.
//!
//! Uses pure-Rust `gix` (gitoxide) for read-only local repository inspection
//! (`HEAD` revision and working-tree cleanliness) with zero `libgit2` C ABI
//! dependencies and zero standalone CLI binary.

use std::path::{Path, PathBuf};
use std::process::Command;

use agent_semantic_config::{
    ProviderGitRev, ProviderPackageId, ProviderPackageSpec, load_providers_manifest,
    save_providers_manifest,
};

/// Observed checkout state for one package declared in `providers.toml`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderCheckoutState {
    Missing,
    Symlinked,
    Synced,
    Drifted,
}

/// Typed status snapshot for one external provider checkout.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderCheckoutStatus {
    pub id: ProviderPackageId,
    pub path: String,
    pub locked_rev: ProviderGitRev,
    pub actual_rev: Option<String>,
    pub state: ProviderCheckoutState,
    pub dirty: bool,
}

/// Action taken while synchronizing or linking one external provider package.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderWorkspaceAction {
    SkippedSymlink,
    AlreadySynced,
    Updated,
    LinkedLocal,
}

/// Typed receipt returned for one provider package operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderSyncOutcome {
    pub id: ProviderPackageId,
    pub path: String,
    pub rev: ProviderGitRev,
    pub action: ProviderWorkspaceAction,
}

/// Inspect local checkouts declared in `providers.toml` using pure-Rust `gix`.
pub fn inspect_provider_workspace(
    workspace: &Path,
    profile: Option<&str>,
    packages: &[String],
) -> Result<Vec<ProviderCheckoutStatus>, String> {
    let manifest = load_providers_manifest(workspace).map_err(|err| err.to_string())?;
    let selected = manifest
        .select_packages(profile, packages)
        .map_err(|err| err.to_string())?;
    Ok(selected
        .into_iter()
        .map(|(id, spec)| inspect_checkout_with_gix(&workspace.join(&spec.path), id, spec))
        .collect())
}

/// Synchronize selected `providers.toml` packages into `workspace`.
pub fn sync_provider_workspace(
    workspace: &Path,
    profile: Option<&str>,
    packages: &[String],
) -> Result<Vec<ProviderSyncOutcome>, String> {
    let manifest = load_providers_manifest(workspace).map_err(|err| err.to_string())?;
    let selected = manifest
        .select_packages(profile.or(Some("core")), packages)
        .map_err(|err| err.to_string())?;
    selected
        .into_iter()
        .map(|(id, spec)| sync_single_package(workspace, id, spec))
        .collect()
}

/// Lock current checked-out `HEAD` commits (read via `gix`) back into `providers.toml`.
pub fn lock_provider_workspace(
    workspace: &Path,
    profile: Option<&str>,
    packages: &[String],
) -> Result<Vec<(ProviderPackageId, ProviderGitRev)>, String> {
    let mut manifest = load_providers_manifest(workspace).map_err(|err| err.to_string())?;
    let ids: Vec<ProviderPackageId> = manifest
        .select_packages(profile.or(Some("core")), packages)
        .map_err(|err| err.to_string())?
        .into_iter()
        .map(|(id, _)| id.clone())
        .collect();
    let mut updated = Vec::new();
    for id in ids {
        let spec = manifest
            .packages
            .get_mut(&id)
            .ok_or_else(|| format!("unknown provider package `{id}`"))?;
        let target = workspace.join(&spec.path);
        if !target.exists() {
            return Err(format!(
                "cannot lock `{id}`: checkout missing at {}",
                target.display()
            ));
        }
        let head = read_head_rev_with_gix(&target)?;
        let rev = ProviderGitRev::new(head).map_err(|err| err.to_string())?;
        if spec.rev != rev {
            spec.rev = rev.clone();
            updated.push((id, rev));
        }
    }
    save_providers_manifest(workspace, &manifest).map_err(|err| err.to_string())?;
    Ok(updated)
}

/// Symlink local `ghq` clones into `workspace` for multi-repo co-development.
pub fn link_local_providers(
    workspace: &Path,
    ghq_root: &Path,
    profile: Option<&str>,
    packages: &[String],
) -> Result<Vec<ProviderSyncOutcome>, String> {
    #[cfg(not(unix))]
    {
        let _ = (workspace, ghq_root, profile, packages);
        return Err("link_local_providers requires Unix symlinks".to_owned());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        let manifest = load_providers_manifest(workspace).map_err(|err| err.to_string())?;
        let selected = manifest
            .select_packages(profile.or(Some("core")), packages)
            .map_err(|err| err.to_string())?;
        let mut outcomes = Vec::with_capacity(selected.len());
        for (id, spec) in selected {
            let source = resolve_ghq_source(ghq_root, &spec.repo)?;
            if !source.exists() {
                return Err(format!(
                    "local ghq checkout not found for `{id}` at {}",
                    source.display()
                ));
            }
            let target = workspace.join(&spec.path);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|err| format!("create {}: {err}", parent.display()))?;
            }
            replace_existing_link_target(&target)?;
            symlink(&source, &target).map_err(|err| {
                format!(
                    "symlink {} -> {}: {err}",
                    target.display(),
                    source.display()
                )
            })?;
            outcomes.push(ProviderSyncOutcome {
                id: id.clone(),
                path: spec.path.clone(),
                rev: spec.rev.clone(),
                action: ProviderWorkspaceAction::LinkedLocal,
            });
        }
        Ok(outcomes)
    }
}

fn inspect_checkout_with_gix(
    target: &Path,
    id: &ProviderPackageId,
    spec: &ProviderPackageSpec,
) -> ProviderCheckoutStatus {
    let Ok(meta) = std::fs::symlink_metadata(target) else {
        return ProviderCheckoutStatus {
            id: id.clone(),
            path: spec.path.clone(),
            locked_rev: spec.rev.clone(),
            actual_rev: None,
            state: ProviderCheckoutState::Missing,
            dirty: false,
        };
    };
    let is_symlink = meta.file_type().is_symlink();
    let (actual_rev, dirty) = match gix::open(target) {
        Ok(repo) => {
            let rev = repo
                .head_id()
                .ok()
                .map(|head| head.detach().to_hex().to_string());
            let dirty = repo.is_dirty().unwrap_or(false);
            (rev, dirty)
        }
        Err(_) => (None, false),
    };
    let state = if is_symlink {
        ProviderCheckoutState::Symlinked
    } else if actual_rev.as_deref() == Some(spec.rev.as_str()) {
        ProviderCheckoutState::Synced
    } else {
        ProviderCheckoutState::Drifted
    };
    ProviderCheckoutStatus {
        id: id.clone(),
        path: spec.path.clone(),
        locked_rev: spec.rev.clone(),
        actual_rev,
        state,
        dirty,
    }
}

fn read_head_rev_with_gix(target: &Path) -> Result<String, String> {
    let repo = gix::open(target)
        .map_err(|err| format!("open git repository at {}: {err}", target.display()))?;
    let head = repo
        .head_id()
        .map_err(|err| format!("resolve HEAD at {}: {err}", target.display()))?;
    Ok(head.detach().to_hex().to_string())
}

fn sync_single_package(
    workspace: &Path,
    id: &ProviderPackageId,
    spec: &ProviderPackageSpec,
) -> Result<ProviderSyncOutcome, String> {
    let target = workspace.join(&spec.path);
    if std::fs::symlink_metadata(&target)
        .map(|meta| meta.file_type().is_symlink())
        .unwrap_or(false)
    {
        return Ok(ProviderSyncOutcome {
            id: id.clone(),
            path: spec.path.clone(),
            rev: spec.rev.clone(),
            action: ProviderWorkspaceAction::SkippedSymlink,
        });
    }
    if !target.exists() {
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| format!("create {}: {err}", parent.display()))?;
        }
        run_git(
            workspace,
            &[
                "clone",
                "--no-checkout",
                "--filter=blob:none",
                &spec.repo,
                &spec.path,
            ],
        )?;
    }
    let status = inspect_checkout_with_gix(&target, id, spec);
    if status.state == ProviderCheckoutState::Synced {
        return Ok(ProviderSyncOutcome {
            id: id.clone(),
            path: spec.path.clone(),
            rev: spec.rev.clone(),
            action: ProviderWorkspaceAction::AlreadySynced,
        });
    }
    if status.dirty {
        return Err(format!(
            "refusing to overwrite dirty checkout for `{id}` at {}",
            target.display()
        ));
    }
    run_git(
        &target,
        &["fetch", "--depth", "1", "origin", spec.rev.as_str()],
    )?;
    run_git(&target, &["checkout", "--detach", spec.rev.as_str()])?;
    Ok(ProviderSyncOutcome {
        id: id.clone(),
        path: spec.path.clone(),
        rev: spec.rev.clone(),
        action: ProviderWorkspaceAction::Updated,
    })
}

fn replace_existing_link_target(target: &Path) -> Result<(), String> {
    let Ok(meta) = std::fs::symlink_metadata(target) else {
        return Ok(());
    };
    if meta.file_type().is_symlink() || meta.is_file() {
        std::fs::remove_file(target).map_err(|err| format!("remove {}: {err}", target.display()))
    } else {
        std::fs::remove_dir_all(target).map_err(|err| format!("remove {}: {err}", target.display()))
    }
}

fn resolve_ghq_source(ghq_root: &Path, repo_url: &str) -> Result<PathBuf, String> {
    let trimmed = repo_url
        .strip_prefix("https://")
        .or_else(|| repo_url.strip_prefix("ssh://git@"))
        .ok_or_else(|| format!("unsupported repo URL for ghq resolution: `{repo_url}`"))?;
    let trimmed = trimmed.strip_suffix(".git").unwrap_or(trimmed);
    Ok(ghq_root.join(trimmed))
}

fn run_git(cwd: &Path, args: &[&str]) -> Result<(), String> {
    let output = Command::new("git")
        .current_dir(cwd)
        .args(args)
        .output()
        .map_err(|err| format!("spawn git {:?} in {}: {err}", args, cwd.display()))?;
    if !output.status.success() {
        return Err(format!(
            "git {:?} failed in {}: {}",
            args,
            cwd.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}
