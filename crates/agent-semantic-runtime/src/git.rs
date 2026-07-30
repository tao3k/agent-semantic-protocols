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

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum GitWorkspaceFileOrigin {
    Tracked,
    Untracked,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GitWorkspaceFile {
    relative_path: PathBuf,
    origin: GitWorkspaceFileOrigin,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GitWorkspaceFileScope {
    worktree_root: PathBuf,
    files: Vec<GitWorkspaceFile>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryCandidateSnapshot {
    pub schema_id: &'static str,
    pub schema_version: &'static str,
    pub mode: RepositoryCandidateMode,
    pub repository_identity: RepositoryIdentity,
    pub worktree_identity: WorktreeIdentity,
    pub candidate_generation: RepositoryCandidateGeneration,
    pub candidates: Vec<RepositoryCandidate>,
    pub metrics: RepositoryCandidateMetrics,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RepositoryCandidateMode {
    Git,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryIdentity {
    pub repository_id: String,
    pub identity_basis: String,
    pub git_common_dir: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_url: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeIdentity {
    pub worktree_id: String,
    pub worktree_root: PathBuf,
    pub git_dir: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryCandidateGeneration {
    pub algorithm: &'static str,
    pub digest: String,
    pub authorities: Vec<RepositoryCandidateAuthority>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryCandidate {
    pub path: PathBuf,
    pub state: RepositoryCandidateState,
    pub authority: RepositoryCandidateAuthority,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RepositoryCandidateState {
    Tracked,
    Untracked,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RepositoryCandidateAuthority {
    GitIndex,
    GitWorktree,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryCandidateMetrics {
    pub index_entry_count: usize,
    pub worktree_addition_count: usize,
    pub candidate_count: usize,
    pub full_workspace_reads: usize,
    pub database_opens: usize,
}

#[derive(Debug)]
pub enum GitWorkspaceFileScopeError {
    DiscoverRepository { message: String },
    MissingWorktree { git_dir: PathBuf },
    LoadIndex { message: String },
    ConfigureDirwalk { message: String },
    WalkWorktree { message: String },
}

impl std::fmt::Display for GitWorkspaceFileScopeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DiscoverRepository { message } => {
                write!(formatter, "failed to discover Git repository: {message}")
            }
            Self::MissingWorktree { git_dir } => write!(
                formatter,
                "Git repository at {} has no worktree",
                git_dir.display()
            ),
            Self::LoadIndex { message } => {
                write!(formatter, "failed to load Git worktree index: {message}")
            }
            Self::ConfigureDirwalk { message } => {
                write!(
                    formatter,
                    "failed to configure Git worktree walk: {message}"
                )
            }
            Self::WalkWorktree { message } => {
                write!(
                    formatter,
                    "failed to walk Git worktree additions: {message}"
                )
            }
        }
    }
}

impl std::error::Error for GitWorkspaceFileScopeError {}

fn discover_git_workspace_file_scope(
    workspace: &Path,
) -> Result<Option<GitWorkspaceFileScope>, GitWorkspaceFileScopeError> {
    let Ok(repository) = gix::discover(workspace) else {
        return Ok(None);
    };
    let worktree_root = repository
        .workdir()
        .map(canonicalize_if_possible)
        .ok_or_else(|| GitWorkspaceFileScopeError::MissingWorktree {
            git_dir: repository.git_dir().to_path_buf(),
        })?;
    let index = repository
        .index_or_load_from_head_or_empty()
        .map_err(|error| GitWorkspaceFileScopeError::LoadIndex {
            message: error.to_string(),
        })?;
    let mut files = std::collections::BTreeMap::new();

    for entry in index.entries() {
        if entry.stage_raw() != 0 {
            continue;
        }
        let relative_path = gix::path::from_bstr(entry.path(&index)).into_owned();
        if is_regular_workspace_file(&worktree_root, &relative_path) {
            files.insert(relative_path, GitWorkspaceFileOrigin::Tracked);
        }
    }

    let options = repository
        .dirwalk_options()
        .map_err(|error| GitWorkspaceFileScopeError::ConfigureDirwalk {
            message: error.to_string(),
        })?
        .emit_tracked(false)
        .emit_ignored(None)
        .emit_untracked(gix::dir::walk::EmissionMode::Matching)
        .emit_empty_directories(false)
        .classify_untracked_bare_repositories(false);
    let entries = repository
        .dirwalk_iter(
            index,
            Vec::<gix::bstr::BString>::new(),
            Default::default(),
            options,
        )
        .map_err(|error| GitWorkspaceFileScopeError::WalkWorktree {
            message: error.to_string(),
        })?;

    for entry in entries {
        let entry = entry.map_err(|error| GitWorkspaceFileScopeError::WalkWorktree {
            message: error.to_string(),
        })?;
        if entry.entry.status != gix::dir::entry::Status::Untracked {
            continue;
        }
        let relative_path = gix::path::from_bstring(entry.entry.rela_path);
        if is_regular_workspace_file(&worktree_root, &relative_path) {
            files
                .entry(relative_path)
                .or_insert(GitWorkspaceFileOrigin::Untracked);
        }
    }

    Ok(Some(GitWorkspaceFileScope {
        worktree_root,
        files: files
            .into_iter()
            .map(|(relative_path, origin)| GitWorkspaceFile {
                relative_path,
                origin,
            })
            .collect(),
    }))
}

pub fn discover_repository_candidate_snapshot(
    workspace: &Path,
) -> Result<Option<RepositoryCandidateSnapshot>, GitWorkspaceFileScopeError> {
    let Some(scope) = discover_git_workspace_file_scope(workspace)? else {
        return Ok(None);
    };
    let repository = gix::discover(workspace).map_err(|error| {
        GitWorkspaceFileScopeError::DiscoverRepository {
            message: error.to_string(),
        }
    })?;
    let git_dir = canonicalize_if_possible(repository.git_dir());
    let git_common_dir = canonicalize_if_possible(repository.common_dir());
    let remote_url = canonical_remote_url_from_repository(&repository)
        .or_else(|| canonical_remote_url_from_git_dir(&git_dir))
        .or_else(|| canonical_remote_url_from_git_dir(&git_common_dir));
    let identity_basis = remote_url
        .as_deref()
        .map(|url| format!("remote:{url}"))
        .unwrap_or_else(|| format!("git-common-dir:{}", git_common_dir.display()));
    let repository_id = stable_identity("repository", identity_basis.as_bytes());
    let worktree_basis = format!(
        "{}\0{}\0{}",
        repository_id,
        scope.worktree_root.display(),
        git_dir.display()
    );
    let worktree_id = stable_identity("worktree", worktree_basis.as_bytes());
    let head_id = repository.head_id().ok().map(|id| id.detach().to_string());
    let mut candidates = scope
        .files
        .iter()
        .map(|file| {
            let (state, authority) = match file.origin {
                GitWorkspaceFileOrigin::Tracked => (
                    RepositoryCandidateState::Tracked,
                    RepositoryCandidateAuthority::GitIndex,
                ),
                GitWorkspaceFileOrigin::Untracked => (
                    RepositoryCandidateState::Untracked,
                    RepositoryCandidateAuthority::GitWorktree,
                ),
            };
            RepositoryCandidate {
                path: file.relative_path.clone(),
                state,
                authority,
            }
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| left.path.cmp(&right.path));
    let index_entry_count = candidates
        .iter()
        .filter(|candidate| candidate.state == RepositoryCandidateState::Tracked)
        .count();
    let worktree_addition_count = candidates.len() - index_entry_count;
    let mut generation = blake3::Hasher::new();
    generation.update(b"agent.semantic-protocols.repository-candidate-snapshot\0");
    generation.update(repository_id.as_bytes());
    generation.update(b"\0");
    generation.update(worktree_id.as_bytes());
    for candidate in &candidates {
        generation.update(b"\0");
        generation.update(candidate.path.as_os_str().as_encoded_bytes());
        generation.update(b"\0");
        generation.update(match candidate.state {
            RepositoryCandidateState::Tracked => b"tracked",
            RepositoryCandidateState::Untracked => b"untracked",
        });
    }
    let candidate_generation = RepositoryCandidateGeneration {
        algorithm: "blake3-path-set-v1",
        digest: format!("blake3:{}", generation.finalize().to_hex()),
        authorities: vec![
            RepositoryCandidateAuthority::GitIndex,
            RepositoryCandidateAuthority::GitWorktree,
        ],
    };

    Ok(Some(RepositoryCandidateSnapshot {
        schema_id: "agent.semantic-protocols.repository-candidate-snapshot",
        schema_version: "1",
        mode: RepositoryCandidateMode::Git,
        repository_identity: RepositoryIdentity {
            repository_id,
            identity_basis,
            git_common_dir,
            remote_url,
        },
        worktree_identity: WorktreeIdentity {
            worktree_id,
            worktree_root: scope.worktree_root,
            git_dir,
            head_id,
        },
        candidate_generation,
        metrics: RepositoryCandidateMetrics {
            index_entry_count,
            worktree_addition_count,
            candidate_count: candidates.len(),
            full_workspace_reads: 0,
            database_opens: 0,
        },
        candidates,
    }))
}

fn stable_identity(namespace: &str, basis: &[u8]) -> String {
    let mut digest = blake3::Hasher::new();
    digest.update(namespace.as_bytes());
    digest.update(b"\0");
    digest.update(basis);
    format!("{namespace}-{}", &digest.finalize().to_hex()[..16])
}

#[cfg(test)]
mod repository_candidate_snapshot_tests {
    use super::{
        RepositoryCandidateAuthority, RepositoryCandidateState,
        discover_repository_candidate_snapshot,
    };
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);

    struct Fixture {
        root: PathBuf,
    }

    impl Fixture {
        fn new(name: &str) -> Self {
            let root = std::env::temp_dir().join(format!(
                "asp-repository-candidates-{name}-{}-{}",
                std::process::id(),
                NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&root).expect("create fixture root");
            Self { root }
        }

        fn write(&self, relative: &str, contents: &str) {
            let path = self.root.join(relative);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).expect("create fixture parent");
            }
            std::fs::write(path, contents).expect("write fixture file");
        }

        fn git(&self, args: &[&str]) {
            let output = Command::new("git")
                .args(args)
                .current_dir(&self.root)
                .output()
                .expect("run git fixture command");
            assert!(
                output.status.success(),
                "git {:?} failed: {}",
                args,
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn git_snapshot_is_deterministic_and_excludes_ignored_files() {
        let fixture = Fixture::new("tracked-untracked");
        fixture.git(&["init", "--quiet"]);
        fixture.write(".gitignore", "ignored/\n");
        fixture.write("src/tracked.rs", "pub fn tracked() {}\n");
        fixture.write("src/untracked.rs", "pub fn untracked() {}\n");
        fixture.write("ignored/generated.rs", "pub fn generated() {}\n");
        fixture.git(&["add", ".gitignore", "src/tracked.rs"]);

        let first = discover_repository_candidate_snapshot(&fixture.root)
            .expect("discover repository candidates")
            .expect("Git snapshot exists");
        let second = discover_repository_candidate_snapshot(&fixture.root)
            .expect("rediscover repository candidates")
            .expect("Git snapshot exists");

        let candidates = first
            .candidates
            .iter()
            .map(|candidate| (candidate.path.clone(), candidate.state, candidate.authority))
            .collect::<Vec<_>>();
        assert_eq!(
            candidates,
            vec![
                (
                    PathBuf::from(".gitignore"),
                    RepositoryCandidateState::Tracked,
                    RepositoryCandidateAuthority::GitIndex,
                ),
                (
                    PathBuf::from("src/tracked.rs"),
                    RepositoryCandidateState::Tracked,
                    RepositoryCandidateAuthority::GitIndex,
                ),
                (
                    PathBuf::from("src/untracked.rs"),
                    RepositoryCandidateState::Untracked,
                    RepositoryCandidateAuthority::GitWorktree,
                ),
            ]
        );
        assert_eq!(
            first.candidate_generation.digest,
            second.candidate_generation.digest
        );
        assert_eq!(first.metrics.index_entry_count, 2);
        assert_eq!(first.metrics.worktree_addition_count, 1);
        assert_eq!(first.metrics.candidate_count, 3);
        assert_eq!(first.metrics.full_workspace_reads, 0);
        assert_eq!(first.metrics.database_opens, 0);
        assert!(
            !first
                .candidates
                .iter()
                .any(|candidate| candidate.path == Path::new("ignored/generated.rs"))
        );
    }

    #[test]
    fn non_git_directory_has_no_repository_candidate_snapshot() {
        let fixture = Fixture::new("non-git");

        let snapshot = discover_repository_candidate_snapshot(&fixture.root)
            .expect("discover non-Git directory");

        assert!(snapshot.is_none());
    }
}

fn is_regular_workspace_file(worktree_root: &Path, relative_path: &Path) -> bool {
    std::fs::symlink_metadata(worktree_root.join(relative_path))
        .is_ok_and(|metadata| metadata.file_type().is_file())
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
