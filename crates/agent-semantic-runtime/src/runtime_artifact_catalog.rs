//! Immutable runtime artifact catalog loaded once by the Tokio supervisor.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

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
}

pub const RUNTIME_ARTIFACT_REFERENCE_SCHEMA_ID: &str =
    "agent.semantic-protocols.runtime-artifact-reference";
pub const RUNTIME_ARTIFACT_REFERENCE_SCHEMA_VERSION: u64 = 1;

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "identity")]
pub enum RuntimeBinaryIdentity {
    Content { value: String, algorithm: String },
}

impl RuntimeBinaryIdentity {
    pub fn kind(&self) -> &'static str {
        "content"
    }

    pub fn algorithm(&self) -> &str {
        let Self::Content { algorithm, .. } = self;
        algorithm
    }

    pub fn value(&self) -> &str {
        let Self::Content { value, .. } = self;
        value
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
        let identity_value = content_digest
            .strip_prefix("blake3-256:")
            .unwrap_or(&content_digest)
            .to_owned();
        Self {
            schema_id: RUNTIME_ARTIFACT_REFERENCE_SCHEMA_ID.to_owned(),
            schema_version: RUNTIME_ARTIFACT_REFERENCE_SCHEMA_VERSION,
            artifact_kind: artifact_kind.into(),
            origin,
            executable_path,
            content_digest: content_digest.clone(),
            identity: RuntimeBinaryIdentity::Content {
                value: identity_value,
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
        let Some(digest_hex) = self.content_digest.strip_prefix("blake3-256:") else {
            return Err("runtime artifact content digest must use blake3-256".to_owned());
        };
        if digest_hex.len() != 64 || !digest_hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(
                "runtime artifact content digest must contain 64 hexadecimal digits".to_owned(),
            );
        }
        let RuntimeBinaryIdentity::Content { value, algorithm } = &self.identity;
        if algorithm != "blake3-256" || value != digest_hex {
            return Err(format!(
                "runtime artifact identity drift: digest={digest_hex} identityAlgorithm={algorithm} identityValue={value}"
            ));
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
    pub source_path: PathBuf,
    pub source_generation: String,
    pub status: &'static str,
    pub artifact_digest: String,
    pub reference: RuntimeArtifactReference,
    pub identity: RuntimeBinaryIdentity,
}

/// Publishes one Runtime artifact according to the state-home authority.
///
/// Development and release sources are provenance inputs only. Both are copied
/// into the same content-addressed store before an executable slot is moved.
pub async fn publish_runtime_artifact(
    state_home: &Path,
    source: &Path,
    target: &Path,
    artifact_root: &Path,
    artifact_kind: impl Into<String>,
) -> Result<RuntimeArtifactPublication, String> {
    let guard = crate::runtime_artifact_retention::RuntimeArtifactMutationGuard::try_acquire(
        artifact_root,
    )?;
    publish_runtime_artifact_under_guard(
        state_home,
        source,
        target,
        artifact_root,
        artifact_kind,
        &guard,
    )
    .await
}

pub async fn publish_runtime_artifact_under_guard(
    state_home: &Path,
    source: &Path,
    target: &Path,
    artifact_root: &Path,
    artifact_kind: impl Into<String>,
    guard: &crate::runtime_artifact_retention::RuntimeArtifactMutationGuard,
) -> Result<RuntimeArtifactPublication, String> {
    if !guard.admits(artifact_root) {
        return Err(format!(
            "Runtime artifact mutation guard authority drift: artifactRoot={}",
            artifact_root.display()
        ));
    }
    let permit = artifact_publication_semaphore()
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| "Runtime artifact publication scheduler closed".to_owned())?;
    let state_home = state_home.to_path_buf();
    let source = source.to_path_buf();
    let target = target.to_path_buf();
    let artifact_root = artifact_root.to_path_buf();
    let artifact_kind = artifact_kind.into();
    let guard = guard.clone();
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let _guard = guard;
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

pub async fn publish_qualified_runtime_artifact_under_guard(
    state_home: &Path,
    source: &Path,
    target: &Path,
    artifact_root: &Path,
    artifact_kind: impl Into<String>,
    authority: QualifiedRuntimeArtifactSource,
    guard: &crate::runtime_artifact_retention::RuntimeArtifactMutationGuard,
) -> Result<RuntimeArtifactPublication, String> {
    if !guard.admits(artifact_root) {
        return Err(format!(
            "Runtime artifact mutation guard authority drift: artifactRoot={}",
            artifact_root.display()
        ));
    }
    let permit = artifact_publication_semaphore()
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| "Runtime artifact publication scheduler closed".to_owned())?;
    let state_home = state_home.to_path_buf();
    let source = source.to_path_buf();
    let target = target.to_path_buf();
    let artifact_root = artifact_root.to_path_buf();
    let artifact_kind = artifact_kind.into();
    let guard = guard.clone();
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let _guard = guard;
        publish_qualified_runtime_artifact_blocking(
            &state_home,
            &source,
            &target,
            &artifact_root,
            artifact_kind,
            authority,
        )
    })
    .await
    .map_err(|error| format!("qualified Runtime artifact publication task failed: {error}"))?
}

