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

    /// Normalize a network Git remote into a scheme-independent repository identity.
    #[must_use]
    pub fn canonical_identity(&self) -> Option<String> {
        let remote = self.0.trim().trim_end_matches('/');
        if remote.is_empty() {
            return None;
        }
        let parsed = gix::Url::try_from(remote).ok()?;
        let host = parsed.host()?.trim().to_ascii_lowercase();
        let host = parsed
            .port
            .map_or(host.clone(), |port| format!("{host}:{port}"));
        let path = String::from_utf8_lossy(parsed.path.as_ref());
        let path = path
            .trim_matches('/')
            .strip_suffix(".git")
            .unwrap_or(path.trim_matches('/'));
        if host.is_empty() || path.is_empty() {
            return None;
        }
        Some(format!("{host}/{path}"))
    }
}

#[cfg(test)]
#[path = "../tests/unit/git_remote_identity.rs"]
mod remote_identity_tests;

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
        if let Some(identity) = Self::discover_with_gix(cwd) {
            return identity;
        }
        if let Some(identity) = Self::discover_from_filesystem(cwd) {
            return identity;
        }
        if !has_git_marker(cwd) {
            return Self::empty();
        }
        let toplevel = git_path(cwd, &["rev-parse", "--show-toplevel"]);
        let git_dir = git_path(cwd, &["rev-parse", "--absolute-git-dir"]);
        let common_git_dir = git_path(cwd, &["rev-parse", "--git-common-dir"]);
        let remote_url = canonical_remote_url_from_git(cwd).map(RemoteUrl);

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

    fn discover_with_gix(cwd: &Path) -> Option<Self> {
        let repository = gix::discover(cwd).ok()?;
        let git_dir = canonicalize_if_possible(repository.git_dir());
        let common_git_dir = canonicalize_if_possible(repository.common_dir());
        let toplevel = repository.workdir().map(canonicalize_if_possible);
        let remote_url = canonical_remote_url_from_repository(&repository)
            .or_else(|| canonical_remote_url_from_git_dir(&git_dir))
            .or_else(|| canonical_remote_url_from_git_dir(&common_git_dir))
            .map(RemoteUrl);

        Some(Self {
            toplevel,
            git_dir: Some(git_dir),
            common_git_dir: Some(common_git_dir),
            remote_url,
        })
    }

    fn discover_from_filesystem(cwd: &Path) -> Option<Self> {
        let toplevel = find_git_toplevel(cwd)?;
        let git_dir = git_dir_from_marker(&toplevel)?;
        let common_git_dir =
            common_git_dir_from_git_dir(&git_dir).unwrap_or_else(|| git_dir.clone());
        let remote_url = canonical_remote_url_from_git_dir(&common_git_dir).map(RemoteUrl);

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

pub(crate) fn canonical_remote_url_from_repository(repository: &gix::Repository) -> Option<String> {
    let config = repository.config_snapshot();
    let configured_remote = config
        .string("agent-semantic.canonicalRemote")
        .map(|value| String::from_utf8_lossy(value.as_ref()).into_owned());
    let push_default = config
        .string("remote.pushDefault")
        .map(|value| String::from_utf8_lossy(value.as_ref()).into_owned());
    let remotes = repository
        .remote_names()
        .into_iter()
        .filter_map(|name| {
            let remote = repository.try_find_remote(name.as_ref())?.ok()?;
            let url = remote.url(gix::remote::Direction::Fetch)?;
            Some((
                String::from_utf8_lossy(name.as_ref()).into_owned(),
                String::from_utf8_lossy(url.to_bstring().as_ref()).into_owned(),
            ))
        })
        .collect::<Vec<_>>();
    select_canonical_remote(
        configured_remote.as_deref(),
        push_default.as_deref(),
        &remotes,
    )
}

fn canonical_remote_url_from_git_dir(git_dir: &Path) -> Option<String> {
    if let Ok(config) = gix_config::File::from_git_dir(git_dir.to_path_buf())
        && let Some(remote) = canonical_remote_url_from_config(&config)
    {
        return Some(remote);
    }
    let config =
        gix_config::File::from_path_no_includes(git_dir.join("config"), gix_config::Source::Local)
            .ok()?;
    canonical_remote_url_from_config(&config)
}

fn canonical_remote_url_from_config(config: &gix_config::File<'_>) -> Option<String> {
    let configured_remote = config
        .string("agent-semantic.canonicalRemote")
        .map(|value| String::from_utf8_lossy(value.as_ref()).into_owned());
    let push_default = config
        .string("remote.pushDefault")
        .map(|value| String::from_utf8_lossy(value.as_ref()).into_owned());
    let remotes = config
        .sections_by_name("remote")
        .into_iter()
        .flatten()
        .filter_map(|section| {
            let name = section.header().subsection_name()?;
            let url = section.value("url")?;
            Some((
                String::from_utf8_lossy(name.as_ref()).into_owned(),
                String::from_utf8_lossy(url.as_ref()).into_owned(),
            ))
        })
        .collect::<Vec<_>>();
    select_canonical_remote(
        configured_remote.as_deref(),
        push_default.as_deref(),
        &remotes,
    )
}

fn select_canonical_remote(
    configured_remote: Option<&str>,
    push_default: Option<&str>,
    remotes: &[(String, String)],
) -> Option<String> {
    for selected_name in [configured_remote, push_default, Some("origin")]
        .into_iter()
        .flatten()
    {
        if let Some((_, url)) = remotes.iter().find(|(name, _)| name == selected_name) {
            return Some(url.clone());
        }
    }
    (remotes.len() == 1).then(|| remotes[0].1.clone())
}

fn canonical_remote_url_from_git(cwd: &Path) -> Option<String> {
    let configured_remote = git_stdout(cwd, &["config", "--get", "agent-semantic.canonicalRemote"]);
    let push_default = git_stdout(cwd, &["config", "--get", "remote.pushDefault"]);
    let remote_lines = git_stdout(cwd, &["config", "--get-regexp", r"^remote\..*\.url$"])?;
    let remotes = remote_lines
        .lines()
        .filter_map(|line| {
            let split_at = line.find(char::is_whitespace)?;
            let (key, url) = line.split_at(split_at);
            let name = key.strip_prefix("remote.")?.strip_suffix(".url")?;
            Some((name.to_string(), url.trim().to_string()))
        })
        .collect::<Vec<_>>();
    select_canonical_remote(
        configured_remote.as_deref(),
        push_default.as_deref(),
        &remotes,
    )
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
