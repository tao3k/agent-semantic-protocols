//! State-root and durable path layout.

use crate::git::canonicalize_if_possible;
use serde::Deserialize;
use serde::Serialize;
use std::env;
use std::path::PathBuf;

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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StateHomeResolutionSource {
    AspStateHome,
    HomeDefault,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateHomeResolution {
    pub state_home: PathBuf,
    pub source: StateHomeResolutionSource,
    pub asp_state_home_present: bool,
    pub home_present: bool,
}

/// Resolve the active ASP v1 state root from process environment variables.
pub fn resolve_state_home() -> Result<PathBuf, String> {
    Ok(resolve_state_home_projection()?.state_home)
}

pub fn resolve_state_home_projection() -> Result<StateHomeResolution, String> {
    resolve_state_home_projection_from(env::var_os(ASP_STATE_HOME_ENV), env::var_os("HOME"))
}

/// Resolve the ASP v2 state root from explicit environment values.
pub fn resolve_state_home_from(
    asp_state_home: Option<std::ffi::OsString>,
    home: Option<std::ffi::OsString>,
) -> Result<PathBuf, String> {
    Ok(resolve_state_home_projection_from(asp_state_home, home)?.state_home)
}

pub fn resolve_state_home_projection_from(
    asp_state_home: Option<std::ffi::OsString>,
    home: Option<std::ffi::OsString>,
) -> Result<StateHomeResolution, String> {
    let asp_state_home_present = asp_state_home.is_some();
    let home_present = home.is_some();
    if let Some(value) = asp_state_home {
        if value.is_empty() {
            return Err(format!("{ASP_STATE_HOME_ENV} is set but empty"));
        }
        return Ok(StateHomeResolution {
            state_home: canonicalize_parent(PathBuf::from(value)),
            source: StateHomeResolutionSource::AspStateHome,
            asp_state_home_present,
            home_present,
        });
    }

    let home = home.ok_or_else(|| "HOME is not set".to_string())?;
    if home.is_empty() {
        return Err("HOME is set but empty".to_string());
    }
    Ok(StateHomeResolution {
        state_home: canonicalize_parent(PathBuf::from(home).join(DEFAULT_STATE_HOME_DIR)),
        source: StateHomeResolutionSource::HomeDefault,
        asp_state_home_present,
        home_present,
    })
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