/// Promotes the current immutable generation only after an external Runtime
/// health authority has qualified the exact same content digest. The empty
/// active/healthy pair is seeded during first publication; later publications cannot move
/// the healthy slot through this API without matching health evidence.
pub async fn promote_active_runtime_artifact_to_healthy(
    state_home: &Path,
    binary: &str,
    qualified_digest: &str,
) -> Result<String, String> {
    if binary.is_empty()
        || Path::new(binary).components().count() != 1
        || Path::new(binary).file_name().and_then(|name| name.to_str()) != Some(binary)
    {
        return Err(format!(
            "invalid Runtime artifact binary identity: {binary:?}"
        ));
    }
    let state_home = state_home.to_path_buf();
    let binary = binary.to_owned();
    let qualified_digest = qualified_digest
        .strip_prefix("blake3-256:")
        .unwrap_or(qualified_digest)
        .to_owned();
    let artifact_root = state_home.join("runtime/artifacts");
    tokio::task::spawn_blocking(move || {
        let _mutation_guard =
            crate::runtime_artifact_retention::RuntimeArtifactMutationGuard::try_acquire(
                &artifact_root,
            )?;
        let runtime_root = state_home.join("runtime");
        let algorithm_root = artifact_root.join("blake3-256");
        let profile_root = runtime_root.join("profiles").join(&binary);
        let active_slot = profile_root.join("active");
        let healthy_slot = profile_root.join("healthy");
        let active_identity = std::fs::canonicalize(&active_slot).map_err(|error| {
            format!(
                "resolve active Runtime artifact {}: {error}",
                active_slot.display()
            )
        })?;
        let canonical_algorithm_root = std::fs::canonicalize(&algorithm_root).map_err(|error| {
            format!(
                "resolve Runtime artifact content store {}: {error}",
                algorithm_root.display()
            )
        })?;
        let relative = active_identity
            .strip_prefix(&canonical_algorithm_root)
            .map_err(|_| {
                format!(
                    "active Runtime artifact escapes content store: {}",
                    active_identity.display()
                )
            })?;
        let mut components = relative.components();
        let digest = components
            .next()
            .and_then(|component| component.as_os_str().to_str())
            .filter(|digest| {
                digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
            .ok_or_else(|| "active Runtime artifact has no valid content digest".to_owned())?;
        if digest != qualified_digest {
            return Err(format!(
                "Runtime health identity does not qualify active artifact: qualified={qualified_digest} active={digest}"
            ));
        }
        let artifact_binary = components
            .next()
            .and_then(|component| component.as_os_str().to_str());
        if artifact_binary != Some(binary.as_str()) || components.next().is_some() {
            return Err(format!(
                "active Runtime artifact identity drift: expected={binary} actual={}",
                active_identity.display()
            ));
        }
        publish_runtime_artifact_link(&active_identity, &healthy_slot)?;
        crate::runtime_artifact_retention::prune_unreachable_runtime_artifacts_blocking(
            &artifact_root,
        )?;
        Ok(digest.to_owned())
    })
    .await
    .map_err(|error| format!("Runtime artifact health promotion task failed: {error}"))?
}

fn artifact_publication_semaphore() -> &'static Arc<tokio::sync::Semaphore> {
    static SEMAPHORE: OnceLock<Arc<tokio::sync::Semaphore>> = OnceLock::new();
    SEMAPHORE.get_or_init(|| Arc::new(tokio::sync::Semaphore::new(2)))
}

fn publish_runtime_artifact_blocking(
    state_home: &Path,
    source: &Path,
    target: &Path,
    artifact_root: &Path,
    artifact_kind: String,
) -> Result<RuntimeArtifactPublication, String> {
    let (origin, checkout_root) = match load_runtime_developer_root(state_home)? {
        Some(root) => {
            let source_identity = std::fs::canonicalize(source).map_err(|error| {
                format!(
                    "canonicalize development artifact source {}: {error}",
                    source.display()
                )
            })?;
            if !source_identity.starts_with(&root) {
                return Err(format!(
                    "development artifact source escapes configured root: source={} root={}",
                    source_identity.display(),
                    root.display()
                ));
            }
            (ArtifactOrigin::DevelopWorkspace, Some(root))
        }
        None => (ArtifactOrigin::LockedRelease, None),
    };
    publish_content_artifact(
        source,
        target,
        artifact_root,
        artifact_kind,
        origin,
        checkout_root,
    )
}

fn publish_qualified_runtime_artifact_blocking(
    state_home: &Path,
    source: &Path,
    target: &Path,
    artifact_root: &Path,
    artifact_kind: String,
    authority: QualifiedRuntimeArtifactSource,
) -> Result<RuntimeArtifactPublication, String> {
    let configured_root = load_runtime_developer_root(state_home)?
        .ok_or_else(|| "qualified development artifact requires Runtime dev mode".to_owned())?;
    let checkout_root = std::fs::canonicalize(&authority.checkout_root).map_err(|error| {
        format!(
            "canonicalize qualified Runtime checkout root {}: {error}",
            authority.checkout_root.display()
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
    let staging_root = state_home
        .join("runtime/provider-artifacts")
        .join(&artifact_kind)
        .join("artifacts");
    if !source_identity.starts_with(&staging_root) {
        return Err(format!(
            "qualified Runtime artifact source escapes provider staging: source={} stagingRoot={}",
            source_identity.display(),
            staging_root.display()
        ));
    }
    publish_content_artifact(
        &source_identity,
        target,
        artifact_root,
        artifact_kind,
        ArtifactOrigin::DevelopWorkspace,
        Some(checkout_root),
    )
}

fn publish_content_artifact(
    source: &Path,
    target: &Path,
    artifact_root: &Path,
    artifact_kind: String,
    origin: ArtifactOrigin,
    checkout_root: Option<PathBuf>,
) -> Result<RuntimeArtifactPublication, String> {
    if !target.is_absolute() || !artifact_root.is_absolute() {
        return Err(format!(
            "runtime artifact publication requires absolute paths: target={} artifactRoot={}",
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
    let source_generation =
        crate::runtime_artifact_identity::runtime_artifact_source_generation(&source_identity)?;
    let file_name = target.file_name().ok_or_else(|| {
        format!(
            "runtime artifact target has no file name: {}",
            target.display()
        )
    })?;
    std::fs::create_dir_all(artifact_root)
        .map_err(|error| format!("failed to create {}: {error}", artifact_root.display()))?;
    let staged = temporary_runtime_artifact_path(&artifact_root.join(file_name));
    remove_stale_staged_artifact(&staged)?;
    let staged_snapshot = (|| -> Result<String, String> {
        std::fs::copy(&source_identity, &staged).map_err(|error| {
            format!(
                "failed to snapshot runtime artifact {}: {error}",
                staged.display()
            )
        })?;
        let permissions = std::fs::metadata(&source_identity)
            .map_err(|error| format!("failed to inspect {}: {error}", source_identity.display()))?
            .permissions();
        std::fs::set_permissions(&staged, permissions)
            .map_err(|error| format!("failed to chmod {}: {error}", staged.display()))?;
        runtime_artifact_content_digest(&staged)
    })();
    let content_digest = match staged_snapshot {
        Ok(digest) => digest,
        Err(error) => {
            let _ = std::fs::remove_file(&staged);
            return Err(error);
        }
    };
    let source_generation_after_snapshot =
        crate::runtime_artifact_identity::runtime_artifact_source_generation(&source_identity)?;
    if source_generation_after_snapshot != source_generation {
        let _ = std::fs::remove_file(&staged);
        return Err(format!(
            "Runtime artifact source changed during snapshot: source={} reasonKind=source-generation-raced",
            source_identity.display()
        ));
    }
    let artifact_path = artifact_root
        .join("blake3-256")
        .join(&content_digest)
        .join(file_name);
    if !artifact_path.is_file() {
        let artifact_parent = artifact_path.parent().ok_or_else(|| {
            format!(
                "runtime artifact has no parent: {}",
                artifact_path.display()
            )
        })?;
        std::fs::create_dir_all(artifact_parent)
            .map_err(|error| format!("failed to create {}: {error}", artifact_parent.display()))?;
        if let Err(error) = atomic_replace_runtime_artifact(&staged, &artifact_path) {
            let _ = std::fs::remove_file(&staged);
            return Err(error);
        }
    } else {
        let staged_len = std::fs::metadata(&staged)
            .map_err(|error| format!("failed to inspect {}: {error}", staged.display()))?
            .len();
        let existing_len = std::fs::metadata(&artifact_path)
            .map_err(|error| format!("failed to inspect {}: {error}", artifact_path.display()))?
            .len();
        let _ = std::fs::remove_file(&staged);
        if existing_len != staged_len {
            return Err(format!(
                "content-addressed runtime artifact size drift: digest={content_digest} stagedBytes={staged_len} existingBytes={existing_len} path={}",
                artifact_path.display(),
            ));
        }
    }
    let artifact_identity = std::fs::canonicalize(&artifact_path).map_err(|error| {
        format!(
            "failed to resolve release runtime artifact {}: {error}",
            artifact_path.display()
        )
    })?;
    let runtime_root = artifact_root.parent().ok_or_else(|| {
        format!(
            "runtime artifact root has no runtime parent: {}",
            artifact_root.display()
        )
    })?;
    let profile_root = runtime_root.join("profiles").join(file_name);
    let active_slot = profile_root.join("active");
    let healthy_slot = profile_root.join("healthy");
    std::fs::create_dir_all(&profile_root)
        .map_err(|error| format!("failed to create {}: {error}", profile_root.display()))?;

    let previous_active = resolve_artifact_profile_slot(&active_slot, artifact_root, file_name)?;
    let current_healthy = resolve_artifact_profile_slot(&healthy_slot, artifact_root, file_name)?;
    if current_healthy.is_none() {
        publish_runtime_artifact_link(
            previous_active.as_deref().unwrap_or(&artifact_identity),
            &healthy_slot,
        )?;
    }
    let target_is_current = std::fs::canonicalize(&active_slot)
        .ok()
        .is_some_and(|identity| identity == artifact_identity);
    let status = if target_is_current {
        "current"
    } else {
        let status = if std::fs::symlink_metadata(&active_slot).is_ok() {
            "updated"
        } else {
            "installed"
        };
        publish_runtime_artifact_link(&artifact_identity, &active_slot)?;
        status
    };
    let target_is_active_slot = std::fs::read_link(target)
        .ok()
        .is_some_and(|link| link == active_slot);
    if !target_is_active_slot {
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        }
        publish_runtime_artifact_link(&active_slot, target)?;
    }
    let reference = RuntimeArtifactReference {
        schema_id: RUNTIME_ARTIFACT_REFERENCE_SCHEMA_ID.to_owned(),
        schema_version: RUNTIME_ARTIFACT_REFERENCE_SCHEMA_VERSION,
        artifact_kind,
        origin,
        executable_path: target.to_path_buf(),
        content_digest: format!("blake3-256:{content_digest}"),
        identity: RuntimeBinaryIdentity::Content {
            value: content_digest.clone(),
            algorithm: "blake3-256".to_owned(),
        },
        checkout_root,
    };
    reference.validate()?;
    crate::runtime_artifact_retention::prune_unreachable_runtime_artifacts_blocking(artifact_root)?;
    Ok(RuntimeArtifactPublication {
        path: target.to_path_buf(),
        source_path: source_identity,
        source_generation,
        status,
        artifact_digest: content_digest,
        identity: reference.identity.clone(),
        reference,
    })
}

fn publish_runtime_artifact_link(artifact: &Path, target: &Path) -> Result<(), String> {
    let staged = temporary_runtime_artifact_path(target);
    remove_stale_staged_artifact(&staged)?;
    stage_runtime_artifact_link(artifact, &staged)?;
    atomic_replace_runtime_artifact(&staged, target)
}

fn resolve_artifact_profile_slot(
    slot: &Path,
    artifact_root: &Path,
    binary: &std::ffi::OsStr,
) -> Result<Option<PathBuf>, String> {
    let identity = match std::fs::canonicalize(slot) {
        Ok(identity) => identity,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "failed to resolve Runtime artifact slot {}: {error}",
                slot.display()
            ));
        }
    };
    let algorithm_root =
        std::fs::canonicalize(artifact_root.join("blake3-256")).map_err(|error| {
            format!(
                "failed to resolve Runtime artifact content store {}: {error}",
                artifact_root.display()
            )
        })?;
    let relative = identity.strip_prefix(&algorithm_root).map_err(|_| {
        format!(
            "Runtime artifact slot escapes content store: slot={} target={}",
            slot.display(),
            identity.display()
        )
    })?;
    let mut components = relative.components();
    let digest_valid = components
        .next()
        .and_then(|component| component.as_os_str().to_str())
        .is_some_and(|digest| {
            digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
        });
    let binary_matches = components
        .next()
        .is_some_and(|component| component.as_os_str() == binary);
    if !digest_valid || !binary_matches || components.next().is_some() {
        return Err(format!(
            "Runtime artifact slot identity drift: slot={} target={}",
            slot.display(),
            identity.display()
        ));
    }
    Ok(Some(identity))
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
        crate::runtime_process_lifecycle::current_process_id(),
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
                let stable_slots = [runtime_root.join("bin"), runtime_root.join("profiles")];
                if !stable_slots
                    .iter()
                    .any(|stable_root| reference.executable_path.starts_with(stable_root))
                {
                    return Err(format!(
                        "development artifact executable is outside stable runtime slots: {}",
                        reference.executable_path.display()
                    ));
                }
                let content_store = runtime_root.join("artifacts").join("blake3-256");
                if reference.executable_path.starts_with(&content_store) {
                    return Err("content-store path cannot be an active executable".to_owned());
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
                let content_store = runtime_root.join("artifacts").join("blake3-256");
                if reference.executable_path.starts_with(&content_store) {
                    return Err("content-store path cannot be an active executable".to_owned());
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
