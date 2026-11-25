// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Typed `providers.toml` package manager commands (`sync`, `status`, `lock`, `link-local`).

use std::path::{Path, PathBuf};
use std::process::Command;

use agent_semantic_config::{
    ProviderGitRev, ProviderPackageId, ProviderPackageSpec, ProvidersManifest,
    load_providers_manifest, save_providers_manifest,
};

#[derive(Debug)]
struct ProvidersCommandArgs {
    action: String,
    workspace: PathBuf,
    profile: Option<String>,
    packages: Vec<String>,
    ghq_root: Option<PathBuf>,
}

pub fn run_providers_cli(arguments: impl IntoIterator<Item = String>) -> Result<(), String> {
    let args = parse_providers_args(arguments)?;
    let mut manifest = load_providers_manifest(&args.workspace).map_err(|err| err.to_string())?;
    match args.action.as_str() {
        "help" | "--help" | "-h" => {
            println!(
                "usage: asp-schema-manager providers <sync|status|lock|link-local> [--workspace <ROOT>] [--profile <NAME>] [--package <ID>]... [--ghq-root <DIR>]"
            );
            Ok(())
        }
        "status" => run_status(
            &args.workspace,
            &manifest,
            args.profile.as_deref(),
            &args.packages,
        ),
        "sync" => run_sync(
            &args.workspace,
            &manifest,
            args.profile.as_deref(),
            &args.packages,
        ),
        "lock" => run_lock(
            &args.workspace,
            &mut manifest,
            args.profile.as_deref(),
            &args.packages,
        ),
        "link-local" => {
            let ghq_root = args
                .ghq_root
                .ok_or_else(|| "providers link-local requires --ghq-root <DIR>".to_owned())?;
            run_link_local(
                &args.workspace,
                &manifest,
                args.profile.as_deref(),
                &args.packages,
                &ghq_root,
            )
        }
        other => Err(format!("unknown providers action: `{other}`")),
    }
}

fn parse_providers_args(
    arguments: impl IntoIterator<Item = String>,
) -> Result<ProvidersCommandArgs, String> {
    let mut iter = arguments.into_iter();
    let action = iter.next().unwrap_or_else(|| "help".to_owned());
    let mut workspace = PathBuf::from(".");
    let mut profile = None;
    let mut packages = Vec::new();
    let mut ghq_root = None;
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--workspace" => {
                workspace = PathBuf::from(
                    iter.next()
                        .ok_or_else(|| "--workspace requires a path".to_owned())?,
                );
            }
            "--profile" => {
                profile = Some(
                    iter.next()
                        .ok_or_else(|| "--profile requires a profile name".to_owned())?,
                );
            }
            "--package" => {
                packages.push(
                    iter.next()
                        .ok_or_else(|| "--package requires a package id".to_owned())?,
                );
            }
            "--ghq-root" => {
                ghq_root =
                    Some(PathBuf::from(iter.next().ok_or_else(|| {
                        "--ghq-root requires a directory".to_owned()
                    })?));
            }
            other => return Err(format!("unknown providers argument: {other}")),
        }
    }
    Ok(ProvidersCommandArgs {
        action,
        workspace,
        profile,
        packages,
        ghq_root,
    })
}

fn run_status(
    workspace: &Path,
    manifest: &ProvidersManifest,
    profile: Option<&str>,
    packages: &[String],
) -> Result<(), String> {
    let selected = manifest
        .select_packages(profile, packages)
        .map_err(|err| err.to_string())?;
    for (id, spec) in selected {
        let target = workspace.join(&spec.path);
        let (state, actual_rev, dirty) = inspect_checkout(&target, spec.rev.as_str());
        println!(
            "[provider-pkg] id={} state={} locked={} actual={} dirty={} path={}",
            id, state, spec.rev, actual_rev, dirty, spec.path
        );
    }
    Ok(())
}

fn run_sync(
    workspace: &Path,
    manifest: &ProvidersManifest,
    profile: Option<&str>,
    packages: &[String],
) -> Result<(), String> {
    let selected = manifest
        .select_packages(profile.or(Some("core")), packages)
        .map_err(|err| err.to_string())?;
    for (id, spec) in selected {
        sync_single_package(workspace, id, spec)?;
    }
    Ok(())
}

