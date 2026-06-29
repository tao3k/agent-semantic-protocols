//! Resolve ASP `State Core` identity and durable state paths.

use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::env;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Environment variable that overrides the ASP v2 state root.
pub const ASP_STATE_HOME_ENV: &str = "ASP_STATE_HOME";
/// Default directory under `HOME` for ASP v2 durable state.
pub const DEFAULT_STATE_HOME_DIR: &str = ".agent-semantic-protocols";
/// Layout version for global ASP state directories.
pub const STATE_LAYOUT_VERSION: &str = "state-v2";
/// Initial scope identity used before multiple named scopes exist.
pub const DEFAULT_SCOPE_ID: &str = "default";
/// Current client DB backend retained during the State Core phase.
pub const SQLITE_V1_BACKEND: &str = "sqlite-v1";
/// Future DB backend recorded in manifests for the Turso migration.
pub const TURSO_BACKEND: &str = "turso";
/// SQLite client DB filename under `live/client`.
pub const CLIENT_DB_FILE: &str = "client.sqlite3";
/// State Core client manifest filename under `live/client`.
pub const STATE_MANIFEST_FILE: &str = "manifest.json";

/// Stable executable identity for a repository.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RepoId(pub String);

impl RepoId {
    /// Borrow the stable repository id as a path-safe string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RepoId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Stable executable identity for a concrete checkout or worktree.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorkspaceId(pub String);

impl WorkspaceId {
    /// Borrow the stable workspace id as a path-safe string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for WorkspaceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Stable executable identity for a state scope.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ScopeId(pub String);

impl fmt::Display for ScopeId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
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
}

impl fmt::Display for RemoteUrl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Fully resolved State Core identity, paths, and cache evidence.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedState {
    pub state_home: PathBuf,
    pub repo: RepoIdentity,
    pub workspace: WorkspaceIdentity,
    pub scope_id: ScopeId,
    pub paths: StatePaths,
    pub project_local_cache: Option<ProjectLocalCacheEvidence>,
}

impl ResolvedState {
    /// Resolve State Core from the process environment.
    pub fn resolve(cwd: impl AsRef<Path>) -> Result<Self, String> {
        let state_home = resolve_state_home()?;
        Self::resolve_with_state_home(cwd, state_home)
    }

    /// Resolve State Core with an explicit state root.
    pub fn resolve_with_state_home(
        cwd: impl AsRef<Path>,
        state_home: impl AsRef<Path>,
    ) -> Result<Self, String> {
        let cwd = canonicalize_if_possible(cwd.as_ref());
        let state_home = canonicalize_parent(state_home.as_ref().to_path_buf());
        let git = GitIdentity::discover(&cwd);
        let checkout = CheckoutIdentity::new(&cwd, &git);
        let repo = RepoIdentity::from_checkout(&git, &checkout);
        let workspace = WorkspaceIdentity::from_checkout(&git, &checkout, &repo.repo_id);
        let paths = StatePaths::new(&state_home, &repo.repo_id, &workspace.workspace_id);
        let project_local_cache = git
            .toplevel
            .as_deref()
            .map(|root| root.join(".cache").join("agent-semantic-protocol"))
            .map(|path| ProjectLocalCacheEvidence {
                path: path.clone(),
                exists: path.exists(),
            });

        Ok(Self {
            state_home,
            repo,
            workspace,
            scope_id: ScopeId(DEFAULT_SCOPE_ID.to_string()),
            paths,
            project_local_cache,
        })
    }

