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
    /// Executable identity admitted by the Runtime Artifact Authority v1.
    pub reference: RuntimeArtifactReference,
}

pub const RUNTIME_ARTIFACT_REFERENCE_SCHEMA_ID: &str =
    "agent.semantic-protocols.runtime-artifact-reference.v1";
pub const RUNTIME_ARTIFACT_REFERENCE_SCHEMA_VERSION: u64 = 1;

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "identity")]
pub enum RuntimeBinaryIdentity {
    Content { value: String, algorithm: String },
    DeveloperSourceGeneration { value: String, algorithm: String },
}

impl RuntimeBinaryIdentity {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Content { .. } => "content",
            Self::DeveloperSourceGeneration { .. } => "developer-source-generation",
        }
    }

    pub fn algorithm(&self) -> &str {
        match self {
            Self::Content { algorithm, .. }
            | Self::DeveloperSourceGeneration { algorithm, .. } => algorithm,
        }
    }

    pub fn value(&self) -> &str {
        match self {
            Self::Content { value, .. } | Self::DeveloperSourceGeneration { value, .. } => value,
        }
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
    pub content_digest: String,
    pub identity: RuntimeBinaryIdentity,
    pub checkout_root: Option<PathBuf>,
}

impl RuntimeArtifactReference {
    #[must_use]
    pub fn new(
        artifact_kind: impl Into<String>,
        origin: ArtifactOrigin,
        executable_path: PathBuf,
        content_digest: impl Into<String>,
        checkout_root: Option<PathBuf>,
    ) -> Self {
        let content_digest = content_digest.into();
        Self {
            schema_id: RUNTIME_ARTIFACT_REFERENCE_SCHEMA_ID.to_owned(),
            schema_version: RUNTIME_ARTIFACT_REFERENCE_SCHEMA_VERSION,
            artifact_kind: artifact_kind.into(),
            origin,
            executable_path,
            content_digest: content_digest.clone(),
            identity: RuntimeBinaryIdentity::Content {
                value: content_digest,
                algorithm: "blake3-256".to_owned(),
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
        if matches!(self.identity, RuntimeBinaryIdentity::Content { .. }) {
            let Some(digest_hex) = self.content_digest.strip_prefix("blake3-256:") else {
                return Err("runtime artifact content digest must use blake3-256".to_owned());
            };
            if digest_hex.len() != 64 || !digest_hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err("runtime artifact content digest must contain 64 hexadecimal digits".to_owned());
            }
        }
        Ok(())
    }
}

/// Immutable artifact mode shared by every workspace session in one daemon.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeArtifactCatalog {
    mode: RuntimeArtifactMode,
    provider_catalog_generation: Option<String>,
}

/// Filesystem-independent plan for Developer-mode executable publication.
#[derive(Clone, Debug, Eq, PartialEq)]
struct DeveloperArtifactPublicationPlan {
    pub source_identity: PathBuf,
    pub target: PathBuf,
    pub artifact_digest: String,
    pub reference: RuntimeArtifactReference,
}

fn runtime_artifact_content_digest(path: &Path) -> Result<String, String> {
    use std::io::Read as _;

    let mut file = std::fs::File::open(path).map_err(|error| {
        format!(
            "failed to open runtime artifact {}: {error}",
            path.display()
        )
    })?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|error| {
            format!(
                "failed to read runtime artifact {}: {error}",
                path.display()
            )
        })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeArtifactPublication {
    pub path: PathBuf,
    pub status: &'static str,
    pub artifact_digest: String,
    pub reference: RuntimeArtifactReference,
    pub identity: RuntimeBinaryIdentity,
}

/// Publishes one Runtime artifact according to the state-home authority.
///
/// Developer mode publishes a direct stable link to the verified checkout build
/// output and never creates a digest generation. Release mode stages one verified
/// digest generation, atomically moves the stable slot, then removes generations
/// that are unreachable from Runtime-owned slots.
pub async fn publish_runtime_artifact(
    state_home: &Path,
    source: &Path,
    target: &Path,
    artifact_root: &Path,
    artifact_kind: impl Into<String>,
) -> Result<RuntimeArtifactPublication, String> {
    let state_home = state_home.to_path_buf();
    let source = source.to_path_buf();
    let target = target.to_path_buf();
    let artifact_root = artifact_root.to_path_buf();
    let artifact_kind = artifact_kind.into();
    tokio::task::spawn_blocking(move || {
        publish_runtime_artifact_blocking(
            &state_home,
            &source,
            &target,
            &artifact_root,
            artifact_kind,
        )
    })
    .await
    .map_err(|error| format!("runtime artifact publication task failed: {error}"))?
}