fn run_lock(
    workspace: &Path,
    manifest: &mut ProvidersManifest,
    profile: Option<&str>,
    packages: &[String],
) -> Result<(), String> {
    let ids: Vec<ProviderPackageId> = manifest
        .select_packages(profile.or(Some("core")), packages)
        .map_err(|err| err.to_string())?
        .into_iter()
        .map(|(id, _)| id.clone())
        .collect();
    let mut updated = 0usize;
    for id in ids {
        let Some(spec) = manifest.packages.get_mut(&id) else {
            continue;
        };
        let target = workspace.join(&spec.path);
        if !target.exists() {
            return Err(format!(
                "cannot lock `{id}`: checkout `{}` does not exist",
                target.display()
            ));
        }
        let head = git_stdout(&target, &["rev-parse", "HEAD"])?;
        let new_rev = ProviderGitRev::new(head).map_err(|err| err.to_string())?;
        if spec.rev != new_rev {
            println!("[provider-pkg] lock id={id} {} -> {}", spec.rev, new_rev);
            spec.rev = new_rev;
            updated += 1;
        }
    }
    if updated > 0 {
        save_providers_manifest(workspace, manifest).map_err(|err| err.to_string())?;
    }
    println!("[provider-pkg] locked={updated}");
    Ok(())
}

fn run_link_local(
    workspace: &Path,
    manifest: &ProvidersManifest,
    profile: Option<&str>,
    packages: &[String],
    ghq_root: &Path,
) -> Result<(), String> {
    let selected = manifest
        .select_packages(profile.or(Some("core")), packages)
        .map_err(|err| err.to_string())?;
    for (id, spec) in selected {
        let local_source = resolve_ghq_source(ghq_root, &spec.repo)?;
        if !local_source.exists() {
            return Err(format!(
                "local ghq checkout not found for `{id}` at {}",
                local_source.display()
            ));
        }
        let target = workspace.join(&spec.path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| format!("create {}: {err}", parent.display()))?;
        }
        if target.symlink_metadata().is_ok() {
            let meta = std::fs::symlink_metadata(&target)
                .map_err(|err| format!("stat {}: {err}", target.display()))?;
            if meta.file_type().is_symlink() {
                std::fs::remove_file(&target)
                    .map_err(|err| format!("remove symlink {}: {err}", target.display()))?;
            } else {
                return Err(format!(
                    "refusing to replace non-symlink checkout at {}; remove it manually before link-local",
                    target.display()
                ));
            }
        }
        #[cfg(unix)]
        std::os::unix::fs::symlink(&local_source, &target).map_err(|err| {
            format!(
                "symlink {} -> {}: {err}",
                target.display(),
                local_source.display()
            )
        })?;
        #[cfg(not(unix))]
        return Err("providers link-local symlinks are supported on unix hosts only".to_owned());
        println!(
            "[provider-pkg] linked id={} {} -> {}",
            id,
            spec.path,
            local_source.display()
        );
    }
    Ok(())
}

fn sync_single_package(
    workspace: &Path,
    id: &ProviderPackageId,
    spec: &ProviderPackageSpec,
) -> Result<(), String> {
    let target = workspace.join(&spec.path);
    if target.symlink_metadata().is_ok()
        && std::fs::symlink_metadata(&target)
            .map(|meta| meta.file_type().is_symlink())
            .unwrap_or(false)
    {
        println!(
            "[provider-pkg] id={} state=symlinked-skipped path={}",
            id, spec.path
        );
        return Ok(());
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
    let current = git_stdout(&target, &["rev-parse", "HEAD"]).ok();
    if current.as_deref() == Some(spec.rev.as_str()) {
        println!("[provider-pkg] id={} state=synced rev={}", id, spec.rev);
        return Ok(());
    }
    let dirty = git_stdout(&target, &["status", "--porcelain"])
        .map(|out| !out.is_empty())
        .unwrap_or(false);
    if dirty {
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
    println!("[provider-pkg] id={} state=updated rev={}", id, spec.rev);
    Ok(())
}

fn inspect_checkout(target: &Path, locked_rev: &str) -> (&'static str, String, bool) {
    let Ok(meta) = std::fs::symlink_metadata(target) else {
        return ("missing", "-".to_owned(), false);
    };
    let is_symlink = meta.file_type().is_symlink();
    let actual =
        git_stdout(target, &["rev-parse", "HEAD"]).unwrap_or_else(|_| "unknown".to_owned());
    let dirty = git_stdout(target, &["status", "--porcelain"])
        .map(|out| !out.is_empty())
        .unwrap_or(false);
    let state = if is_symlink {
        "symlinked"
    } else if actual == locked_rev {
        "synced"
    } else {
        "drifted"
    };
    (state, actual, dirty)
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

fn git_stdout(cwd: &Path, args: &[&str]) -> Result<String, String> {
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
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}