    /// Create the minimal State Core v2 directory layout.
    pub fn ensure_minimal_layout(&self) -> Result<(), String> {
        fs::create_dir_all(&self.paths.registry_dir).map_err(io_error("create registry dir"))?;
        fs::create_dir_all(&self.paths.aliases_by_display_name_dir)
            .map_err(io_error("create aliases dir"))?;
        fs::create_dir_all(&self.paths.project_dir).map_err(io_error("create project dir"))?;
        fs::create_dir_all(&self.paths.workspace_dir).map_err(io_error("create workspace dir"))?;
        fs::create_dir_all(&self.paths.client_dir).map_err(io_error("create client dir"))?;
        fs::create_dir_all(&self.paths.artifacts_dir).map_err(io_error("create artifacts dir"))?;

        write_if_missing(
            &self.paths.version_file,
            format!("{STATE_LAYOUT_VERSION}\n"),
        )?;
        write_json_if_missing(
            &self.paths.state_json,
            &json!({
                "layoutVersion": STATE_LAYOUT_VERSION,
                "stateHome": self.state_home,
                "registryEventsPath": self.paths.registry_events_jsonl,
                "aliasesByDisplayNamePath": self.paths.aliases_by_display_name_dir,
            }),
        )?;
        write_if_missing(&self.paths.registry_events_jsonl, String::new())?;
        write_json_if_missing(
            &self.paths.project_json,
            &json!({
                "layoutVersion": STATE_LAYOUT_VERSION,
                "repoId": self.repo.repo_id,
                "displayName": self.repo.display_name,
                "checkoutRoot": self.repo.checkout_root,
                "gitToplevel": self.repo.git_toplevel,
                "gitDir": self.repo.git_dir,
                "gitCommonDir": self.repo.git_common_dir,
                "remoteUrl": self.repo.remote_url,
                "identityBasis": self.repo.identity_basis,
            }),
        )?;
        write_json_if_missing(
            &self.paths.workspace_json,
            &json!({
                "layoutVersion": STATE_LAYOUT_VERSION,
                "repoId": self.repo.repo_id,
                "workspaceId": self.workspace.workspace_id,
                "scopeId": self.scope_id,
                "displayName": self.workspace.display_name,
                "root": self.workspace.root,
                "gitDir": self.workspace.git_dir,
                "identityBasis": self.workspace.identity_basis,
            }),
        )?;
        write_json_if_missing(
            &self.paths.client_manifest_json,
            &json!({
                "layoutVersion": STATE_LAYOUT_VERSION,
                "backend": SQLITE_V1_BACKEND,
                "futureBackend": TURSO_BACKEND,
                "repoId": self.repo.repo_id,
                "workspaceId": self.workspace.workspace_id,
                "scopeId": self.scope_id,
                "dbPath": self.paths.client_db_path,
                "artifactPath": self.paths.artifacts_dir,
                "generationManifestPath": self.paths.client_cache_manifest_path,
            }),
        )?;

        Ok(())
    }

    /// Render a diagnostic DTO for `asp state locate`.
    pub fn locate_report(&self) -> StateLocateReport {
        StateLocateReport {
            layout_version: STATE_LAYOUT_VERSION.to_string(),
            state_home: self.state_home.clone(),
            repo_id: self.repo.repo_id.clone(),
            workspace_id: self.workspace.workspace_id.clone(),
            scope_id: self.scope_id.clone(),
            repo_display_name: self.repo.display_name.clone(),
            workspace_display_name: self.workspace.display_name.clone(),
            checkout_root: self.workspace.root.clone(),
            git_toplevel: self.repo.git_toplevel.clone(),
            git_dir: self.workspace.git_dir.clone(),
            remote_url: self.repo.remote_url.clone(),
            db_path: self.paths.client_db_path.clone(),
            artifact_path: self.paths.artifacts_dir.clone(),
            manifest_path: self.paths.client_manifest_json.clone(),
            generation_manifest_path: self.paths.client_cache_manifest_path.clone(),
            backend: SQLITE_V1_BACKEND.to_string(),
            future_backend: TURSO_BACKEND.to_string(),
            project_local_cache: self.project_local_cache.clone(),
        }
    }
}

/// Repository identity and the facts used to derive it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoIdentity {
    pub repo_id: RepoId,
    pub display_name: String,
    pub checkout_root: PathBuf,
    pub git_toplevel: Option<PathBuf>,
    pub git_dir: Option<PathBuf>,
    pub git_common_dir: Option<PathBuf>,
    pub remote_url: Option<RemoteUrl>,
    pub identity_basis: String,
}

