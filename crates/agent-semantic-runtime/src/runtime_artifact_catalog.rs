//! Immutable runtime artifact catalog loaded once by the Tokio supervisor.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use agent_semantic_config::runtime_dev::{
    ArtifactOrigin, RuntimeArtifactMode, parse_runtime_artifact_mode,
};

/// Artifact provenance required for runtime catalog admission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeArtifactReceipt {
    /// Installation origin declared by the installer receipt.
    pub origin: ArtifactOrigin,
    /// Canonical checkout root for development-workspace artifacts.
    pub checkout_root: Option<PathBuf>,
}

/// Immutable artifact mode shared by every workspace session in one daemon.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeArtifactCatalog {
    mode: RuntimeArtifactMode,
    provider_catalog_generation: Option<String>,
}

impl RuntimeArtifactCatalog {
    /// Construct a catalog from an already validated runtime mode.
    #[must_use]
    pub const fn new(mode: RuntimeArtifactMode) -> Self {
        Self {
            mode,
            provider_catalog_generation: None,
        }
    }

    /// Bind the immutable provider catalog consumed by this daemon process.
    #[must_use]
    pub fn with_provider_catalog_generation(mut self, generation: impl Into<String>) -> Self {
        self.provider_catalog_generation = Some(generation.into());
        self
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
        if let Some(generation) = &self.provider_catalog_generation {
            hasher.update(b"\0provider-catalog\0");
            hasher.update(generation.as_bytes());
        }
        format!("blake3-256:{}", hasher.finalize().to_hex())
    }

    /// Admit a receipt only when both origin and checkout identity match.
    #[must_use]
    pub fn admits(&self, receipt: &RuntimeArtifactReceipt) -> bool {
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

/// Load one immutable catalog generation from `$ASP_STATE_HOME/asp.toml`.
///
/// The supervisor owns this asynchronous read. Search and query consumers keep
/// the returned value in memory and never call this function.
pub async fn load_runtime_artifact_catalog(
    state_home: &Path,
) -> Result<RuntimeArtifactCatalog, String> {
    let config_path = state_home.join("asp.toml");
    let input = match tokio::fs::read_to_string(&config_path).await {
        Ok(input) => input,
        Err(error) if error.kind() == ErrorKind::NotFound => String::new(),
        Err(error) => {
            return Err(format!(
                "read runtime configuration {}: {error}",
                config_path.display()
            ));
        }
    };
    let mode = parse_runtime_artifact_mode(&input)?;
    let mode = match mode {
        RuntimeArtifactMode::Dev { root } => {
            let root = tokio::fs::canonicalize(&root).await.map_err(|error| {
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
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct ProviderCatalogIdentity {
        catalog_generation: String,
    }

    let provider_catalog_path = state_home.join("runtime/provider-catalog.v1.json");
    let provider_catalog = match tokio::fs::read(&provider_catalog_path).await {
        Ok(bytes) => {
            let identity: ProviderCatalogIdentity =
                serde_json::from_slice(&bytes).map_err(|error| {
                    format!(
                        "parse runtime provider catalog identity {}: {error}",
                        provider_catalog_path.display()
                    )
                })?;
            if identity.catalog_generation.trim().is_empty() {
                return Err("runtime provider catalog generation must be non-empty".to_owned());
            }
            Some(identity.catalog_generation)
        }
        Err(error) if error.kind() == ErrorKind::NotFound => None,
        Err(error) => {
            return Err(format!(
                "read runtime provider catalog identity {}: {error}",
                provider_catalog_path.display()
            ));
        }
    };
    let catalog = RuntimeArtifactCatalog::new(mode);
    Ok(match provider_catalog {
        Some(generation) => catalog.with_provider_catalog_generation(generation),
        None => catalog,
    })
}
