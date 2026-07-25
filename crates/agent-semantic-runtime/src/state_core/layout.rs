//! State-root and durable path layout.

use super::identity::{RepoId, WorkspaceId};
use crate::git::canonicalize_if_possible;
use serde::{Deserialize, Serialize};
use std::{
    env,
    path::{Path, PathBuf},
};

/// Environment variable that overrides the ASP v2 state root.
pub const ASP_STATE_HOME_ENV: &str = "ASP_STATE_HOME";
/// Default directory under `HOME` for ASP v1 durable state.
pub const DEFAULT_STATE_HOME_DIR: &str = ".agent-semantic-protocols";
/// Layout version for global ASP state directories.
pub const STATE_LAYOUT_VERSION: &str = "state-v1";
/// Initial scope identity used before multiple named scopes exist.
pub const DEFAULT_SCOPE_ID: &str = "default";
/// Active DB backend recorded in State Core manifests.
pub const TURSO_BACKEND: &str = "turso";
/// Turso client DB filename under `live/client`.
pub const CLIENT_DB_FILE: &str = "facts.turso";
/// State Core client manifest filename under `live/client`.
pub const STATE_MANIFEST_FILE: &str = "manifest.json";

/// Concrete paths for the State Core v1 layout.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatePaths {
    pub version_file: PathBuf,
    pub state_json: PathBuf,
    pub registry_dir: PathBuf,
    pub registry_events_jsonl: PathBuf,
    pub aliases_by_display_name_dir: PathBuf,
    #[serde(default)]
    pub projects_by_id_dir: PathBuf,
    pub project_dir: PathBuf,
    pub project_json: PathBuf,
    #[serde(default)]
    pub project_client_dir: PathBuf,
    #[serde(default)]
    pub project_client_db_path: PathBuf,
    pub workspace_dir: PathBuf,
    pub workspace_json: PathBuf,
    pub hooks_dir: PathBuf,
    pub client_dir: PathBuf,
    pub client_manifest_json: PathBuf,
    pub client_cache_manifest_path: PathBuf,
    pub client_db_path: PathBuf,
    pub artifacts_dir: PathBuf,
}

impl StatePaths {
    /// Project already-resolved repository and workspace identities into the
    /// canonical State Core path layout.
    pub fn new(state_home: &Path, repo_id: &RepoId, workspace_id: &WorkspaceId) -> Self {
        let registry_dir = state_home.join("registry");
        let aliases_by_display_name_dir = state_home.join("aliases").join("by-display-name");
        let projects_by_id_dir = state_home.join("projects").join("by-id");
        let project_dir = projects_by_id_dir.join(repo_id.as_str());
        let project_client_dir = project_dir.join("live").join("client");
        let workspace_dir = project_dir.join("workspaces").join(workspace_id.as_str());
        let hooks_dir = workspace_dir.join("hooks");
        let client_dir = workspace_dir.join("live").join("client");
        let artifacts_dir = workspace_dir.join("artifacts");

        Self {
            version_file: state_home.join("VERSION"),
            state_json: state_home.join("state.json"),
            registry_events_jsonl: registry_dir.join("events.jsonl"),
            registry_dir,
            aliases_by_display_name_dir,
            projects_by_id_dir,
            project_json: project_dir.join("project.json"),
            project_dir,
            project_client_db_path: project_client_dir.join(CLIENT_DB_FILE),
            project_client_dir,
            workspace_json: workspace_dir.join("workspace.json"),
            workspace_dir: workspace_dir.clone(),
            hooks_dir,
            client_manifest_json: client_dir.join(STATE_MANIFEST_FILE),
            client_cache_manifest_path: client_dir.join("cache-manifest.json"),
            client_db_path: client_dir.join(CLIENT_DB_FILE),
            client_dir,
            artifacts_dir,
        }
    }
}

/// Resolve the active ASP v1 state root from process environment variables.
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

pub(super) fn canonicalize_parent(path: PathBuf) -> PathBuf {
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
