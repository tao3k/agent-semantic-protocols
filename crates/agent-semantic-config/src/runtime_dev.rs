//! Typed runtime development configuration and artifact-origin admission.

use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;

/// Runtime artifact source selected by the state-home `asp.toml` document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeArtifactMode {
    /// Only release-lock artifacts are eligible.
    Release,
    /// Only artifacts built from the configured checkout are eligible.
    Dev { root: PathBuf },
}

/// Provenance carried by an installed ASP or language-provider artifact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtifactOrigin {
    /// Root development installer receipt (`develop-workspace`).
    DevelopWorkspace,
    /// Pinned release-lock receipt (`locked-release`).
    LockedRelease,
    /// Unowned executable resolved from `PATH`.
    PathFallback,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DevSection {
    enabled: bool,
    root: PathBuf,
}

#[derive(Debug, Deserialize)]
struct AspConfigDocument {
    dev: Option<DevSection>,
}

/// Parse the artifact mode from one state-home `asp.toml` document.
pub fn parse_runtime_artifact_mode(input: &str) -> Result<RuntimeArtifactMode, String> {
    let document: AspConfigDocument =
        toml::from_str(input).map_err(|error| format!("parse asp.toml dev model: {error}"))?;
    match document.dev {
        None => Ok(RuntimeArtifactMode::Release),
        Some(DevSection { enabled: false, .. }) => Ok(RuntimeArtifactMode::Release),
        Some(DevSection {
            enabled: true,
            root,
        }) => validate_dev_root(root),
    }
}

fn validate_dev_root(root: PathBuf) -> Result<RuntimeArtifactMode, String> {
    if !root.is_absolute() {
        return Err("asp.toml [dev].root must be an absolute path".to_string());
    }
    if root == Path::new("/") {
        return Err("asp.toml [dev].root cannot be the filesystem root".to_string());
    }
    Ok(RuntimeArtifactMode::Dev { root })
}

impl RuntimeArtifactMode {
    /// Decide whether an artifact origin belongs to this mutually exclusive mode.
    #[must_use]
    pub const fn admits(&self, origin: ArtifactOrigin) -> bool {
        matches!(
            (self, origin),
            (Self::Dev { .. }, ArtifactOrigin::DevelopWorkspace)
                | (Self::Release, ArtifactOrigin::LockedRelease)
        )
    }
}
