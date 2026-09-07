// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Typed runtime development configuration and artifact-origin admission.

use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;

/// Process-wide Hook engine policy from the State Home `asp.toml` contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HookEngineConfig {
    enabled: bool,
}

impl HookEngineConfig {
    #[must_use]
    pub const fn enabled(self) -> bool {
        self.enabled
    }
}

/// One parsed generation of the global ASP configuration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AspGlobalConfig {
    runtime_artifact_mode: RuntimeArtifactMode,
    hook_engine: HookEngineConfig,
}

impl AspGlobalConfig {
    #[must_use]
    pub const fn runtime_artifact_mode(&self) -> &RuntimeArtifactMode {
        &self.runtime_artifact_mode
    }

    #[must_use]
    pub const fn hook_engine(&self) -> HookEngineConfig {
        self.hook_engine
    }
}

/// Runtime artifact source selected by the State Home `asp.toml` contract.
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
#[serde(deny_unknown_fields)]
struct HookEngineSection {
    enabled: bool,
}

#[derive(Debug, Deserialize)]
struct AspConfigDocument {
    dev: Option<DevSection>,
    #[serde(rename = "hook-engine")]
    hook_engine: Option<HookEngineSection>,
}

/// Parse one immutable generation of the State Home `asp.toml` contract.
pub fn parse_asp_global_config(input: &str) -> Result<AspGlobalConfig, String> {
    let document: AspConfigDocument =
        toml::from_str(input).map_err(|error| format!("parse global asp.toml: {error}"))?;
    let runtime_artifact_mode = match document.dev {
        None => Ok(RuntimeArtifactMode::Release),
        Some(DevSection { enabled: false, .. }) => Ok(RuntimeArtifactMode::Release),
        Some(DevSection {
            enabled: true,
            root,
        }) => validate_dev_root(root),
    }?;
    let hook_engine = HookEngineConfig {
        enabled: document.hook_engine.map_or(true, |section| section.enabled),
    };
    Ok(AspGlobalConfig {
        runtime_artifact_mode,
        hook_engine,
    })
}

/// Parse only the artifact mode while retaining one global document grammar.
pub fn parse_runtime_artifact_mode(input: &str) -> Result<RuntimeArtifactMode, String> {
    parse_asp_global_config(input).map(|config| config.runtime_artifact_mode)
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
