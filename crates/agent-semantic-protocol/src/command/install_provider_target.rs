//! Provider install target resolution for language harness binaries.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct ProviderBinaryInstallTarget {
    pub(super) path: PathBuf,
    pub(super) source: &'static str,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct ProviderBinaryInvocation {
    pub(super) command: String,
    pub(super) source: &'static str,
}

pub(super) fn resolve_provider_binary_install_target(
    configured_bin: Option<&str>,
    language_id: &str,
    provider_binary: &str,
    project_root: &Path,
    home_dir: Option<&Path>,
    path_dirs: &[PathBuf],
) -> Result<ProviderBinaryInstallTarget, String> {
    if let Some(configured_bin) = configured_bin {
        return resolve_configured_provider_binary_install_target(
            configured_bin,
            project_root,
            home_dir,
            path_dirs,
        );
    }
    if let Some(home_dir) = home_dir {
        return Ok(ProviderBinaryInstallTarget {
            path: home_dir.join(".local/bin").join(provider_binary),
            source: "home-local-bin",
        });
    }
    if let Some(existing) = provider_binary_on_path(provider_binary, path_dirs) {
        return Ok(ProviderBinaryInstallTarget {
            path: existing,
            source: "path-existing",
        });
    }
    if let Some(writable_dir) = first_writable_path_dir(path_dirs) {
        return Ok(ProviderBinaryInstallTarget {
            path: writable_dir.join(provider_binary),
            source: "path-writable",
        });
    }
    Err(format!(
        "no install target for provider binary `{provider_binary}`; set [languages.{language_id}].bin in asp.toml, set HOME, or put a writable directory on PATH"
    ))
}

pub(super) fn resolve_provider_binary_invocation(
    configured_bin: Option<&str>,
    provider_binary: &str,
    project_root: &Path,
    home_dir: Option<&Path>,
    path_dirs: &[PathBuf],
) -> Result<Option<ProviderBinaryInvocation>, String> {
    if let Some(configured_bin) = configured_bin {
        return resolve_configured_provider_binary_invocation(
            configured_bin,
            project_root,
            home_dir,
            path_dirs,
        )
        .map(Some);
    }
    if let Some(home_bin) = home_local_bin(provider_binary, home_dir)
        && home_bin.is_file()
    {
        return Ok(Some(ProviderBinaryInvocation {
            command: home_bin.to_string_lossy().to_string(),
            source: "home-local-bin",
        }));
    }
    if let Some(existing) = provider_binary_on_path(provider_binary, path_dirs) {
        return Ok(Some(ProviderBinaryInvocation {
            command: existing.to_string_lossy().to_string(),
            source: "path-existing",
        }));
    }
    Ok(None)
}

fn resolve_configured_provider_binary_install_target(
    configured_bin: &str,
    project_root: &Path,
    home_dir: Option<&Path>,
    path_dirs: &[PathBuf],
) -> Result<ProviderBinaryInstallTarget, String> {
    let configured_bin = configured_bin.trim();
    if configured_bin.is_empty() {
        return Err("asp.toml provider bin must not be empty".to_string());
    }
    let configured_path = Path::new(configured_bin);
    let path = if configured_path.is_absolute() {
        configured_path.to_path_buf()
    } else if configured_path.components().count() > 1 {
        project_root.join(configured_path)
    } else if let Some(home_dir) = home_dir {
        home_dir.join(".local/bin").join(configured_path)
    } else if let Some(existing) = provider_binary_on_path(configured_bin, path_dirs) {
        existing
    } else if let Some(writable_dir) = first_writable_path_dir(path_dirs) {
        writable_dir.join(configured_path)
    } else {
        return Err(format!(
            "no install target for asp.toml provider bin `{configured_bin}`; set HOME or put a writable directory on PATH"
        ));
    };
    Ok(ProviderBinaryInstallTarget {
        path,
        source: "asp.toml",
    })
}

fn resolve_configured_provider_binary_invocation(
    configured_bin: &str,
    project_root: &Path,
    home_dir: Option<&Path>,
    path_dirs: &[PathBuf],
) -> Result<ProviderBinaryInvocation, String> {
    let configured_bin = configured_bin.trim();
    if configured_bin.is_empty() {
        return Err("asp.toml provider bin must not be empty".to_string());
    }
    let configured_path = Path::new(configured_bin);
    let command = if configured_path.is_absolute() {
        configured_path.to_path_buf()
    } else if configured_path.components().count() > 1 {
        project_root.join(configured_path)
    } else {
        home_local_bin(configured_bin, home_dir)
            .filter(|path| path.is_file())
            .or_else(|| provider_binary_on_path(configured_bin, path_dirs))
            .unwrap_or_else(|| configured_path.to_path_buf())
    };
    Ok(ProviderBinaryInvocation {
        command: command.to_string_lossy().to_string(),
        source: "asp.toml",
    })
}

fn provider_binary_on_path(binary: &str, path_dirs: &[PathBuf]) -> Option<PathBuf> {
    path_dirs
        .iter()
        .map(|dir| dir.join(binary))
        .find(|candidate| candidate.is_file())
}

fn first_writable_path_dir(path_dirs: &[PathBuf]) -> Option<&PathBuf> {
    path_dirs
        .iter()
        .find(|dir| dir.is_dir() && directory_is_writable(dir))
}

fn home_local_bin(binary: &str, home_dir: Option<&Path>) -> Option<PathBuf> {
    home_dir.map(|home_dir| home_dir.join(".local/bin").join(binary))
}

pub(super) fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

pub(super) fn path_dirs() -> Vec<PathBuf> {
    env::var_os("PATH")
        .map(|path| env::split_paths(&path).collect())
        .unwrap_or_default()
}

fn directory_is_writable(dir: &Path) -> bool {
    let probe = dir.join(format!(
        ".agent-semantic-provider-install-check-{}",
        std::process::id()
    ));
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
    {
        Ok(_) => {
            let _ = fs::remove_file(probe);
            true
        }
        Err(_) => false,
    }
}

#[cfg(test)]
#[path = "../../tests/unit/install_provider_target.rs"]
mod install_provider_target_tests;
