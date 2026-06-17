//! PATH-visible `asp` binary installation helpers.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

const SEMANTIC_AGENT_PROTOCOL_BIN: &str = "asp";
const SEMANTIC_AGENT_BIN_DIR_ENV: &str = "SEMANTIC_AGENT_BIN_DIR";

pub(crate) struct ProtocolBinaryInstall {
    pub(crate) path: PathBuf,
    pub(crate) status: &'static str,
}

pub(crate) fn ensure_protocol_binary_installed_for_path() -> Result<ProtocolBinaryInstall, String> {
    let current_exe = env::current_exe()
        .map_err(|error| format!("failed to resolve current protocol binary: {error}"))?;
    let current_name = current_exe
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if current_name.trim_end_matches(".exe") != SEMANTIC_AGENT_PROTOCOL_BIN {
        return Err(format!(
            "semantic hook setup must run through `{SEMANTIC_AGENT_PROTOCOL_BIN}` so generated hooks can resolve the same binary on PATH"
        ));
    }

    if let Some(bin_dir) = env::var_os(SEMANTIC_AGENT_BIN_DIR_ENV).filter(|value| !value.is_empty())
    {
        let bin_dir = PathBuf::from(bin_dir);
        fs::create_dir_all(&bin_dir)
            .map_err(|error| format!("failed to create {}: {error}", bin_dir.display()))?;
        require_path_contains_dir(&bin_dir)?;
        let target = bin_dir.join(SEMANTIC_AGENT_PROTOCOL_BIN);
        let status = install_protocol_binary(&current_exe, &target)?;
        return Ok(ProtocolBinaryInstall {
            path: target,
            status,
        });
    }

    if let Some(existing) = protocol_binary_on_path() {
        let status = install_protocol_binary(&current_exe, &existing)?;
        return Ok(ProtocolBinaryInstall {
            path: existing,
            status,
        });
    }

    let target_dir = first_writable_path_dir().ok_or_else(|| {
        format!(
            "`{SEMANTIC_AGENT_PROTOCOL_BIN}` is not on PATH and no writable PATH directory was found; set {SEMANTIC_AGENT_BIN_DIR_ENV} to a PATH directory and rerun install"
        )
    })?;
    let target = target_dir.join(SEMANTIC_AGENT_PROTOCOL_BIN);
    let status = install_protocol_binary(&current_exe, &target)?;
    Ok(ProtocolBinaryInstall {
        path: target,
        status,
    })
}

pub(crate) fn protocol_binary_on_path() -> Option<PathBuf> {
    path_dirs().into_iter().find_map(|dir| {
        let candidate = dir.join(SEMANTIC_AGENT_PROTOCOL_BIN);
        candidate.is_file().then_some(candidate)
    })
}

pub(crate) fn install_protocol_binary(
    source: &Path,
    target: &Path,
) -> Result<&'static str, String> {
    if same_file(source, target) {
        return Ok("already-present");
    }
    let status = if target.is_file() {
        "updated"
    } else {
        "installed"
    };
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let temp = temporary_protocol_binary_path(target);
    if temp.exists() {
        fs::remove_file(&temp)
            .map_err(|error| format!("failed to remove stale {}: {error}", temp.display()))?;
    }
    fs::copy(source, &temp).map_err(|error| {
        format!(
            "failed to stage {SEMANTIC_AGENT_PROTOCOL_BIN} at {}: {error}",
            temp.display()
        )
    })?;
    let permissions = fs::metadata(source)
        .map_err(|error| format!("failed to inspect {}: {error}", source.display()))?
        .permissions();
    fs::set_permissions(&temp, permissions)
        .map_err(|error| format!("failed to chmod {}: {error}", temp.display()))?;
    if target.exists() {
        fs::remove_file(target)
            .map_err(|error| format!("failed to replace {}: {error}", target.display()))?;
    }
    fs::rename(&temp, target).map_err(|error| {
        format!(
            "failed to install {SEMANTIC_AGENT_PROTOCOL_BIN} to {}: {error}",
            target.display()
        )
    })?;
    Ok(status)
}

fn temporary_protocol_binary_path(target: &Path) -> PathBuf {
    let file_name = target
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(SEMANTIC_AGENT_PROTOCOL_BIN);
    target.with_file_name(format!(".{file_name}.{}.tmp", process::id()))
}

fn require_path_contains_dir(dir: &Path) -> Result<(), String> {
    if path_dirs().iter().any(|path_dir| same_dir(path_dir, dir)) {
        Ok(())
    } else {
        Err(format!(
            "{SEMANTIC_AGENT_BIN_DIR_ENV}={} is not present in PATH; generated hooks use bare `{SEMANTIC_AGENT_PROTOCOL_BIN}`",
            dir.display()
        ))
    }
}

fn first_writable_path_dir() -> Option<PathBuf> {
    path_dirs()
        .into_iter()
        .find(|dir| dir.is_dir() && directory_is_writable(dir))
}

fn directory_is_writable(dir: &Path) -> bool {
    let probe = dir.join(format!(
        ".agent-semantic-protocol-install-check-{}",
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

fn path_dirs() -> Vec<PathBuf> {
    env::var_os("PATH")
        .map(|path| env::split_paths(&path).collect())
        .unwrap_or_default()
}

fn same_file(left: &Path, right: &Path) -> bool {
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

fn same_dir(left: &Path, right: &Path) -> bool {
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}
