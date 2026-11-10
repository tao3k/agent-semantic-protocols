// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Immutable runtime artifact catalog owned by the Artifacts package.

use std::path::Path;
use std::path::PathBuf;

pub fn runtime_artifact_source_generation(source_path: &Path) -> Result<String, String> {
    let source_path = std::fs::canonicalize(source_path).map_err(|error| {
        format!(
            "resolve Runtime artifact source generation {}: {error}",
            source_path.display()
        )
    })?;
    let metadata = std::fs::metadata(&source_path).map_err(|error| {
        format!(
            "inspect Runtime artifact source generation {}: {error}",
            source_path.display()
        )
    })?;
    if !metadata.is_file() {
        return Err(format!(
            "Runtime artifact source generation is not a file: {}",
            source_path.display()
        ));
    }
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"agent.semantic-protocols.filesystem-generation.v1\0");
    hasher.update(source_path.to_string_lossy().as_bytes());
    hasher.update(&metadata.len().to_le_bytes());
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;

        hasher.update(&metadata.dev().to_le_bytes());
        hasher.update(&metadata.ino().to_le_bytes());
        hasher.update(&metadata.mtime().to_le_bytes());
        hasher.update(&metadata.mtime_nsec().to_le_bytes());
        hasher.update(&metadata.ctime().to_le_bytes());
        hasher.update(&metadata.ctime_nsec().to_le_bytes());
    }
    #[cfg(not(unix))]
    {
        let modified = metadata
            .modified()
            .map_err(|error| format!("read source modification time: {error}"))?
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| format!("source modification time predates epoch: {error}"))?;
        hasher.update(&modified.as_nanos().to_le_bytes());
    }
    Ok(format!("blake3-256:{}", hasher.finalize().to_hex()))
}

use agent_semantic_config::runtime_dev::ArtifactOrigin;
use agent_semantic_config::runtime_dev::RuntimeArtifactMode;

/// Artifact provenance required for runtime catalog admission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeArtifactReceipt {
    /// Installation origin declared by the installer receipt.
    pub origin: ArtifactOrigin,
    /// Canonical checkout root for development-workspace artifacts.
    pub checkout_root: Option<PathBuf>,
    /// Executable identity admitted by the Runtime Artifact Authority v1.
    pub reference: RuntimeArtifactReference,
}

/// Explicit authority for a qualified artifact whose final launcher is
/// materialized in State Home from a development-workspace build.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QualifiedRuntimeArtifactSource {
    checkout_root: PathBuf,
}

impl QualifiedRuntimeArtifactSource {
    pub fn develop_state_home_staging(checkout_root: PathBuf) -> Result<Self, String> {
        if !checkout_root.is_absolute() {
            return Err("qualified Runtime artifact checkout root must be absolute".to_owned());
        }
        Ok(Self { checkout_root })
    }

    pub fn validate_source(
        &self,
        state_home: &Path,
        source: &Path,
        artifact_kind: &str,
    ) -> Result<(PathBuf, PathBuf), String> {
        let configured_root = load_runtime_developer_root(state_home)?
            .ok_or_else(|| "qualified development artifact requires Runtime dev mode".to_owned())?;
        let checkout_root = std::fs::canonicalize(&self.checkout_root).map_err(|error| {
            format!(
                "canonicalize qualified Runtime checkout root {}: {error}",
                self.checkout_root.display()
            )
        })?;
        if checkout_root != configured_root {
            return Err(format!(
                "qualified Runtime artifact checkout authority drift: expected={} actual={}",
                configured_root.display(),
                checkout_root.display()
            ));
        }
        let source_identity = std::fs::canonicalize(source).map_err(|error| {
            format!(
                "canonicalize qualified Runtime artifact source {}: {error}",
                source.display()
            )
        })?;
        let staging_root = crate::RuntimeArtifactStateLayout::new(state_home)
            .provider_staging()
            .join(artifact_kind);
        if !source_identity.starts_with(&staging_root) {
            return Err(format!(
                "qualified Runtime artifact source escapes provider staging: source={} stagingRoot={}",
                source_identity.display(),
                staging_root.display()
            ));
        }
        Ok((checkout_root, source_identity))
    }
}