fn publish_runtime_artifact_blocking(
    state_home: &Path,
    source: &Path,
    target: &Path,
    artifact_root: &Path,
    artifact_kind: String,
) -> Result<RuntimeArtifactPublication, String> {
    if let Some(developer_root) = load_runtime_developer_root(state_home)? {
        let plan = prepare_developer_artifact_publication_blocking(
            &developer_root,
            source,
            target,
            artifact_kind,
        )?;
        return publish_developer_artifact(plan, artifact_root);
    }
    publish_release_artifact(source, target, artifact_root, artifact_kind)
}

fn publish_developer_artifact(
    plan: DeveloperArtifactPublicationPlan,
    _artifact_root: &Path,
) -> Result<RuntimeArtifactPublication, String> {
    let target_is_current = std::fs::canonicalize(&plan.target)
        .ok()
        .is_some_and(|identity| identity == plan.source_identity);
    let status = if target_is_current {
        "current"
    } else {
        let parent = plan.target.parent().ok_or_else(|| {
            format!(
                "development runtime artifact target has no parent: {}",
                plan.target.display()
            )
        })?;
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        let staged = temporary_runtime_artifact_path(&plan.target);
        remove_stale_staged_artifact(&staged)?;
        stage_runtime_artifact_link(&plan.source_identity, &staged)?;
        let status = if std::fs::symlink_metadata(&plan.target).is_ok() {
            "updated"
        } else {
            "installed"
        };
        atomic_replace_runtime_artifact(&staged, &plan.target)?;
        status
    };
    let published_identity = std::fs::canonicalize(&plan.target).map_err(|error| {
        format!(
            "failed to resolve published development runtime artifact {}: {error}",
            plan.target.display()
        )
    })?;
    if published_identity != plan.source_identity {
        return Err(format!(
            "development runtime artifact publication drift: expected={} actual={}",
            plan.source_identity.display(),
            published_identity.display()
        ));
    }
    Ok(RuntimeArtifactPublication {
        path: plan.target,
        status,
        artifact_digest: plan.artifact_digest,
        identity: plan.reference.identity.clone(),
        reference: plan.reference,
    })
}

