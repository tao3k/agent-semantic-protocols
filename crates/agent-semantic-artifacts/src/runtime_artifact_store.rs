//! Immutable content-store publication and stable artifact links.

use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::OnceLock;

use agent_semantic_config::runtime_dev::ArtifactOrigin;

use crate::runtime_artifact_catalog::QualifiedRuntimeArtifactSource;
use crate::runtime_artifact_catalog::RUNTIME_ARTIFACT_REFERENCE_SCHEMA_ID;
use crate::runtime_artifact_catalog::RUNTIME_ARTIFACT_REFERENCE_SCHEMA_VERSION;
use crate::runtime_artifact_catalog::RuntimeArtifactReference;
use crate::runtime_artifact_catalog::RuntimeBinaryIdentity;
use crate::runtime_artifact_catalog::load_runtime_developer_root;
use crate::runtime_artifact_catalog::runtime_artifact_source_generation;

pub(crate) fn runtime_artifact_content_digest(
    path: &Path,
) -> Result<crate::blake3_content_digest::Blake3ContentDigest, String> {
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
    let content = agent_semantic_content_identity::exact_selector_merkle::ContentDigestV1::parse(
        hasher.finalize().to_hex().as_str(),
    )
    .map_err(|error| format!("encode runtime artifact content digest: {error}"))?;
    Ok(crate::blake3_content_digest::Blake3ContentDigest::from_content_digest(content))
}

pub(crate) fn stage_runtime_artifact(source: &Path, staged: &Path) -> Result<(), String> {
    // The content-addressed store is an immutable snapshot boundary. A hard
    // link would leave the published artifact on the source executable's inode,
    // so a later build-side chmod or in-place update could mutate the active
    // Runtime artifact and invalidate its identity receipt.
    std::fs::copy(source, staged).map_err(|error| {
        format!(
            "failed to snapshot runtime artifact {}: {error}",
            staged.display()
        )
    })?;
    let permissions = std::fs::metadata(source)
        .map_err(|error| format!("failed to inspect {}: {error}", source.display()))?
        .permissions();
    std::fs::set_permissions(staged, permissions)
        .map_err(|error| format!("failed to chmod {}: {error}", staged.display()))?;
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeArtifactPublication {
    pub path: PathBuf,
    pub source_path: PathBuf,
    pub source_generation: String,
    pub source_generation_algorithm: String,
    pub status: &'static str,
    pub artifact_digest: crate::blake3_content_digest::Blake3ContentDigest,
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
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
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
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
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
    qualified_digest: &crate::blake3_content_digest::Blake3ContentDigest,
) -> Result<crate::blake3_content_digest::Blake3ContentDigest, String> {
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
    let qualified_digest = qualified_digest.clone();
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
        let digest_hex = components
            .next()
            .and_then(|component| component.as_os_str().to_str())
            .ok_or_else(|| "active Runtime artifact has no valid content digest".to_owned())?;
        let digest = crate::blake3_content_digest::Blake3ContentDigest::parse(&format!(
            "blake3-256:{digest_hex}"
        ))?;
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
        Ok(digest)
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
    let (checkout_root, source_identity) =
        authority.validate_source(state_home, source, &artifact_kind)?;
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
    let source_generation = runtime_artifact_source_generation(&source_identity)?;
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
    let staged_snapshot = (|| {
        stage_runtime_artifact(&source_identity, &staged)?;
        runtime_artifact_content_digest(&staged)
    })();
    let content_digest = match staged_snapshot {
        Ok(digest) => digest,
        Err(error) => {
            let _ = std::fs::remove_file(&staged);
            return Err(error);
        }
    };
    let source_generation_after_snapshot = runtime_artifact_source_generation(&source_identity)?;
    if source_generation_after_snapshot != source_generation {
        let _ = std::fs::remove_file(&staged);
        return Err(format!(
            "Runtime artifact source changed during snapshot: source={} reasonKind=source-generation-raced",
            source_identity.display()
        ));
    }
    let artifact_path = artifact_root
        .join("blake3-256")
        .join(content_digest.content_digest().as_str())
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
        content_digest: content_digest.clone(),
        identity: RuntimeBinaryIdentity::Content {
            digest: content_digest.clone(),
        },
        checkout_root,
    };
    reference.validate()?;
    crate::runtime_artifact_retention::prune_unreachable_runtime_artifacts_blocking(artifact_root)?;
    Ok(RuntimeArtifactPublication {
        path: target.to_path_buf(),
        source_path: source_identity,
        source_generation,
        source_generation_algorithm: "filesystem-generation-v1".to_owned(),
        status,
        artifact_digest: content_digest,
        identity: reference.identity.clone(),
        reference,
    })
}

pub(crate) fn publish_runtime_artifact_link(artifact: &Path, target: &Path) -> Result<(), String> {
    let parent = target
        .parent()
        .ok_or_else(|| format!("Runtime artifact link has no parent: {}", target.display()))?;
    std::fs::create_dir_all(parent).map_err(|error| {
        format!(
            "create Runtime artifact link directory {}: {error}",
            parent.display()
        )
    })?;
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
    use std::sync::atomic::AtomicU64;
    use std::sync::atomic::Ordering;
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