pub const RUNTIME_ARTIFACT_REFERENCE_SCHEMA_ID: &str =
    "agent.semantic-protocols.runtime-artifact-reference";
pub const RUNTIME_ARTIFACT_REFERENCE_SCHEMA_VERSION: u64 = 1;

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "identity")]
pub enum RuntimeBinaryIdentity {
    Content {
        digest: crate::blake3_content_digest::Blake3ContentDigest,
    },
}

impl RuntimeBinaryIdentity {
    pub fn from_content_digest(digest: crate::blake3_content_digest::Blake3ContentDigest) -> Self {
        Self::Content { digest }
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self::from_content_digest(
            crate::blake3_content_digest::Blake3ContentDigest::from_bytes(bytes),
        )
    }

    pub fn kind(&self) -> &'static str {
        "content"
    }

    pub fn algorithm(&self) -> &str {
        "blake3-256"
    }

    pub fn content_digest(&self) -> &crate::blake3_content_digest::Blake3ContentDigest {
        let Self::Content { digest } = self;
        digest
    }
}

/// The single executable identity consumed by Runtime, Hook, and providers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeArtifactReference {
    pub schema_id: String,
    pub schema_version: u64,
    pub artifact_kind: String,
    pub origin: ArtifactOrigin,
    pub executable_path: PathBuf,
    pub content_digest: crate::blake3_content_digest::Blake3ContentDigest,
    pub identity: RuntimeBinaryIdentity,
    pub checkout_root: Option<PathBuf>,
}

impl RuntimeArtifactReference {
    #[must_use]
    pub fn new(
        artifact_kind: impl Into<String>,
        origin: ArtifactOrigin,
        executable_path: PathBuf,
        content_digest: crate::blake3_content_digest::Blake3ContentDigest,
        checkout_root: Option<PathBuf>,
    ) -> Self {
        Self {
            schema_id: RUNTIME_ARTIFACT_REFERENCE_SCHEMA_ID.to_owned(),
            schema_version: RUNTIME_ARTIFACT_REFERENCE_SCHEMA_VERSION,
            artifact_kind: artifact_kind.into(),
            origin,
            executable_path,
            content_digest: content_digest.clone(),
            identity: RuntimeBinaryIdentity::Content {
                digest: content_digest.clone(),
            },
            checkout_root,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != RUNTIME_ARTIFACT_REFERENCE_SCHEMA_ID {
            return Err(format!(
                "runtime artifact reference schema drift: expected={} actual={}",
                RUNTIME_ARTIFACT_REFERENCE_SCHEMA_ID, self.schema_id
            ));
        }
        if self.schema_version != RUNTIME_ARTIFACT_REFERENCE_SCHEMA_VERSION {
            return Err(format!(
                "runtime artifact reference version drift: expected={} actual={}",
                RUNTIME_ARTIFACT_REFERENCE_SCHEMA_VERSION, self.schema_version
            ));
        }
        if self.artifact_kind.trim().is_empty() {
            return Err("runtime artifact reference kind must be non-empty".to_owned());
        }
        if !self.executable_path.is_absolute() {
            return Err(format!(
                "runtime artifact executable path must be absolute: {}",
                self.executable_path.display()
            ));
        }
        let RuntimeBinaryIdentity::Content { digest } = &self.identity;
        if digest != &self.content_digest {
            return Err(format!(
                "runtime artifact identity drift: referenceDigest={} identityDigest={digest}",
                self.content_digest
            ));
        }
        Ok(())
    }
}

/// Immutable artifact mode shared by every workspace session in one daemon.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeArtifactCatalog {
    mode: RuntimeArtifactMode,
    runtime_bundle_digest: Option<String>,
    active_provider_targets: Vec<(String, String)>,
}

