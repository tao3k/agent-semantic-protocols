use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Git remote URL captured as identity evidence.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RemoteUrl(pub String);

impl RemoteUrl {
    /// Borrow the remote URL string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RemoteUrl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GitIdentity {
    pub(crate) toplevel: Option<PathBuf>,
    pub(crate) git_dir: Option<PathBuf>,
    pub(crate) common_git_dir: Option<PathBuf>,
    pub(crate) remote_url: Option<RemoteUrl>,
}

impl GitIdentity {
    pub(crate) fn discover(cwd: &Path) -> Self {
        if let Some(identity) = Self::discover_from_filesystem(cwd) {
            return identity;
        }
        if !has_git_marker(cwd) {
            return Self::empty();
        }
        let toplevel = git_path(cwd, &["rev-parse", "--show-toplevel"]);
        let git_dir = git_path(cwd, &["rev-parse", "--absolute-git-dir"]);
        let common_git_dir = git_path(cwd, &["rev-parse", "--git-common-dir"]);
        let remote_url = git_stdout(cwd, &["config", "--get", "remote.origin.url"]).map(RemoteUrl);

        Self {
            toplevel,
            git_dir,
            common_git_dir,
            remote_url,
        }
    }

    fn empty() -> Self {
        Self {
            toplevel: None,
            git_dir: None,
            common_git_dir: None,
            remote_url: None,
        }
    }

    fn discover_from_filesystem(cwd: &Path) -> Option<Self> {
        let toplevel = find_git_toplevel(cwd)?;
        let git_dir = git_dir_from_marker(&toplevel)?;
        let common_git_dir =
            common_git_dir_from_git_dir(&git_dir).unwrap_or_else(|| git_dir.clone());
        let remote_url =
            remote_origin_url_from_config(&common_git_dir.join("config")).map(RemoteUrl);

        Some(Self {
            toplevel: Some(toplevel),
            git_dir: Some(canonicalize_if_possible(&git_dir)),
            common_git_dir: Some(canonicalize_if_possible(&common_git_dir)),
            remote_url,
        })
    }
}

pub(crate) fn canonicalize_if_possible(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

pub(crate) fn path_identity(path: &Path) -> String {
    canonicalize_if_possible(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn has_git_marker(cwd: &Path) -> bool {
    find_git_toplevel(cwd).is_some()
}

fn find_git_toplevel(cwd: &Path) -> Option<PathBuf> {
    let mut current = Some(canonicalize_if_possible(cwd));
    while let Some(path) = current {
        if path.join(".git").exists() {
            return Some(path);
        }
        current = path.parent().map(Path::to_path_buf);
    }
    None
}

fn git_dir_from_marker(toplevel: &Path) -> Option<PathBuf> {
    let marker = toplevel.join(".git");
    if marker.is_dir() {
        return Some(marker);
    }
    let content = fs::read_to_string(&marker).ok()?;
    let git_dir = content.trim().strip_prefix("gitdir:")?.trim();
    let path = PathBuf::from(git_dir);
    Some(if path.is_absolute() {
        path
    } else {
        toplevel.join(path)
    })
}

fn common_git_dir_from_git_dir(git_dir: &Path) -> Option<PathBuf> {
    let content = fs::read_to_string(git_dir.join("commondir")).ok()?;
    let common_dir = content.trim();
    if common_dir.is_empty() {
        return None;
    }
    let path = PathBuf::from(common_dir);
    Some(if path.is_absolute() {
        path
    } else {
        git_dir.join(path)
    })
}

fn remote_origin_url_from_config(config_path: &Path) -> Option<String> {
    let content = fs::read_to_string(config_path).ok()?;
    let mut in_origin = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_origin = trimmed == r#"[remote "origin"]"#;
            continue;
        }
        if in_origin {
            let Some((key, value)) = trimmed.split_once('=') else {
                continue;
            };
            if key.trim() == "url" {
                let value = value.trim();
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
    }
    None
}

fn git_path(cwd: &Path, args: &[&str]) -> Option<PathBuf> {
    git_stdout(cwd, args)
        .map(PathBuf::from)
        .map(|path| {
            if path.is_absolute() {
                path
            } else {
                cwd.join(path)
            }
        })
        .map(|path| canonicalize_if_possible(&path))
}

fn git_stdout(cwd: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    let value = stdout.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}
