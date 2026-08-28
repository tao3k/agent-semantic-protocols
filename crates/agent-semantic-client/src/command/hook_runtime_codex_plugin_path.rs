use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum CodexPluginAspPathStatus {
    Current {
        resolved_path: PathBuf,
        artifact_path: PathBuf,
    },
    Missing,
    Stale {
        resolved_path: PathBuf,
        artifact_path: PathBuf,
    },
}

pub(super) fn inspect(asp_binary_path: &Path) -> Result<CodexPluginAspPathStatus, String> {
    let artifact_path = fs::canonicalize(asp_binary_path).map_err(|error| {
        format!(
            "failed to resolve active ASP artifact {}: {error}",
            asp_binary_path.display()
        )
    })?;
    let Some(path) = std::env::var_os("PATH") else {
        return Ok(CodexPluginAspPathStatus::Missing);
    };
    let executable_name = if cfg!(windows) { "asp.exe" } else { "asp" };
    let Some(resolved_path) = std::env::split_paths(&path)
        .map(|directory| directory.join(executable_name))
        .find(|candidate| path_is_executable(candidate))
    else {
        return Ok(CodexPluginAspPathStatus::Missing);
    };
    let resolved_path = fs::canonicalize(&resolved_path).map_err(|error| {
        format!(
            "failed to resolve ASP from Codex plugin PATH {}: {error}",
            resolved_path.display()
        )
    })?;
    let same_artifact = resolved_path == artifact_path
        || fs::read(&resolved_path)
            .and_then(|resolved| {
                fs::read(&artifact_path).map(|artifact| resolved.as_slice() == artifact.as_slice())
            })
            .map_err(|error| format!("failed to compare ASP PATH artifact identity: {error}"))?;
    if same_artifact {
        Ok(CodexPluginAspPathStatus::Current {
            resolved_path,
            artifact_path,
        })
    } else {
        Ok(CodexPluginAspPathStatus::Stale {
            resolved_path,
            artifact_path,
        })
    }
}

#[cfg(unix)]
fn path_is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    path.metadata()
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn path_is_executable(path: &Path) -> bool {
    path.is_file()
}

pub(super) fn require_current(asp_binary_path: &Path) -> Result<PathBuf, String> {
    match inspect(asp_binary_path)? {
        CodexPluginAspPathStatus::Current { resolved_path, .. } => Ok(resolved_path),
        CodexPluginAspPathStatus::Missing => Err(
            "Codex App PATH cannot resolve `asp`; install the global ASP launcher before enabling plugin hooks"
                .to_string(),
        ),
        CodexPluginAspPathStatus::Stale {
            resolved_path,
            artifact_path,
        } => Err(format!(
            "Codex App PATH resolves stale ASP {} instead of active artifact {}",
            resolved_path.display(),
            artifact_path.display()
        )),
    }
}