fn publish_release_artifact(
    source: &Path,
    target: &Path,
    artifact_root: &Path,
    artifact_kind: String,
) -> Result<RuntimeArtifactPublication, String> {
    if !target.is_absolute() || !artifact_root.is_absolute() {
        return Err(format!(
            "release runtime artifact publication requires absolute paths: target={} artifactRoot={}",
            target.display(),
            artifact_root.display()
        ));
    }
    let source_identity = std::fs::canonicalize(source).map_err(|error| {
        format!(
            "failed to resolve runtime artifact {}: {error}",
            source.display()
        )
    })?;
    if !source_identity.is_file() {
        return Err(format!(
            "runtime artifact source is not a regular file: {}",
            source_identity.display()
        ));
    }
    let content_digest = runtime_artifact_content_digest(&source_identity)?;
    let file_name = target.file_name().ok_or_else(|| {
        format!(
            "release runtime artifact target has no file name: {}",
            target.display()
        )
    })?;
    let artifact_path = artifact_root
        .join("blake3-256")
        .join(&content_digest)
        .join(file_name);
    if !artifact_path.is_file() {
        let artifact_parent = artifact_path.parent().ok_or_else(|| {
            format!(
                "release runtime artifact has no parent: {}",
                artifact_path.display()
            )
        })?;
        std::fs::create_dir_all(artifact_parent)
            .map_err(|error| format!("failed to create {}: {error}", artifact_parent.display()))?;
        let staged = temporary_runtime_artifact_path(&artifact_path);
        remove_stale_staged_artifact(&staged)?;
        std::fs::copy(&source_identity, &staged).map_err(|error| {
            format!(
                "failed to stage release runtime artifact {}: {error}",
                staged.display()
            )
        })?;
        let permissions = std::fs::metadata(&source_identity)
            .map_err(|error| format!("failed to inspect {}: {error}", source_identity.display()))?
            .permissions();
        std::fs::set_permissions(&staged, permissions)
            .map_err(|error| format!("failed to chmod {}: {error}", staged.display()))?;
        let staged_digest = runtime_artifact_content_digest(&staged)?;
        if staged_digest != content_digest {
            let _ = std::fs::remove_file(&staged);
            return Err(format!(
                "release runtime artifact digest drift: expected={content_digest} actual={staged_digest}"
            ));
        }
        atomic_replace_runtime_artifact(&staged, &artifact_path)?;
    }
    let artifact_identity = std::fs::canonicalize(&artifact_path).map_err(|error| {
        format!(
            "failed to resolve release runtime artifact {}: {error}",
            artifact_path.display()
        )
    })?;
    let target_is_current = std::fs::canonicalize(target)
        .ok()
        .is_some_and(|identity| identity == artifact_identity);
    let status = if target_is_current {
        "current"
    } else {
        let parent = target.parent().ok_or_else(|| {
            format!(
                "release runtime artifact target has no parent: {}",
                target.display()
            )
        })?;
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        let staged = temporary_runtime_artifact_path(target);
        remove_stale_staged_artifact(&staged)?;
        stage_runtime_artifact_link(&artifact_identity, &staged)?;
        let status = if std::fs::symlink_metadata(target).is_ok() {
            "updated"
        } else {
            "installed"
        };
        atomic_replace_runtime_artifact(&staged, target)?;
        status
    };
    let reference = RuntimeArtifactReference {
        schema_id: RUNTIME_ARTIFACT_REFERENCE_SCHEMA_ID.to_owned(),
        schema_version: RUNTIME_ARTIFACT_REFERENCE_SCHEMA_VERSION,
        artifact_kind,
        origin: ArtifactOrigin::LockedRelease,
        executable_path: target.to_path_buf(),
        content_digest: format!("blake3-256:{content_digest}"),
        identity: RuntimeBinaryIdentity::Content {
            value: content_digest.clone(),
            algorithm: "blake3-256".to_owned(),
        },
        checkout_root: None,
    };
    reference.validate()?;
    crate::runtime_artifact_retention::prune_unreachable_runtime_artifacts_blocking(artifact_root)?;
    Ok(RuntimeArtifactPublication {
        path: target.to_path_buf(),
        status,
        artifact_digest: content_digest,
        identity: reference.identity.clone(),
        reference,
    })
}

fn temporary_runtime_artifact_path(target: &Path) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);

    let file_name = target
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("runtime-artifact");
    target.with_file_name(format!(
        ".{file_name}.{}.{}.tmp",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ))
}

fn remove_stale_staged_artifact(staged: &Path) -> Result<(), String> {
    if std::fs::symlink_metadata(staged).is_ok() {
        std::fs::remove_file(staged)
            .map_err(|error| format!("failed to remove stale {}: {error}", staged.display()))?;
    }
    Ok(())
}

#[cfg(unix)]
fn stage_runtime_artifact_link(artifact: &Path, staged: &Path) -> Result<(), String> {
    std::os::unix::fs::symlink(artifact, staged).map_err(|error| {
        format!(
            "failed to stage runtime artifact link {} -> {}: {error}",
            staged.display(),
            artifact.display()
        )
    })
}

#[cfg(windows)]
fn stage_runtime_artifact_link(artifact: &Path, staged: &Path) -> Result<(), String> {
    std::os::windows::fs::symlink_file(artifact, staged).map_err(|error| {
        format!(
            "failed to stage runtime artifact link {} -> {}: {error}",
            staged.display(),
            artifact.display()
        )
    })
}

#[cfg(not(any(unix, windows)))]
fn stage_runtime_artifact_link(artifact: &Path, staged: &Path) -> Result<(), String> {
    std::fs::copy(artifact, staged)
        .map(|_| ())
        .map_err(|error| {
            format!(
                "failed to stage runtime artifact {}: {error}",
                staged.display()
            )
        })
}

