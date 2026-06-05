//! Executable discovery shared by hook activation and runtime profiles.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExecutableStatus {
    Available,
    Missing,
    Unexecutable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExecutableResolution {
    pub(crate) path: Option<PathBuf>,
    pub(crate) status: ExecutableStatus,
    pub(crate) reason: Option<String>,
}

pub(crate) fn resolve_executable(program: &str) -> Option<PathBuf> {
    resolve_executable_with_status(program).path
}

pub(crate) fn resolve_executable_with_status(program: &str) -> ExecutableResolution {
    let path = PathBuf::from(program);
    if program_has_path_separator(program) || path.is_absolute() {
        return resolve_explicit_executable(path);
    }

    let mut first_existing = None;
    if let Some(paths) = env::var_os("PATH") {
        for dir in env::split_paths(&paths) {
            let candidate = dir.join(program);
            if is_executable_file(&candidate) {
                return ExecutableResolution {
                    path: Some(canonical_path(candidate)),
                    status: ExecutableStatus::Available,
                    reason: None,
                };
            }
            if first_existing.is_none() && candidate.exists() {
                first_existing = Some(candidate);
            }
        }
    }

    if let Some(candidate) = first_existing {
        return ExecutableResolution {
            path: None,
            status: ExecutableStatus::Unexecutable,
            reason: Some(format!(
                "{} exists but is not executable",
                candidate.display()
            )),
        };
    }

    ExecutableResolution {
        path: None,
        status: ExecutableStatus::Missing,
        reason: Some(format!("`{program}` was not found on PATH")),
    }
}

pub(crate) fn is_executable_file(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn resolve_explicit_executable(path: PathBuf) -> ExecutableResolution {
    if is_executable_file(&path) {
        return ExecutableResolution {
            path: Some(canonical_path(path)),
            status: ExecutableStatus::Available,
            reason: None,
        };
    }
    if path.exists() {
        ExecutableResolution {
            path: None,
            status: ExecutableStatus::Unexecutable,
            reason: Some(format!("{} exists but is not executable", path.display())),
        }
    } else {
        ExecutableResolution {
            path: None,
            status: ExecutableStatus::Missing,
            reason: Some(format!("{} does not exist", path.display())),
        }
    }
}

fn canonical_path(path: PathBuf) -> PathBuf {
    fs::canonicalize(&path).unwrap_or(path)
}

fn program_has_path_separator(program: &str) -> bool {
    program.contains('/') || program.contains('\\')
}