impl RepoIdentity {
    fn from_checkout(git: &GitIdentity, checkout: &CheckoutIdentity) -> Self {
        let repo_basis = git
            .remote_url
            .as_ref()
            .map(|remote| format!("git-remote:{}", remote.as_str()))
            .or_else(|| {
                git.common_git_dir
                    .as_deref()
                    .map(|git_dir| format!("git-common-dir:{}", path_identity(git_dir)))
            })
            .or_else(|| {
                git.git_dir
                    .as_deref()
                    .map(|git_dir| format!("git-dir:{}", path_identity(git_dir)))
            })
            .unwrap_or_else(|| format!("path:{}", path_identity(&checkout.root)));
        let repo_id = RepoId(stable_id("repo", &repo_basis));

        Self {
            repo_id,
            display_name: checkout.display_name.clone(),
            checkout_root: checkout.root.clone(),
            git_toplevel: git.toplevel.clone(),
            git_dir: git.git_dir.clone(),
            git_common_dir: git.common_git_dir.clone(),
            remote_url: git.remote_url.clone(),
            identity_basis: repo_basis,
        }
    }
}

/// Workspace identity and the facts used to derive it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceIdentity {
    pub workspace_id: WorkspaceId,
    pub display_name: String,
    pub root: PathBuf,
    pub git_dir: Option<PathBuf>,
    pub identity_basis: String,
}

impl WorkspaceIdentity {
    fn from_checkout(git: &GitIdentity, checkout: &CheckoutIdentity, repo_id: &RepoId) -> Self {
        let workspace_basis = format!(
            "repo:{}|checkout:{}|git-dir:{}",
            repo_id.as_str(),
            path_identity(&checkout.root),
            git.git_dir
                .as_deref()
                .map(path_identity)
                .unwrap_or_else(|| "none".to_string())
        );
        let workspace_id = WorkspaceId(stable_id("workspace", &workspace_basis));

        Self {
            workspace_id,
            display_name: checkout.display_name.clone(),
            root: checkout.root.clone(),
            git_dir: git.git_dir.clone(),
            identity_basis: workspace_basis,
        }
    }
}

/// Concrete paths for the State Core v2 layout.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatePaths {
    pub version_file: PathBuf,
    pub state_json: PathBuf,
    pub registry_dir: PathBuf,
    pub registry_events_jsonl: PathBuf,
    pub aliases_by_display_name_dir: PathBuf,
    pub project_dir: PathBuf,
    pub project_json: PathBuf,
    pub workspace_dir: PathBuf,
    pub workspace_json: PathBuf,
    pub client_dir: PathBuf,
    pub client_manifest_json: PathBuf,
    pub client_cache_manifest_path: PathBuf,
    pub client_db_path: PathBuf,
    pub artifacts_dir: PathBuf,
}

impl StatePaths {
    fn new(state_home: &Path, repo_id: &RepoId, workspace_id: &WorkspaceId) -> Self {
        let registry_dir = state_home.join("registry");
        let aliases_by_display_name_dir = state_home.join("aliases").join("by-display-name");
        let project_dir = state_home
            .join("projects")
            .join("by-id")
            .join(repo_id.as_str());
        let workspace_dir = project_dir.join("workspaces").join(workspace_id.as_str());
        let client_dir = workspace_dir.join("live").join("client");
        let artifacts_dir = workspace_dir.join("artifacts");

        Self {
            version_file: state_home.join("VERSION"),
            state_json: state_home.join("state.json"),
            registry_events_jsonl: registry_dir.join("events.jsonl"),
            registry_dir,
            aliases_by_display_name_dir,
            project_json: project_dir.join("project.json"),
            project_dir,
            workspace_json: workspace_dir.join("workspace.json"),
            workspace_dir: workspace_dir.clone(),
            client_manifest_json: client_dir.join(STATE_MANIFEST_FILE),
            client_cache_manifest_path: client_dir.join("cache-manifest.json"),
            client_db_path: client_dir.join(CLIENT_DB_FILE),
            client_dir,
            artifacts_dir,
        }
    }
}

/// Evidence that a project-local v1 cache exists without using it as fallback.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectLocalCacheEvidence {
    pub path: PathBuf,
    pub exists: bool,
}

