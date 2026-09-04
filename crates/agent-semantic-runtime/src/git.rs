use serde::Deserialize;
use serde::Serialize;
use std::fmt;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use agent_semantic_config::load_asp_project_config_file;

pub trait CancellationProbe: Send + Sync {
    fn is_cancelled(&self) -> bool;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NeverCancelled;

impl CancellationProbe for NeverCancelled {
    fn is_cancelled(&self) -> bool {
        false
    }
}

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

#[path = "git_candidate_model.rs"]
mod candidate_model;
pub use candidate_model::RepositoryCandidate;
pub use candidate_model::RepositoryCandidateAuthority;
pub use candidate_model::RepositoryCandidateGeneration;
pub use candidate_model::RepositoryCandidateMetrics;
pub use candidate_model::RepositoryCandidateMode;
pub use candidate_model::RepositoryCandidatePolicyExclusion;
pub use candidate_model::RepositoryCandidateScope;
pub use candidate_model::RepositoryCandidateSnapshot;
pub use candidate_model::RepositoryCandidateState;
pub use candidate_model::RepositoryIdentity;
pub use candidate_model::WorktreeIdentity;

#[derive(Debug)]
pub enum GitWorkspaceFileScopeError {
    DiscoverRepository {
        message: String,
    },
    MissingWorktree {
        git_dir: PathBuf,
    },
    LoadIndex {
        message: String,
    },
    ConfigureDirwalk {
        message: String,
    },
    WalkWorktree {
        message: String,
    },
    InspectWorktreeOverlay {
        message: String,
    },
    ReadWorktreeOverlay {
        path: PathBuf,
        message: String,
    },
    LoadProjectConfig {
        path: PathBuf,
        message: String,
    },
    ProjectOutsideWorktree {
        project_root: PathBuf,
        worktree_root: PathBuf,
    },
    ProjectOutsideCandidateScope {
        project_root: PathBuf,
        candidate_root: PathBuf,
    },
    ProjectRepositoryUnavailable {
        project_root: PathBuf,
    },
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
            Self::InspectWorktreeOverlay { message } => {
                write!(
                    formatter,
                    "failed to inspect Git worktree overlay: {message}"
                )
            }
            Self::ReadWorktreeOverlay { path, message } => write!(
                formatter,
                "failed to read Git worktree overlay {}: {message}",
                path.display()
            ),
            Self::LoadProjectConfig { path, message } => write!(
                formatter,
                "failed to load ASP project discovery config {}: {message}",
                path.display()
            ),
            Self::ProjectOutsideWorktree {
                project_root,
                worktree_root,
            } => write!(
                formatter,
                "repository candidate project root {} is outside worktree {}",
                project_root.display(),
                worktree_root.display()
            ),
            Self::ProjectOutsideCandidateScope {
                project_root,
                candidate_root,
            } => write!(
                formatter,
                "repository candidate project root {} is outside candidate scope {}",
                project_root.display(),
                candidate_root.display()
            ),
            Self::ProjectRepositoryUnavailable { project_root } => write!(
                formatter,
                "repository candidate project root {} is no longer in a Git worktree",
                project_root.display()
            ),
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

fn repository_candidate_generation(
    repository_id: &str,
    worktree_id: &str,
    head_id: Option<&str>,
    candidate_scope: &RepositoryCandidateScope,
    candidates: &[RepositoryCandidate],
    policy_overlay_digest: &str,
    worktree_overlay_digest: &str,
) -> RepositoryCandidateGeneration {
    let mut generation = blake3::Hasher::new();
    generation.update(b"agent.semantic-protocols.repository-candidate-snapshot\0");
    generation.update(repository_id.as_bytes());
    generation.update(b"\0");
    generation.update(worktree_id.as_bytes());
    generation.update(b"\0head\0");
    generation.update(head_id.unwrap_or("unborn").as_bytes());
    generation.update(b"\0candidate-scope\0");
    generation.update(candidate_scope.project_root.as_os_str().as_encoded_bytes());
    for candidate in candidates {
        generation.update(b"\0");
        generation.update(candidate.path.as_os_str().as_encoded_bytes());
        generation.update(b"\0");
        generation.update(match candidate.state {
            RepositoryCandidateState::Tracked => b"tracked",
            RepositoryCandidateState::Untracked => b"untracked",
        });
    }
    generation.update(b"\0policy-overlay\0");
    generation.update(policy_overlay_digest.as_bytes());
    generation.update(b"\0worktree-overlay\0");
    generation.update(worktree_overlay_digest.as_bytes());
    RepositoryCandidateGeneration {
        algorithm: "blake3-worktree-state-v1".to_owned(),
        digest: format!("blake3:{}", generation.finalize().to_hex()),
        authorities: vec![
            RepositoryCandidateAuthority::GitIndex,
            RepositoryCandidateAuthority::GitWorktree,
        ],
    }
}

pub fn discover_repository_candidate_snapshot(
    workspace: &Path,
) -> Result<Option<RepositoryCandidateSnapshot>, GitWorkspaceFileScopeError> {
    discover_repository_candidate_snapshot_cancellable(workspace, &NeverCancelled)
}

pub fn discover_repository_candidate_snapshot_cancellable(
    workspace: &Path,
    probe: &dyn CancellationProbe,
) -> Result<Option<RepositoryCandidateSnapshot>, GitWorkspaceFileScopeError> {
    if probe.is_cancelled() {
        return Err(GitWorkspaceFileScopeError::DiscoverRepository {
            message: "cancelled".to_owned(),
        });
    }
    let Some(scope) = discover_git_workspace_file_scope(workspace)? else {
        return Ok(None);
    };
    let project_root = canonicalize_if_possible(workspace);
    let worktree_prefix = project_root
        .strip_prefix(&scope.worktree_root)
        .map_err(|_| GitWorkspaceFileScopeError::ProjectOutsideWorktree {
            project_root: project_root.clone(),
            worktree_root: scope.worktree_root.clone(),
        })?;
    let worktree_prefix = if worktree_prefix.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        worktree_prefix.to_path_buf()
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
        .filter_map(|file| {
            if probe.is_cancelled() {
                return None;
            }
            let relative_path = if worktree_prefix == Path::new(".") {
                file.relative_path.clone()
            } else {
                file.relative_path
                    .strip_prefix(&worktree_prefix)
                    .ok()?
                    .to_path_buf()
            };
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
            Some(RepositoryCandidate {
                path: relative_path,
                state,
                authority,
            })
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| left.path.cmp(&right.path));
    let (policy_overlay_digest, policy_exclusions) =
        resolve_asp_discovery_policy(&scope.worktree_root, workspace, &candidates)?;
    let worktree_overlay_digest = repository_worktree_overlay_digest(
        &repository,
        &scope.worktree_root,
        &worktree_prefix,
        &scope.files,
    )?;
    let index_entry_count = candidates
        .iter()
        .filter(|candidate| candidate.state == RepositoryCandidateState::Tracked)
        .count();
    let worktree_addition_count = candidates.len() - index_entry_count;
    let candidate_scope = RepositoryCandidateScope { project_root };
    let candidate_generation = repository_candidate_generation(
        &repository_id,
        &worktree_id,
        head_id.as_deref(),
        &candidate_scope,
        &candidates,
        &policy_overlay_digest,
        &worktree_overlay_digest,
    );

    Ok(Some(RepositoryCandidateSnapshot {
        schema_id: "agent.semantic-protocols.repository-candidate-snapshot".to_owned(),
        schema_version: "1".to_owned(),
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
        candidate_scope,
        candidate_generation,
        policy_overlay_digest,
        policy_exclusions: policy_exclusions.clone(),
        metrics: RepositoryCandidateMetrics {
            index_entry_count,
            worktree_addition_count,
            candidate_count: candidates.len(),
            policy_exclusion_count: policy_exclusions.len(),
            full_workspace_reads: 0,
            full_merkle_rebuilds: 0,
            direct_db_opens: 0,
        },
        candidates,
    }))
}

fn repository_worktree_overlay_digest(
    repository: &gix::Repository,
    worktree_root: &Path,
    worktree_prefix: &Path,
    workspace_files: &[GitWorkspaceFile],
) -> Result<String, GitWorkspaceFileScopeError> {
    let status = repository
        .status(gix::progress::Discard)
        .map_err(|error| GitWorkspaceFileScopeError::InspectWorktreeOverlay {
            message: error.to_string(),
        })?
        .index_worktree_submodules(None)
        .untracked_files(gix::status::UntrackedFiles::Files);
    let changes = status
        .into_iter(std::iter::empty::<gix::bstr::BString>())
        .map_err(|error| GitWorkspaceFileScopeError::InspectWorktreeOverlay {
            message: error.to_string(),
        })?;
    let mut paths = changes
        .map(|change| {
            change
                .map(|item| gix::path::from_bstr(item.location()).into_owned())
                .map_err(|error| GitWorkspaceFileScopeError::InspectWorktreeOverlay {
                    message: error.to_string(),
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let directory_prefixes = paths
        .iter()
        .filter(|path| worktree_root.join(path).is_dir())
        .cloned()
        .collect::<Vec<_>>();
    paths.retain(|path| !worktree_root.join(path).is_dir());
    paths.extend(
        workspace_files
            .iter()
            .filter(|file| {
                directory_prefixes
                    .iter()
                    .any(|prefix| file.relative_path.starts_with(prefix))
            })
            .map(|file| file.relative_path.clone()),
    );
    paths.sort();
    paths.dedup();

    let mut digest = blake3::Hasher::new();
    digest.update(b"agent.semantic-protocols.repository-worktree-overlay\0");
    for repository_path in paths {
        let scoped_path = if worktree_prefix == Path::new(".") {
            repository_path.as_path()
        } else if let Ok(path) = repository_path.strip_prefix(worktree_prefix) {
            path
        } else {
            continue;
        };
        digest.update(scoped_path.as_os_str().as_encoded_bytes());
        digest.update(b"\0");
        let absolute = worktree_root.join(&repository_path);
        match std::fs::read(&absolute) {
            Ok(bytes) => {
                digest.update(b"present\0");
                digest.update(blake3::hash(&bytes).as_bytes());
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                digest.update(b"removed\0");
            }
            Err(error) => {
                return Err(GitWorkspaceFileScopeError::ReadWorktreeOverlay {
                    path: absolute,
                    message: error.to_string(),
                });
            }
        }
    }
    Ok(format!("blake3:{}", digest.finalize().to_hex()))
}

fn resolve_asp_discovery_policy(
    activation_root: &Path,
    invocation_root: &Path,
    candidates: &[RepositoryCandidate],
) -> Result<(String, Vec<RepositoryCandidatePolicyExclusion>), GitWorkspaceFileScopeError> {
    let mut ignored_dir_names = Vec::new();
    let mut include_hidden_dir_names = Vec::new();
    let mut roots = vec![activation_root];
    if invocation_root != activation_root {
        roots.push(invocation_root);
    }
    for root in roots {
        for path in [root.join("asp.toml"), root.join(".agents").join("asp.toml")] {
            if !path.is_file() {
                continue;
            }
            let config = load_asp_project_config_file(&path).map_err(|message| {
                GitWorkspaceFileScopeError::LoadProjectConfig {
                    path: path.clone(),
                    message,
                }
            })?;
            if let Some(value) = config.discovery.ignored_dir_names {
                ignored_dir_names = value;
            }
            if let Some(value) = config.discovery.include_hidden_dir_names {
                include_hidden_dir_names = value;
            }
        }
    }
    ignored_dir_names.sort();
    ignored_dir_names.dedup();
    include_hidden_dir_names.sort();
    include_hidden_dir_names.dedup();
    let mut exclusions = candidates
        .iter()
        .filter_map(|candidate| {
            let matched_value = candidate.path.components().find_map(|component| {
                let name = component.as_os_str().to_string_lossy();
                (ignored_dir_names.iter().any(|ignored| ignored == &name)
                    && !(name.starts_with('.')
                        && include_hidden_dir_names
                            .iter()
                            .any(|included| included == &name)))
                .then(|| name.into_owned())
            })?;
            Some(RepositoryCandidatePolicyExclusion {
                path: candidate.path.clone(),
                authority: "user-policy".to_owned(),
                reason_kind: "ignored-dir-name".to_owned(),
                matched_value,
            })
        })
        .collect::<Vec<_>>();
    exclusions.sort_by(|left, right| left.path.cmp(&right.path));
    let mut digest = blake3::Hasher::new();
    digest.update(b"agent.semantic-protocols.discovery-policy-overlay\0");
    for value in &ignored_dir_names {
        digest.update(b"\0ignore\0");
        digest.update(value.as_bytes());
    }
    for value in &include_hidden_dir_names {
        digest.update(b"\0include-hidden\0");
        digest.update(value.as_bytes());
    }
    Ok((format!("blake3:{}", digest.finalize().to_hex()), exclusions))
}

fn stable_identity(namespace: &str, basis: &[u8]) -> String {
    let mut digest = blake3::Hasher::new();
    digest.update(namespace.as_bytes());
    digest.update(b"\0");
    digest.update(basis);
    format!("{namespace}-{}", &digest.finalize().to_hex()[..16])
}

#[cfg(test)]
#[path = "../tests/unit/git/repository_candidate_snapshot.rs"]
mod repository_candidate_snapshot_tests;

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
    /// Discover the Git identity used to admit durable writes.
    ///
    /// Identity intentionally has no filesystem-marker or Git subprocess
    /// fallback: only a repository opened by Gix may own persistent state.
    pub(crate) fn discover(cwd: &Path) -> Self {
        Self::discover_with_gix(cwd).unwrap_or_else(Self::empty)
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
}

pub(crate) fn canonicalize_if_possible(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

pub(crate) fn path_identity(path: &Path) -> String {
    canonicalize_if_possible(path)
        .to_string_lossy()
        .replace('\\', "/")
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