impl RuntimeArtifactCatalog {
    pub fn admit_reference(
        &self,
        reference: &RuntimeArtifactReference,
        runtime_root: &Path,
    ) -> Result<(), String> {
        reference.validate()?;
        match &self.mode {
            RuntimeArtifactMode::Dev { root } => {
                if reference.origin != ArtifactOrigin::DevelopWorkspace {
                    return Err("development runtime rejects non-development artifact".to_owned());
                }
                if reference.checkout_root.as_deref() != Some(root.as_path()) {
                    return Err("development artifact checkout root drift".to_owned());
                }
                let stable_root = runtime_root.join("bin");
                if !reference.executable_path.starts_with(&stable_root) {
                    return Err(format!(
                        "development artifact executable is outside stable runtime slots: {}",
                        reference.executable_path.display()
                    ));
                }
                if reference
                    .executable_path
                    .starts_with(runtime_root.join("artifacts"))
                {
                    return Err("generation-store path cannot be a stable executable".to_owned());
                }
            }
            RuntimeArtifactMode::Release => {
                if reference.origin != ArtifactOrigin::LockedRelease {
                    return Err("release runtime rejects non-release artifact".to_owned());
                }
                if reference.checkout_root.is_some() {
                    return Err("release artifact must not retain a checkout root".to_owned());
                }
                let stable_root = runtime_root.join("bin");
                if !reference.executable_path.starts_with(&stable_root) {
                    return Err(format!(
                        "release artifact executable is outside stable runtime slots: {}",
                        reference.executable_path.display()
                    ));
                }
                if reference
                    .executable_path
                    .starts_with(runtime_root.join("artifacts"))
                {
                    return Err("generation-store path cannot be a stable executable".to_owned());
                }
            }
        }
        Ok(())
    }

    /// Construct a catalog from an already validated runtime mode.
    #[must_use]
    pub const fn new(mode: RuntimeArtifactMode) -> Self {
        Self {
            mode,
            runtime_bundle_digest: None,
            active_provider_targets: Vec::new(),
        }
    }

    /// Bind the verified active Runtime bundle consumed by this daemon.
    #[must_use]
    pub fn with_runtime_bundle_digest(mut self, generation: impl Into<String>) -> Self {
        self.runtime_bundle_digest = Some(generation.into());
        self
    }

    #[must_use]
    pub fn runtime_bundle_digest(&self) -> Option<&str> {
        self.runtime_bundle_digest.as_deref()
    }

    #[must_use]
    pub fn active_provider_targets(&self) -> &[(String, String)] {
        &self.active_provider_targets
    }

    /// Return the immutable mode captured for this catalog generation.
    #[must_use]
    pub const fn mode(&self) -> &RuntimeArtifactMode {
        &self.mode
    }

    /// Stable mode label carried by the Runtime Server endpoint contract.
    #[must_use]
    pub const fn mode_label(&self) -> &'static str {
        match self.mode {
            RuntimeArtifactMode::Dev { .. } => "dev",
            RuntimeArtifactMode::Release => "release",
        }
    }

    /// Content identity for the immutable catalog generation loaded by the
    /// daemon. The configured canonical checkout is part of dev identity.
    #[must_use]
    pub fn digest(&self) -> String {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"agent.semantic-protocols.runtime-artifact-catalog.v1\0");
        hasher.update(self.mode_label().as_bytes());
        if let RuntimeArtifactMode::Dev { root } = &self.mode {
            hasher.update(b"\0");
            hasher.update(root.to_string_lossy().as_bytes());
        }
        if let Some(generation) = &self.runtime_bundle_digest {
            hasher.update(b"\0runtime-bundle\0");
            hasher.update(generation.as_bytes());
        }
        format!("blake3-256:{}", hasher.finalize().to_hex())
    }

    /// Admit a receipt only when both origin and checkout identity match.
    #[must_use]
    pub fn admits(&self, receipt: &RuntimeArtifactReceipt) -> bool {
        if receipt.origin != receipt.reference.origin
            || receipt.checkout_root != receipt.reference.checkout_root
            || receipt.reference.validate().is_err()
        {
            return false;
        }
        if !self.mode.admits(receipt.origin) {
            return false;
        }
        match (&self.mode, &receipt.checkout_root) {
            (RuntimeArtifactMode::Dev { root, .. }, Some(receipt_root)) => root == receipt_root,
            (RuntimeArtifactMode::Release, None) => true,
            _ => false,
        }
    }
}