fn atomic_replace_runtime_artifact(staged: &Path, target: &Path) -> Result<(), String> {
    std::fs::rename(staged, target)
        .map_err(|error| format!("failed to atomically publish {}: {error}", target.display()))
}

fn prepare_developer_artifact_publication_blocking(
    developer_root: &Path,
    source: &Path,
    target: &Path,
    artifact_kind: impl Into<String>,
) -> Result<DeveloperArtifactPublicationPlan, String> {
    let developer_root = std::fs::canonicalize(developer_root).map_err(|error| {
        format!(
            "canonicalize development artifact root {}: {error}",
            developer_root.display()
        )
    })?;
    let source_identity = std::fs::canonicalize(source).map_err(|error| {
        format!(
            "canonicalize development artifact source {}: {error}",
            source.display()
        )
    })?;
    if !source_identity.is_file() {
        return Err(format!(
            "development artifact source must be a file: {}",
            source_identity.display()
        ));
    }
    if !source_identity.starts_with(&developer_root) {
        return Err(format!(
            "development artifact source escapes configured root: source={} root={}",
            source_identity.display(),
            developer_root.display()
        ));
    }
    if !target.is_absolute() {
        return Err(format!(
            "development artifact target must be absolute: {}",
            target.display()
        ));
    }
    let metadata = std::fs::metadata(&source_identity).map_err(|error| {
        format!("inspect development artifact {}: {error}", source_identity.display())
    })?;
    let modified_ns = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |duration| duration.as_nanos());
    let generation_input = format!("{}:{}:{}", source_identity.display(), metadata.len(), modified_ns);
    let artifact_digest = blake3::hash(generation_input.as_bytes()).to_hex().to_string();
    let reference = RuntimeArtifactReference::new(
        artifact_kind,
        ArtifactOrigin::DevelopWorkspace,
        source_identity.clone(),
        String::new(),
        Some(developer_root),
    );
    let mut reference = reference;
    reference.identity = RuntimeBinaryIdentity::DeveloperSourceGeneration {
        value: artifact_digest.clone(),
        algorithm: "blake3-metadata-v1".to_owned(),
    };
    reference.validate()?;
    Ok(DeveloperArtifactPublicationPlan {
        source_identity,
        target: target.to_path_buf(),
        artifact_digest,
        reference,
    })
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
                if !reference.executable_path.starts_with(root) {
                    return Err(format!(
                        "development artifact executable escapes configured root: {}",
                        reference.executable_path.display()
                    ));
                }
            }
            RuntimeArtifactMode::Release => {
                if reference.origin != ArtifactOrigin::LockedRelease {
                    return Err("release runtime rejects non-release artifact".to_owned());
                }
                if reference.checkout_root.is_some() {
                    return Err("release artifact must not retain a checkout root".to_owned());
                }
                let stable_slots = [runtime_root.join("bin"), runtime_root.join("profiles")];
                if !stable_slots
                    .iter()
                    .any(|stable_root| reference.executable_path.starts_with(stable_root))
                {
                    return Err(format!(
                        "release artifact executable is outside stable runtime slots: {}",
                        reference.executable_path.display()
                    ));
                }
                let digest_lattice = runtime_root.join("artifacts").join("blake3-256");
                if reference.executable_path.starts_with(&digest_lattice) {
                    return Err("digest lattice path cannot be an active executable".to_owned());
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

/// Load one immutable catalog generation from `$ASP_STATE_HOME/asp.toml`.
///
/// The supervisor owns this asynchronous read. Search and query consumers keep
/// the returned value in memory and never call this function.
/// Load only the Runtime Artifact Authority mode for synchronous publishers.
pub fn load_runtime_artifact_mode(state_home: &Path) -> Result<RuntimeArtifactMode, String> {
    let config_path = state_home.join("asp.toml");
    let input = match std::fs::read_to_string(&config_path) {
        Ok(input) => input,
        Err(error) if error.kind() == ErrorKind::NotFound => String::new(),
        Err(error) => {
            return Err(format!(
                "read runtime configuration {}: {error}",
                config_path.display()
            ));
        }
    };
    match parse_runtime_artifact_mode(&input)? {
        RuntimeArtifactMode::Dev { root } => {
            let root = std::fs::canonicalize(&root).map_err(|error| {
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