/// JSON-compatible diagnostic report for `asp state locate`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateLocateReport {
    pub layout_version: String,
    pub state_home: PathBuf,
    pub repo_id: RepoId,
    pub workspace_id: WorkspaceId,
    pub scope_id: ScopeId,
    pub repo_display_name: String,
    pub workspace_display_name: String,
    pub checkout_root: PathBuf,
    pub git_toplevel: Option<PathBuf>,
    pub git_dir: Option<PathBuf>,
    pub remote_url: Option<RemoteUrl>,
    pub db_path: PathBuf,
    pub artifact_path: PathBuf,
    pub manifest_path: PathBuf,
    pub generation_manifest_path: PathBuf,
    pub backend: String,
    pub future_backend: String,
    pub project_local_cache: Option<ProjectLocalCacheEvidence>,
}

/// Resolve the active ASP v2 state root from process environment variables.
pub fn resolve_state_home() -> Result<PathBuf, String> {
    resolve_state_home_from(env::var_os(ASP_STATE_HOME_ENV), env::var_os("HOME"))
}

/// Resolve the ASP v2 state root from explicit environment values.
pub fn resolve_state_home_from(
    asp_state_home: Option<std::ffi::OsString>,
    home: Option<std::ffi::OsString>,
) -> Result<PathBuf, String> {
    if let Some(value) = asp_state_home {
        if value.is_empty() {
            return Err(format!("{ASP_STATE_HOME_ENV} is set but empty"));
        }
        return Ok(canonicalize_parent(PathBuf::from(value)));
    }

    let home = home.ok_or_else(|| "HOME is not set".to_string())?;
    if home.is_empty() {
        return Err("HOME is set but empty".to_string());
    }
    Ok(canonicalize_parent(
        PathBuf::from(home).join(DEFAULT_STATE_HOME_DIR),
    ))
}

/// Resolve state identity and optionally create the minimal v2 layout.
pub fn locate_state(
    cwd: impl AsRef<Path>,
    ensure_layout: bool,
) -> Result<StateLocateReport, String> {
    let state = ResolvedState::resolve(cwd)?;
    if ensure_layout {
        state.ensure_minimal_layout()?;
    }
    Ok(state.locate_report())
}

fn canonicalize_parent(path: PathBuf) -> PathBuf {
    if path.exists() {
        return canonicalize_if_possible(&path);
    }
    match path.parent() {
        Some(parent) => canonicalize_if_possible(parent).join(
            path.file_name()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("")),
        ),
        None => path,
    }
}

fn canonicalize_if_possible(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn path_identity(path: &Path) -> String {
    canonicalize_if_possible(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn stable_id(prefix: &str, value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    let digest = hasher.finalize();
    format!("{prefix}-{:x}", digest)[..(prefix.len() + 1 + 16)].to_string()
}

fn write_if_missing(path: &Path, content: String) -> Result<(), String> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(io_error("create parent dir"))?;
    }
    fs::write(path, content).map_err(io_error("write state file"))
}

fn write_json_if_missing(path: &Path, value: &serde_json::Value) -> Result<(), String> {
    let content = serde_json::to_string_pretty(value)
        .map_err(|error| format!("serialize state json: {error}"))?;
    write_if_missing(path, format!("{content}\n"))
}

fn io_error(action: &'static str) -> impl FnOnce(std::io::Error) -> String {
    move |error| format!("{action}: {error}")
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GitIdentity {
    toplevel: Option<PathBuf>,
    git_dir: Option<PathBuf>,
    common_git_dir: Option<PathBuf>,
    remote_url: Option<RemoteUrl>,
}

impl GitIdentity {
    fn discover(cwd: &Path) -> Self {
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct CheckoutIdentity {
    root: PathBuf,
    display_name: String,
}

impl CheckoutIdentity {
    fn new(cwd: &Path, git: &GitIdentity) -> Self {
        let root = git.toplevel.clone().unwrap_or_else(|| cwd.to_path_buf());
        let root = canonicalize_if_possible(&root);
        let display_name = root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("workspace")
            .to_string();

        Self { root, display_name }
    }
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