/// Load one immutable catalog generation from the typed control-plane config.
///
/// The supervisor owns this asynchronous read. Search and query consumers keep
/// the returned value in memory and never call this function.
/// Load only the Runtime Artifact Authority mode for synchronous publishers.
pub fn load_runtime_artifact_mode(state_home: &Path) -> Result<RuntimeArtifactMode, String> {
    match crate::load_asp_global_config(state_home)?.runtime_artifact_mode() {
        RuntimeArtifactMode::Dev { root } => {
            let root = std::fs::canonicalize(root).map_err(|error| {
                format!(
                    "canonicalize runtime [dev].root {}: {error}",
                    root.display()
                )
            })?;
            let metadata = std::fs::metadata(&root).map_err(|error| {
                format!("inspect runtime [dev].root {}: {error}", root.display())
            })?;
            if !metadata.is_dir() {
                return Err(format!(
                    "runtime [dev].root must be a directory: {}",
                    root.display()
                ));
            }
            Ok(RuntimeArtifactMode::Dev { root })
        }
        RuntimeArtifactMode::Release => Ok(RuntimeArtifactMode::Release),
    }
}

/// Resolve the configured Developer Source root without exposing mode internals.
pub fn load_runtime_developer_root(state_home: &Path) -> Result<Option<PathBuf>, String> {
    match load_runtime_artifact_mode(state_home)? {
        RuntimeArtifactMode::Dev { root } => Ok(Some(root)),
        RuntimeArtifactMode::Release => Ok(None),
    }
}

pub async fn load_runtime_artifact_catalog(
    state_home: &Path,
) -> Result<RuntimeArtifactCatalog, String> {
    let config = crate::load_asp_global_config_async(state_home).await?;
    let mode = match config.runtime_artifact_mode() {
        RuntimeArtifactMode::Dev { root } => {
            let root = tokio::fs::canonicalize(root).await.map_err(|error| {
                format!(
                    "canonicalize runtime [dev].root {}: {error}",
                    root.display()
                )
            })?;
            let metadata = tokio::fs::metadata(&root).await.map_err(|error| {
                format!("inspect runtime [dev].root {}: {error}", root.display())
            })?;
            if !metadata.is_dir() {
                return Err(format!(
                    "runtime [dev].root must be a directory: {}",
                    root.display()
                ));
            }
            RuntimeArtifactMode::Dev { root }
        }
        RuntimeArtifactMode::Release => RuntimeArtifactMode::Release,
    };
    let mut catalog = RuntimeArtifactCatalog::new(mode);
    let providers =
        match crate::runtime_active_provider_set::load_active_runtime_bound_provider_set_async(
            state_home,
        )
        .await
        {
            Ok(providers) => providers,
            Err(error) if error.contains("runtime-active-generation-unavailable") => {
                return Ok(catalog);
            }
            Err(error) => return Err(error),
        };
    catalog.active_provider_targets = providers
        .providers
        .into_iter()
        .map(|provider| (provider.language_id, provider.provider_id))
        .collect();
    Ok(catalog.with_runtime_bundle_digest(providers.runtime_bundle_digest))
}
