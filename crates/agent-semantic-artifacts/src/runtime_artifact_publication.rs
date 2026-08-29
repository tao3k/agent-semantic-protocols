//! Millisecond artifact publication and typed Runtime activation receipts.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::blake3_content_digest::Blake3ContentDigest;

use crate::runtime_artifact_catalog::{
    RuntimeArtifactSlotAuthority, discard_prepared_runtime_artifact,
    publish_resident_runtime_alias, runtime_artifact_candidate_digest,
};
use crate::runtime_artifact_quiescence::prepare_runtime_artifact_quiescence_lease;
use crate::runtime_artifact_retention::RuntimeArtifactMutationGuard;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeArtifactCandidateIdentityReceipt {
    pub artifact_digest: Blake3ContentDigest,
    pub artifact_path: PathBuf,
    pub stable_path: PathBuf,
    pub artifact_mode: String,
    pub publication_nonce: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeArtifactActivationEvent {
    pub artifact_digest: Blake3ContentDigest,
    pub artifact_path: PathBuf,
    pub candidate_slot_path: PathBuf,
    pub previous_artifact_digest: Option<Blake3ContentDigest>,
    pub artifact_mode: String,
    pub published_at_unix_millis: u128,
    pub publication_nonce: String,
    pub activation_generation: u64,
    pub candidate_identity: RuntimeArtifactCandidateIdentityReceipt,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeArtifactPublicationReceipt {
    pub path: PathBuf,
    pub status: &'static str,
    pub artifact_digest: Blake3ContentDigest,
    pub activation_event_path: PathBuf,
    pub lock_elapsed_micros: u128,
    pub phase_trace: RuntimeArtifactPublicationPhaseTrace,
    pub lock_acquisition_count: u8,
    pub quiescence_operation: String,
    pub quiescence_lease_nonce: String,
    pub lease_producer_process_id: u32,
    pub lease_consumer_process_id: u32,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeArtifactPublicationPhaseTrace {
    pub guard_acquisition_micros: u128,
    pub lease_consume_micros: u128,
    pub receipt_read_validate_micros: u128,
    pub generation_read_micros: u128,
    pub candidate_activation_staging_micros: u128,
    pub pending_publication_micros: u128,
    pub client_launcher_switch_micros: u128,
}

pub async fn current_runtime_artifact_activation_generation(
    state_home: &Path,
) -> Result<u64, String> {
    let pending_generation = read_runtime_artifact_activation_event(state_home)
        .await?
        .map(|event| event.activation_generation)
        .unwrap_or(0);
    let applied_path = state_home.join("runtime/activation/applied.json");
    let applied_generation = match tokio::fs::read(&applied_path).await {
        Ok(bytes) => {
            let value = serde_json::from_slice::<serde_json::Value>(&bytes).map_err(|error| {
                format!(
                    "failed to decode Runtime artifact activation commit receipt {}: {error}",
                    applied_path.display()
                )
            })?;
            match value.get("activationGeneration") {
                None => 0,
                Some(generation) => generation.as_u64().ok_or_else(|| {
                    format!(
                        "Runtime artifact activation commit receipt {} has a non-u64 activationGeneration",
                        applied_path.display()
                    )
                })?,
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
        Err(error) => {
            return Err(format!(
                "failed to read Runtime artifact activation commit receipt {}: {error}",
                applied_path.display()
            ));
        }
    };
    Ok(pending_generation.max(applied_generation))
}

fn current_runtime_artifact_activation_generation_under_guard(
    state_home: &Path,
) -> Result<u64, String> {
    let generation = |path: &Path, context: &str| -> Result<u64, String> {
        match std::fs::read(path) {
            Ok(bytes) => Ok(
                decode_runtime_artifact_activation_event(&bytes, context)?.activation_generation
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(0),
            Err(error) => Err(format!("read {context} {}: {error}", path.display())),
        }
    };
    let pending = generation(
        &runtime_artifact_activation_event_path(state_home),
        "Runtime artifact activation event",
    )?;
    let applied = generation(
        &state_home.join("runtime/activation/applied.json"),
        "applied Runtime artifact activation event",
    )?;
    pending
        .max(applied)
        .checked_add(1)
        .ok_or_else(|| "Runtime artifact activation generation overflow".to_owned())
}

pub fn runtime_artifact_activation_event_path(state_home: &Path) -> PathBuf {
    state_home.join("runtime/activation/pending.json")
}

fn publish_pending_runtime_artifact_activation(
    path: &Path,
    bytes: &[u8],
    publication_nonce: &str,
) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Runtime artifact activation path has no parent".to_owned())?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("create Runtime artifact activation directory: {error}"))?;
    let staged = parent.join(format!(".pending-{publication_nonce}.json.tmp"));
    std::fs::write(&staged, bytes)
        .map_err(|error| format!("stage Runtime artifact activation event: {error}"))?;
    std::fs::rename(&staged, path)
        .map_err(|error| format!("publish Runtime artifact activation event: {error}"))
}

pub fn runtime_artifact_activation_socket_path(state_home: &Path) -> PathBuf {
    let digest = blake3::hash(state_home.to_string_lossy().as_bytes())
        .to_hex()
        .to_string();
    std::env::temp_dir()
        .join("asp-activation")
        .join(format!("{}.sock", &digest[..32]))
}

pub async fn read_runtime_artifact_activation_event(
    state_home: &Path,
) -> Result<Option<RuntimeArtifactActivationEvent>, String> {
    let path = runtime_artifact_activation_event_path(state_home);
    match tokio::fs::read(&path).await {
        Ok(bytes) => {
            let event = decode_runtime_artifact_activation_event(
                &bytes,
                "Runtime artifact activation event",
            )?;
            if let Some(applied_event) =
                read_applied_runtime_artifact_activation_event(state_home).await?
            {
                if runtime_artifact_activation_is_applied(&event, &applied_event) {
                    return Ok(None);
                }
            }
            Ok(Some(event))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!(
            "read Runtime artifact activation event {}: {error}",
            path.display()
        )),
    }
}

fn runtime_artifact_activation_is_applied(
    pending: &RuntimeArtifactActivationEvent,
    applied: &RuntimeArtifactActivationEvent,
) -> bool {
    pending.activation_generation == applied.activation_generation
        && pending.artifact_digest == applied.artifact_digest
}

pub async fn read_applied_runtime_artifact_activation_event(
    state_home: &Path,
) -> Result<Option<RuntimeArtifactActivationEvent>, String> {
    let path = state_home.join("runtime/activation/applied.json");
    match tokio::fs::read(&path).await {
        Ok(bytes) => decode_runtime_artifact_activation_event(
            &bytes,
            "applied Runtime artifact activation event",
        )
        .map(Some),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!(
            "read applied Runtime artifact activation event {}: {error}",
            path.display()
        )),
    }
}

fn decode_runtime_artifact_activation_event(
    bytes: &[u8],
    context: &str,
) -> Result<RuntimeArtifactActivationEvent, String> {
    let event: RuntimeArtifactActivationEvent =
        serde_json::from_slice(bytes).map_err(|error| format!("decode {context}: {error}"))?;
    validate_runtime_artifact_activation_event(&event, context)?;
    Ok(event)
}

fn validate_runtime_artifact_activation_event(
    event: &RuntimeArtifactActivationEvent,
    context: &str,
) -> Result<(), String> {
    let identity = &event.candidate_identity;
    if event.activation_generation == 0
        || event.publication_nonce.trim().is_empty()
        || event.artifact_mode.trim().is_empty()
        || !event.artifact_path.is_absolute()
        || !event.candidate_slot_path.is_absolute()
        || !identity.stable_path.is_absolute()
        || event.artifact_digest != identity.artifact_digest
        || event.artifact_path != identity.artifact_path
        || event.artifact_mode != identity.artifact_mode
        || event.publication_nonce != identity.publication_nonce
    {
        return Err(format!(
            "state=runtime-artifact-activation-failed reasonKind=activation-receipt-identity-invalid context={context}"
        ));
    }
    Ok(())
}

async fn previous_serving_digest_under_guard(
    slots: &RuntimeArtifactSlotAuthority,
    artifact_kind: &str,
) -> Result<Option<Blake3ContentDigest>, String> {
    let active_path = slots.active_path();
    let active_exists = match std::fs::symlink_metadata(&active_path) {
        Ok(_) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => {
            return Err(format!(
                "state=runtime-artifact-publication-failed reasonKind=active-slot-unreadable path={} error={error}",
                active_path.display()
            ));
        }
    };
    match active_exists {
        false => Ok(None),
        true => {
            let active = slots.active_target().await.map_err(|error| {
                format!(
                    "state=runtime-artifact-publication-failed reasonKind=active-slot-dangling error={error}"
                )
            })?;
            let active = active.ok_or_else(|| {
                "state=runtime-artifact-publication-failed reasonKind=active-slot-dangling"
                    .to_owned()
            })?;
            let candidate_dir = active.parent().ok_or_else(|| {
                format!(
                    "state=runtime-artifact-publication-failed reasonKind=active-slot-has-no-candidate-parent path={}",
                    active.display()
                )
            })?;
            let receipt_path = candidate_dir.join("activation.json");
            let receipt_bytes = std::fs::read(&receipt_path).map_err(|error| {
                format!(
                    "state=runtime-artifact-publication-failed reasonKind=active-receipt-unreadable path={} error={error}",
                    receipt_path.display()
                )
            })?;
            let applied = decode_runtime_artifact_activation_event(
                &receipt_bytes,
                "active Runtime artifact activation receipt",
            )?;
            let expected = applied.candidate_slot_path.join(artifact_kind);
            if active != expected {
                return Err(format!(
                    "state=runtime-artifact-publication-failed reasonKind=active-applied-identity-mismatch active={} applied={}",
                    active.display(),
                    expected.display()
                ));
            }
            let candidate_target = std::fs::read_link(&expected).map_err(|error| {
                format!(
                    "state=runtime-artifact-publication-failed reasonKind=applied-candidate-slot-dangling path={} error={error}",
                    expected.display()
                )
            })?;
            if candidate_target != applied.artifact_path {
                return Err(format!(
                    "state=runtime-artifact-publication-failed reasonKind=applied-artifact-identity-mismatch candidate={} applied={}",
                    candidate_target.display(),
                    applied.artifact_path.display()
                ));
            }
            Ok(Some(applied.artifact_digest))
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeArtifactActivationCommitReceipt {
    pub artifact_digest: Blake3ContentDigest,
    pub previous_serving_digest: Option<Blake3ContentDigest>,
    pub activation_generation: u64,
    pub state: String,
}

pub async fn commit_runtime_artifact_activation(
    state_home: &Path,
    event: &RuntimeArtifactActivationEvent,
    serving_digest: Option<&Blake3ContentDigest>,
) -> Result<RuntimeArtifactActivationCommitReceipt, String> {
    validate_runtime_artifact_activation_event(event, "Runtime artifact activation event")?;
    let artifact_root = state_home.join("runtime/artifacts");
    let resident_root = state_home.join("runtime/resident");
    let artifact_kind = event
        .candidate_identity
        .stable_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "Runtime activation target has no artifact kind".to_owned())?;
    let slots = RuntimeArtifactSlotAuthority::for_artifact(&resident_root, artifact_kind);
    // Content validation is deliberately outside the mutation guard. The candidate is
    // immutable and the guard later binds the already-validated event to the canonical
    // applied receipt and active slot identity.
    let event_artifact_digest = runtime_artifact_candidate_digest(&event.artifact_path).await?;
    if event_artifact_digest != event.artifact_digest {
        return Err(format!(
            "state=runtime-artifact-activation-failed reasonKind=artifact-bytes-digest-mismatch expected={} observed={event_artifact_digest}",
            event.artifact_digest
        ));
    }
    let candidate_artifact = event.candidate_slot_path.join(artifact_kind);
    let candidate_artifact_digest = runtime_artifact_candidate_digest(&candidate_artifact).await?;
    if candidate_artifact_digest != event.artifact_digest {
        return Err(format!(
            "state=runtime-artifact-activation-failed reasonKind=candidate-slot-digest-mismatch expected={} observed={candidate_artifact_digest}",
            event.artifact_digest
        ));
    }
    let guard = RuntimeArtifactMutationGuard::try_acquire(&artifact_root)?;
    let derived_serving_digest = previous_serving_digest_under_guard(&slots, artifact_kind).await?;
    if event.previous_artifact_digest.as_ref() != derived_serving_digest.as_ref() {
        return Err(format!(
            "state=runtime-artifact-activation-failed reasonKind=serving-artifact-identity-mismatch expected={:?} observed={:?}",
            event.previous_artifact_digest, derived_serving_digest
        ));
    }
    if let Some(caller_digest) = serving_digest
        && Some(caller_digest) != derived_serving_digest.as_ref()
    {
        return Err(format!(
            "state=runtime-artifact-activation-failed reasonKind=serving-artifact-caller-drift expected={:?} observed={caller_digest}",
            derived_serving_digest
        ));
    }
    let pending_path = runtime_artifact_activation_event_path(state_home);
    let pending_bytes = std::fs::read(&pending_path).map_err(|error| {
        format!(
            "state=runtime-artifact-activation-failed reasonKind=pending-activation-unavailable path={} error={error}",
            pending_path.display()
        )
    })?;
    let current_pending = decode_runtime_artifact_activation_event(
        &pending_bytes,
        "current pending Runtime artifact activation event",
    )?;
    if current_pending.activation_generation != event.activation_generation
        || current_pending.artifact_digest != event.artifact_digest
        || current_pending.publication_nonce != event.publication_nonce
    {
        return Err(format!(
            "state=runtime-artifact-activation-failed reasonKind=stale-activation-generation attempted={} current={}",
            event.activation_generation, current_pending.activation_generation
        ));
    }
    let active_before = slots.active_target().await?;
    let healthy_before = slots.healthy_target().await?;
    let stable_before = match std::fs::read_link(&event.candidate_identity.stable_path) {
        Ok(target) => Some(target),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            return Err(format!(
                "state=runtime-artifact-activation-failed reasonKind=stable-alias-unreadable path={} error={error}",
                event.candidate_identity.stable_path.display()
            ));
        }
    };
    let applied_path = state_home.join("runtime/activation/applied.json");
    let applied_before = match std::fs::read(&applied_path) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            return Err(format!(
                "state=runtime-artifact-activation-failed reasonKind=applied-receipt-unreadable path={} error={error}",
                applied_path.display()
            ));
        }
    };
    let commit = async {
        slots.commit_ready(&candidate_artifact).await?;
        publish_resident_runtime_alias(&event.candidate_identity.stable_path, &resident_root)
            .await?;
        publish_applied_runtime_artifact_activation(state_home, event)
    }
    .await;
    if let Err(error) = commit {
        let slot_restore = slots
            .restore_targets(active_before.as_deref(), healthy_before.as_deref())
            .await;
        let alias_restore = restore_runtime_artifact_symlink(
            &event.candidate_identity.stable_path,
            stable_before.as_deref(),
            &event.publication_nonce,
        );
        let applied_restore = restore_applied_runtime_artifact_activation(
            &applied_path,
            applied_before.as_deref(),
            &event.publication_nonce,
        );
        if let Err(restore_error) = slot_restore.and(alias_restore).and(applied_restore) {
            return Err(format!(
                "{error}; state=runtime-artifact-activation-failed reasonKind=rollback-failed error={restore_error}"
            ));
        }
        return Err(error);
    }
    drop(guard);

    slots.prune_unreachable_publications().await?;
    Ok(RuntimeArtifactActivationCommitReceipt {
        artifact_digest: event.artifact_digest.clone(),
        previous_serving_digest: derived_serving_digest,
        activation_generation: event.activation_generation,
        state: "applied".to_owned(),
    })
}

fn restore_runtime_artifact_symlink(
    path: &Path,
    target: Option<&Path>,
    publication_nonce: &str,
) -> Result<(), String> {
    match target {
        Some(target) => {
            let parent = path.parent().ok_or_else(|| {
                format!("Runtime artifact alias has no parent: {}", path.display())
            })?;
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("create Runtime artifact alias parent: {error}"))?;
            let staged = parent.join(format!(".rollback-{publication_nonce}.tmp"));
            let _ = std::fs::remove_file(&staged);
            #[cfg(unix)]
            std::os::unix::fs::symlink(target, &staged)
                .map_err(|error| format!("stage Runtime artifact alias rollback: {error}"))?;
            #[cfg(not(unix))]
            return Err("Runtime artifact alias rollback requires symlink support".to_owned());
            std::fs::rename(&staged, path)
                .map_err(|error| format!("publish Runtime artifact alias rollback: {error}"))
        }
        None => match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!(
                "remove Runtime artifact alias during rollback: {error}"
            )),
        },
    }
}

fn restore_applied_runtime_artifact_activation(
    path: &Path,
    previous: Option<&[u8]>,
    publication_nonce: &str,
) -> Result<(), String> {
    match previous {
        Some(bytes) => {
            let parent = path.parent().ok_or_else(|| {
                format!("Runtime applied receipt has no parent: {}", path.display())
            })?;
            let staged = parent.join(format!(".applied-rollback-{publication_nonce}.tmp"));
            std::fs::write(&staged, bytes)
                .map_err(|error| format!("stage Runtime applied receipt rollback: {error}"))?;
            std::fs::rename(&staged, path)
                .map_err(|error| format!("publish Runtime applied receipt rollback: {error}"))
        }
        None => match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!(
                "remove Runtime applied receipt during rollback: {error}"
            )),
        },
    }
}

fn publish_applied_runtime_artifact_activation(
    state_home: &Path,
    event: &RuntimeArtifactActivationEvent,
) -> Result<(), String> {
    let applied = state_home.join("runtime/activation/applied.json");
    let applied_parent = applied.parent().ok_or_else(|| {
        format!(
            "Runtime activation acknowledgement has no parent: {}",
            applied.display()
        )
    })?;
    std::fs::create_dir_all(applied_parent)
        .map_err(|error| format!("create Runtime activation acknowledgement directory: {error}"))?;
    let bytes = serde_json::to_vec_pretty(event)
        .map_err(|error| format!("encode applied Runtime artifact activation event: {error}"))?;
    let staged = applied_parent.join(format!(".applied-{}.json.tmp", event.publication_nonce));
    let mut staged_file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&staged)
        .map_err(|error| format!("create Runtime activation acknowledgement stage: {error}"))?;
    std::io::Write::write_all(&mut staged_file, &bytes)
        .map_err(|error| format!("stage Runtime activation acknowledgement: {error}"))?;
    staged_file
        .sync_all()
        .map_err(|error| format!("sync Runtime activation acknowledgement stage: {error}"))?;
    std::fs::rename(&staged, &applied)
        .map_err(|error| format!("publish Runtime activation acknowledgement: {error}"))?;
    std::fs::File::open(applied_parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| format!("sync Runtime activation acknowledgement directory: {error}"))?;
    let pending = runtime_artifact_activation_event_path(state_home);
    match std::fs::remove_file(pending) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("retire Runtime activation pending event: {error}")),
    }
}

pub async fn acknowledge_runtime_artifact_activation(
    state_home: &Path,
    artifact_digest: &Blake3ContentDigest,
) -> Result<(), String> {
    let Some(event) = read_runtime_artifact_activation_event(state_home).await? else {
        return Ok(());
    };
    if &event.artifact_digest != artifact_digest {
        return Err(format!(
            "Runtime artifact activation acknowledgement mismatch: expected={} observed={}",
            event.artifact_digest, artifact_digest
        ));
    }
    let applied = state_home.join("runtime/activation/applied.json");
    let applied_parent = applied.parent().ok_or_else(|| {
        format!(
            "Runtime activation acknowledgement has no parent: {}",
            applied.display()
        )
    })?;
    tokio::fs::create_dir_all(applied_parent)
        .await
        .map_err(|error| format!("create Runtime activation acknowledgement directory: {error}"))?;
    let bytes = serde_json::to_vec_pretty(&event)
        .map_err(|error| format!("encode applied Runtime artifact activation event: {error}"))?;
    let staged = applied_parent.join(".applied.json.tmp");
    tokio::fs::write(&staged, bytes)
        .await
        .map_err(|error| format!("stage Runtime activation acknowledgement: {error}"))?;
    tokio::fs::rename(&staged, &applied)
        .await
        .map_err(|error| format!("publish Runtime activation acknowledgement: {error}"))
}

pub async fn publish_runtime_artifact(
    state_home: &Path,
    source: &Path,
    target: &Path,
    artifact_mode: &str,
) -> Result<RuntimeArtifactPublicationReceipt, String> {
    publish_runtime_artifact_with_before_guard(
        state_home,
        source,
        target,
        artifact_mode,
        || async {},
    )
    .await
}

async fn publish_runtime_artifact_with_before_guard<BeforeGuard, BeforeGuardFuture>(
    state_home: &Path,
    source: &Path,
    target: &Path,
    artifact_mode: &str,
    before_guard: BeforeGuard,
) -> Result<RuntimeArtifactPublicationReceipt, String>
where
    BeforeGuard: FnOnce() -> BeforeGuardFuture,
    BeforeGuardFuture: std::future::Future<Output = ()>,
{
    let artifact_root = state_home.join("runtime/artifacts");
    let resident_root = state_home.join("runtime/resident");
    let digest = runtime_artifact_candidate_digest(source).await?;
    let token = digest.content_digest().as_str().to_owned();
    let binary_name = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "Runtime artifact target has no binary name".to_owned())?;
    let candidate_dir = resident_root
        .join("candidates")
        .join(binary_name)
        .join(&token);
    let slots = RuntimeArtifactSlotAuthority::for_artifact(&resident_root, binary_name);

    // Immutable materialization is deliberately outside the artifact mutation lock.
    let prepared = crate::runtime_artifact_catalog::prepare_runtime_artifact_candidate_for_kind(
        state_home,
        &candidate_dir,
        source,
        binary_name,
    )
    .await?;
    if let Err(error) = slots
        .stage_candidate_artifact(&candidate_dir, &prepared.path)
        .await
    {
        discard_prepared_runtime_artifact(&prepared).await?;
        return Err(error);
    }
    let activation_event_path = runtime_artifact_activation_event_path(state_home);

    let quiescence_operation = format!("publish:{binary_name}");
    let quiescence = match prepare_runtime_artifact_quiescence_lease(
        state_home,
        &quiescence_operation,
        &prepared.content_digest,
    )
    .await
    {
        Ok(quiescence) => quiescence,
        Err(error) => {
            discard_prepared_runtime_artifact(&prepared).await?;
            return Err(error);
        }
    };

    before_guard().await;

    // Pending publication and the canonical client launcher are one Artifacts-owned
    // transaction. Resident active/healthy remain the actor-owned serving authority.
    let lock_started = std::time::Instant::now();
    let mut phase_trace = RuntimeArtifactPublicationPhaseTrace::default();
    let phase_started = std::time::Instant::now();
    let guard = RuntimeArtifactMutationGuard::try_acquire(&artifact_root)?;
    phase_trace.guard_acquisition_micros = phase_started.elapsed().as_micros();
    let phase_started = std::time::Instant::now();
    let consumed_lease = match quiescence.consume_under_artifact_guard() {
        Ok(consumed) => consumed,
        Err(error) => {
            drop(guard);
            discard_prepared_runtime_artifact(&prepared).await?;
            return Err(error);
        }
    };
    phase_trace.lease_consume_micros = phase_started.elapsed().as_micros();
    let phase_started = std::time::Instant::now();
    let previous_artifact_digest =
        match previous_serving_digest_under_guard(&slots, binary_name).await {
            Ok(digest) => digest,
            Err(error) => {
                drop(guard);
                quiescence.restore_after_failed_commit(&consumed_lease)?;
                discard_prepared_runtime_artifact(&prepared).await?;
                return Err(error);
            }
        };
    phase_trace.receipt_read_validate_micros = phase_started.elapsed().as_micros();
    let transaction = (|| -> Result<(RuntimeArtifactActivationEvent, Vec<u8>), String> {
        let phase_started = std::time::Instant::now();
        let activation_generation =
            current_runtime_artifact_activation_generation_under_guard(state_home)?;
        phase_trace.generation_read_micros = phase_started.elapsed().as_micros();
        let publication_nonce = format!(
            "activation-{activation_generation}-{}",
            prepared.content_digest.content_digest().as_str()
        );
        let event = RuntimeArtifactActivationEvent {
            artifact_digest: prepared.content_digest.clone(),
            artifact_path: prepared.path.clone(),
            candidate_slot_path: candidate_dir.clone(),
            previous_artifact_digest,
            artifact_mode: artifact_mode.to_owned(),
            published_at_unix_millis: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|error| format!("Runtime artifact publication clock failed: {error}"))?
                .as_millis(),
            publication_nonce: publication_nonce.clone(),
            activation_generation,
            candidate_identity: RuntimeArtifactCandidateIdentityReceipt {
                artifact_digest: prepared.content_digest.clone(),
                artifact_path: prepared.path.clone(),
                stable_path: target.to_path_buf(),
                artifact_mode: artifact_mode.to_owned(),
                publication_nonce,
            },
        };
        let phase_started = std::time::Instant::now();
        let event_bytes = serde_json::to_vec_pretty(&event)
            .map_err(|error| format!("encode Runtime artifact activation event: {error}"))?;
        std::fs::write(candidate_dir.join("activation.json"), &event_bytes)
            .map_err(|error| format!("stage Runtime artifact activation event: {error}"))?;
        phase_trace.candidate_activation_staging_micros = phase_started.elapsed().as_micros();
        let previous_pending = match std::fs::read(&activation_event_path) {
            Ok(bytes) => Some(bytes),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => {
                return Err(format!(
                    "read previous Runtime artifact activation event {}: {error}",
                    activation_event_path.display()
                ));
            }
        };
        let phase_started = std::time::Instant::now();
        let commit = publish_pending_runtime_artifact_activation(
            &activation_event_path,
            &event_bytes,
            &event.publication_nonce,
        );
        phase_trace.pending_publication_micros = phase_started.elapsed().as_micros();
        let phase_started = std::time::Instant::now();
        let commit = commit.and_then(|()| {
            let result = publish_runtime_client_launcher(target, &event.artifact_path, &event);
            phase_trace.client_launcher_switch_micros = phase_started.elapsed().as_micros();
            result
        });
        if let Err(error) = commit {
            restore_pending_runtime_artifact_activation(
                &activation_event_path,
                previous_pending.as_deref(),
                &event.publication_nonce,
            )?;
            return Err(error);
        }
        Ok((event, event_bytes))
    })();
    drop(guard);
    let lock_elapsed_micros = lock_started.elapsed().as_micros();

    let (_event, event_bytes) = match transaction {
        Ok(committed) => committed,
        Err(error) => {
            quiescence.restore_after_failed_commit(&consumed_lease)?;
            discard_prepared_runtime_artifact(&prepared).await?;
            return Err(error);
        }
    };
    quiescence.finish_consumption(&consumed_lease)?;

    notify_runtime_artifact_activation(state_home, &event_bytes).await?;

    Ok(RuntimeArtifactPublicationReceipt {
        path: target.to_path_buf(),
        status: "published-activation-pending",
        artifact_digest: prepared.content_digest,
        activation_event_path,
        lock_elapsed_micros,
        phase_trace,
        lock_acquisition_count: 1,
        quiescence_operation,
        quiescence_lease_nonce: quiescence.lease.lease_nonce,
        lease_producer_process_id: quiescence.lease.producer_process_id,
        lease_consumer_process_id: std::process::id(),
    })
}

fn restore_pending_runtime_artifact_activation(
    path: &Path,
    previous: Option<&[u8]>,
    publication_nonce: &str,
) -> Result<(), String> {
    match previous {
        Some(bytes) => publish_pending_runtime_artifact_activation(path, bytes, publication_nonce),
        None => match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!(
                "rollback Runtime artifact activation event {}: {error}",
                path.display()
            )),
        },
    }
}

fn publish_runtime_client_launcher(
    target: &Path,
    candidate: &Path,
    event: &RuntimeArtifactActivationEvent,
) -> Result<(), String> {
    if event.candidate_identity.stable_path != target
        || event.candidate_identity.artifact_path != candidate
        || event.candidate_identity.artifact_digest != event.artifact_digest
    {
        return Err("reasonKind=runtime-client-launcher-activation-binding-mismatch".to_owned());
    }
    let parent = target.parent().ok_or_else(|| {
        format!(
            "Runtime client launcher has no parent: {}",
            target.display()
        )
    })?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("create Runtime client launcher directory: {error}"))?;
    let staged = parent.join(format!(
        ".asp.activation-{}.tmp",
        event.activation_generation
    ));
    match std::fs::remove_file(&staged) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("remove stale Runtime client launcher: {error}")),
    }
    stage_runtime_client_launcher(candidate, &staged)?;
    let expected = std::fs::canonicalize(candidate)
        .map_err(|error| format!("resolve Runtime client candidate: {error}"))?;
    let observed = std::fs::canonicalize(&staged)
        .map_err(|error| format!("resolve staged Runtime client launcher: {error}"))?;
    if observed != expected {
        let _ = std::fs::remove_file(&staged);
        return Err("reasonKind=runtime-client-launcher-candidate-mismatch".to_owned());
    }
    std::fs::rename(&staged, target).map_err(|error| {
        let _ = std::fs::remove_file(&staged);
        format!(
            "reasonKind=runtime-client-launcher-publication-failed target={} error={error}",
            target.display()
        )
    })
}

#[cfg(unix)]
fn stage_runtime_client_launcher(candidate: &Path, staged: &Path) -> Result<(), String> {
    std::os::unix::fs::symlink(candidate, staged)
        .map_err(|error| format!("stage Runtime client launcher: {error}"))
}

#[cfg(not(unix))]
fn stage_runtime_client_launcher(candidate: &Path, staged: &Path) -> Result<(), String> {
    std::fs::copy(candidate, staged)
        .map(|_| ())
        .map_err(|error| format!("stage Runtime client launcher: {error}"))
}

async fn notify_runtime_artifact_activation(
    state_home: &Path,
    event_bytes: &[u8],
) -> Result<(), String> {
    #[cfg(unix)]
    {
        let socket = tokio::net::UnixDatagram::unbound()
            .map_err(|error| format!("create Runtime artifact activation notifier: {error}"))?;
        let endpoint = runtime_artifact_activation_socket_path(state_home);
        match socket.send_to(event_bytes, &endpoint).await {
            Ok(_) => Ok(()),
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
                ) =>
            {
                Ok(())
            }
            Err(error) => Err(format!("notify Runtime artifact activation actor: {error}")),
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (state_home, event_bytes);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    fn write_executable(path: &Path, body: &str) {
        std::fs::write(path, body).expect("write fixture executable");
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
            .expect("fixture executable permissions");
    }

    #[tokio::test]
    async fn publication_does_not_launch_or_wait_for_a_runtime_candidate() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let state_home = temporary.path().join("state");
        let source = temporary.path().join("asp");
        let target = temporary.path().join("bin/asp");
        tokio::fs::write(&source, b"#!/bin/sh\nexit 99\n")
            .await
            .expect("write artifact");
        std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o755))
            .expect("artifact permissions");

        let receipt = publish_runtime_artifact(&state_home, &source, &target, "dev")
            .await
            .expect("artifact publication must not launch the candidate");

        assert_eq!(receipt.status, "published-activation-pending");
        assert!(receipt.activation_event_path.is_file());
        assert!(receipt.lock_elapsed_micros < 10_000);
        let artifact_root = state_home.join("runtime/artifacts");
        let guard = RuntimeArtifactMutationGuard::try_acquire(&artifact_root)
            .expect("artifact lock must be released when publication returns");
        drop(guard);
    }

    #[tokio::test]
    async fn production_publication_switches_client_to_pending_and_keeps_serving_previous() {
        let temporary = tempfile::tempdir().expect("production-shaped publication fixture");
        let state_home = temporary.path().join("state");
        let previous_source = temporary.path().join("asp-previous");
        let pending_source = temporary.path().join("asp-pending");
        let target = state_home.join("runtime/bin/asp");
        write_executable(&previous_source, "#!/bin/sh\nexit 21\n");
        write_executable(&pending_source, "#!/bin/sh\nexit 28\n");

        publish_runtime_artifact(&state_home, &previous_source, &target, "release")
            .await
            .expect("publish previous generation");
        let previous = read_runtime_artifact_activation_event(&state_home)
            .await
            .unwrap()
            .unwrap();
        commit_runtime_artifact_activation(&state_home, &previous, None)
            .await
            .expect("commit previous serving generation");
        let slots =
            RuntimeArtifactSlotAuthority::for_artifact(&state_home.join("runtime/resident"), "asp");
        let active_before = slots.active_target().await.unwrap();
        let healthy_before = slots.healthy_target().await.unwrap();

        publish_runtime_artifact(&state_home, &pending_source, &target, "release")
            .await
            .expect("publish pending generation");
        let pending = read_runtime_artifact_activation_event(&state_home)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(
            std::fs::canonicalize(&target).unwrap(),
            std::fs::canonicalize(&pending.artifact_path).unwrap()
        );
        assert_eq!(slots.active_target().await.unwrap(), active_before);
        assert_eq!(slots.healthy_target().await.unwrap(), healthy_before);
        assert_eq!(
            pending.previous_artifact_digest,
            Some(previous.artifact_digest)
        );
    }

    #[tokio::test]
    async fn publication_derives_previous_serving_after_acquiring_the_mutation_guard() {
        let temporary = tempfile::tempdir().expect("publication interleaving fixture");
        let state_home = temporary.path().join("state");
        let old_source = temporary.path().join("asp-old");
        let new_serving = temporary.path().join("asp-new-serving");
        let pending_source = temporary.path().join("asp-pending");
        let target = state_home.join("runtime/bin/asp");
        write_executable(&old_source, "#!/bin/sh\nexit 10\n");
        write_executable(&new_serving, "#!/bin/sh\nexit 11\n");
        write_executable(&pending_source, "#!/bin/sh\nexit 12\n");

        publish_runtime_artifact(&state_home, &old_source, &target, "release")
            .await
            .expect("publish old serving generation");
        let old = read_runtime_artifact_activation_event(&state_home)
            .await
            .expect("read old pending event")
            .expect("old pending event");
        commit_runtime_artifact_activation(&state_home, &old, None)
            .await
            .expect("commit old serving generation");

        let new_serving_digest = runtime_artifact_candidate_digest(&new_serving)
            .await
            .expect("digest newer serving identity");
        let new_token = new_serving_digest.content_digest().as_str();
        let new_candidate_dir = state_home
            .join("runtime/resident/candidates/asp")
            .join(new_token);
        let new_prepared =
            crate::runtime_artifact_catalog::prepare_runtime_artifact_candidate_for_kind(
                &state_home,
                &new_candidate_dir,
                &new_serving,
                "asp",
            )
            .await
            .expect("prepare newer serving identity");
        let slots =
            RuntimeArtifactSlotAuthority::for_artifact(state_home.join("runtime/resident"), "asp");
        slots
            .stage_candidate_artifact(&new_candidate_dir, &new_prepared.path)
            .await
            .expect("stage newer serving identity");
        let new_nonce = format!(
            "activation-{}-{}",
            old.activation_generation + 1,
            new_serving_digest.content_digest().as_str()
        );
        let new_applied = RuntimeArtifactActivationEvent {
            artifact_digest: new_serving_digest.clone(),
            artifact_path: new_prepared.path,
            candidate_slot_path: new_candidate_dir.clone(),
            previous_artifact_digest: Some(old.artifact_digest.clone()),
            artifact_mode: "release".to_owned(),
            published_at_unix_millis: old.published_at_unix_millis + 1,
            publication_nonce: new_nonce.clone(),
            activation_generation: old.activation_generation + 1,
            candidate_identity: RuntimeArtifactCandidateIdentityReceipt {
                artifact_digest: new_serving_digest.clone(),
                artifact_path: new_candidate_dir.join("asp").read_link().unwrap(),
                stable_path: target.clone(),
                artifact_mode: "release".to_owned(),
                publication_nonce: new_nonce,
            },
        };
        std::fs::write(
            new_candidate_dir.join("activation.json"),
            serde_json::to_vec_pretty(&new_applied).unwrap(),
        )
        .expect("stage the newer serving receipt before actor commit");

        let (before_guard_tx, before_guard_rx) = tokio::sync::oneshot::channel();
        let (continue_tx, continue_rx) = tokio::sync::oneshot::channel();
        let publish_state_home = state_home.clone();
        let publish_target = target.clone();
        let publisher = tokio::spawn(async move {
            publish_runtime_artifact_with_before_guard(
                &publish_state_home,
                &pending_source,
                &publish_target,
                "release",
                move || async move {
                    before_guard_tx
                        .send(())
                        .expect("signal publisher reached pre-guard seam");
                    continue_rx
                        .await
                        .expect("release publisher after active switch");
                },
            )
            .await
        });
        before_guard_rx
            .await
            .expect("publisher reaches deterministic pre-guard seam");

        let artifact_root = state_home.join("runtime/artifacts");
        let guard = RuntimeArtifactMutationGuard::try_acquire(&artifact_root)
            .expect("test actor acquires the shared mutation guard");
        slots
            .commit_ready(&new_candidate_dir.join("asp"))
            .await
            .expect("actor commits the newer serving identity under the guard");
        publish_applied_runtime_artifact_activation(&state_home, &new_applied)
            .expect("actor publishes the matching canonical applied receipt under the guard");
        drop(guard);
        continue_tx
            .send(())
            .expect("continue publisher after actor commit");
        publisher
            .await
            .expect("publisher task joins")
            .expect("publisher commits pending event");

        let pending = read_runtime_artifact_activation_event(&state_home)
            .await
            .expect("read interleaved pending event")
            .expect("interleaved pending event");
        assert_eq!(pending.previous_artifact_digest, Some(new_serving_digest));
        assert_ne!(pending.previous_artifact_digest, Some(old.artifact_digest));
    }

    #[tokio::test]
    async fn alias_and_malformed_state_failures_preserve_previous_client_and_serving() {
        let temporary = tempfile::tempdir().expect("failure preservation fixture");
        let state_home = temporary.path().join("state");
        let previous_source = temporary.path().join("asp-previous");
        let next_source = temporary.path().join("asp-next");
        let target = state_home.join("runtime/bin/asp");
        write_executable(&previous_source, "#!/bin/sh\nexit 0\n");
        write_executable(&next_source, "#!/bin/sh\nexit 1\n");
        publish_runtime_artifact(&state_home, &previous_source, &target, "release")
            .await
            .unwrap();
        let previous = read_runtime_artifact_activation_event(&state_home)
            .await
            .unwrap()
            .unwrap();
        commit_runtime_artifact_activation(&state_home, &previous, None)
            .await
            .unwrap();
        let client_before = std::fs::canonicalize(&target).unwrap();
        let slots =
            RuntimeArtifactSlotAuthority::for_artifact(&state_home.join("runtime/resident"), "asp");
        let active_before = slots.active_target().await.unwrap();
        let healthy_before = slots.healthy_target().await.unwrap();

        let staged_conflict = target.parent().unwrap().join(format!(
            ".asp.activation-{}.tmp",
            previous.activation_generation + 1
        ));
        std::fs::create_dir(&staged_conflict).expect("create deterministic alias conflict");
        let alias_error = publish_runtime_artifact(&state_home, &next_source, &target, "release")
            .await
            .expect_err("alias failure must fail closed");
        assert!(alias_error.contains("remove stale Runtime client launcher"));
        assert_eq!(std::fs::canonicalize(&target).unwrap(), client_before);
        assert!(
            read_runtime_artifact_activation_event(&state_home)
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(slots.active_target().await.unwrap(), active_before);
        assert_eq!(slots.healthy_target().await.unwrap(), healthy_before);

        std::fs::remove_dir(staged_conflict).unwrap();
        std::fs::write(
            runtime_artifact_activation_event_path(&state_home),
            b"{malformed",
        )
        .unwrap();
        let malformed = publish_runtime_artifact(&state_home, &next_source, &target, "release")
            .await
            .expect_err("malformed pending must fail closed");
        assert!(malformed.contains("decode Runtime artifact activation event"));
        assert_eq!(std::fs::canonicalize(&target).unwrap(), client_before);
        assert_eq!(slots.active_target().await.unwrap(), active_before);
        assert_eq!(slots.healthy_target().await.unwrap(), healthy_before);
    }

    #[tokio::test]
    async fn stale_actor_commit_cannot_overwrite_newer_pending_generation() {
        let temporary = tempfile::tempdir().expect("actor race fixture");
        let state_home = temporary.path().join("state");
        let target = state_home.join("runtime/bin/asp");
        let sources = (0..3)
            .map(|generation| {
                let source = temporary.path().join(format!("asp-{generation}"));
                write_executable(&source, &format!("#!/bin/sh\nexit {generation}\n"));
                source
            })
            .collect::<Vec<_>>();
        publish_runtime_artifact(&state_home, &sources[0], &target, "release")
            .await
            .unwrap();
        let applied = read_runtime_artifact_activation_event(&state_home)
            .await
            .unwrap()
            .unwrap();
        commit_runtime_artifact_activation(&state_home, &applied, None)
            .await
            .unwrap();
        publish_runtime_artifact(&state_home, &sources[1], &target, "release")
            .await
            .unwrap();
        let older = read_runtime_artifact_activation_event(&state_home)
            .await
            .unwrap()
            .unwrap();
        publish_runtime_artifact(&state_home, &sources[2], &target, "release")
            .await
            .unwrap();
        let newer = read_runtime_artifact_activation_event(&state_home)
            .await
            .unwrap()
            .unwrap();

        let stale =
            commit_runtime_artifact_activation(&state_home, &older, Some(&applied.artifact_digest))
                .await
                .expect_err("old actor commit must be rejected");
        assert!(stale.contains("reasonKind=stale-activation-generation"));
        assert_eq!(
            read_runtime_artifact_activation_event(&state_home)
                .await
                .unwrap()
                .unwrap(),
            newer
        );
        assert_eq!(
            std::fs::canonicalize(&target).unwrap(),
            std::fs::canonicalize(&newer.artifact_path).unwrap()
        );

        commit_runtime_artifact_activation(&state_home, &newer, Some(&applied.artifact_digest))
            .await
            .expect("new actor commit converges");
        let applied_after = read_applied_runtime_artifact_activation_event(&state_home)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            applied_after.activation_generation,
            newer.activation_generation
        );
        assert_eq!(applied_after.artifact_digest, newer.artifact_digest);
    }

    #[tokio::test]
    async fn applied_receipt_retires_only_the_matching_pending_generation() {
        let temporary = tempfile::tempdir().expect("activation transaction fixture");
        let state_home = temporary.path().join("state");
        let source = temporary.path().join("asp");
        let target = temporary.path().join("bin/asp");
        std::fs::write(&source, b"#!/bin/sh\nexit 99\n").expect("write fixture executable");
        let mut permissions = std::fs::metadata(&source)
            .expect("fixture executable metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&source, permissions).expect("mark fixture executable");

        publish_runtime_artifact(&state_home, &source, &target, "dev")
            .await
            .expect("publish pending activation");
        let pending = read_runtime_artifact_activation_event(&state_home)
            .await
            .expect("read pending activation")
            .expect("pending activation generation");
        assert!(
            read_applied_runtime_artifact_activation_event(&state_home)
                .await
                .expect("read absent applied activation")
                .is_none()
        );

        let committed = commit_runtime_artifact_activation(&state_home, &pending, None)
            .await
            .expect("commit matching activation generation");
        assert_eq!(committed.state, "applied");
        assert_eq!(committed.artifact_digest, pending.artifact_digest);
        assert_eq!(
            committed.activation_generation,
            pending.activation_generation
        );
        let applied = read_applied_runtime_artifact_activation_event(&state_home)
            .await
            .expect("read applied activation")
            .expect("applied activation generation");
        assert_eq!(applied.artifact_digest, pending.artifact_digest);
        assert_eq!(applied.activation_generation, pending.activation_generation);
        assert!(
            read_runtime_artifact_activation_event(&state_home)
                .await
                .expect("read retired pending activation")
                .is_none()
        );

        publish_runtime_artifact(&state_home, &source, &target, "dev")
            .await
            .expect("republish the same content as a newer activation generation");
        let republished = read_runtime_artifact_activation_event(&state_home)
            .await
            .expect("read republished activation")
            .expect("a stale applied generation must not hide a newer pending generation");
        assert_eq!(republished.artifact_digest, pending.artifact_digest);
        assert!(republished.activation_generation > pending.activation_generation);
        commit_runtime_artifact_activation(
            &state_home,
            &republished,
            Some(&pending.artifact_digest),
        )
        .await
        .expect("commit the newer matching activation generation");
        assert!(
            read_runtime_artifact_activation_event(&state_home)
                .await
                .expect("read second retired pending activation")
                .is_none()
        );

        std::fs::write(&source, b"#!/bin/sh\nexit 98\n").expect("write distinct fixture content");
        publish_runtime_artifact(&state_home, &source, &target, "dev")
            .await
            .expect("publish distinct content as the next activation generation");
        let distinct = read_runtime_artifact_activation_event(&state_home)
            .await
            .expect("read distinct pending activation")
            .expect("an applied digest must not hide distinct pending content");
        assert_ne!(distinct.artifact_digest, republished.artifact_digest);
        assert!(distinct.activation_generation > republished.activation_generation);
    }

    #[tokio::test]
    async fn digest_validation_failure_preserves_lease_and_artifact_slots() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let state_home = temporary.path().join("state");
        let initial_source = temporary.path().join("asp-initial");
        let next_source = temporary.path().join("asp-next");
        let target = temporary.path().join("bin/asp");
        tokio::fs::write(&initial_source, b"#!/bin/sh\nexit 0\n")
            .await
            .expect("write initial artifact");
        tokio::fs::write(&next_source, b"#!/bin/sh\nexit 1\n")
            .await
            .expect("write next artifact");
        for source in [&initial_source, &next_source] {
            std::fs::set_permissions(source, std::fs::Permissions::from_mode(0o755))
                .expect("artifact permissions");
        }

        publish_runtime_artifact(&state_home, &initial_source, &target, "dev")
            .await
            .expect("initial publication");
        let slots = RuntimeArtifactSlotAuthority::new(state_home.join("runtime/resident"));
        let active_before = slots.active_target().await.expect("active before");
        let healthy_before = slots.healthy_target().await.expect("healthy before");

        let precise_digest = Blake3ContentDigest::parse(
            "blake3-256:9313893f2985088dc3e5b14fdfd4877b0ed37c8dc8d5ef60bd6531ab5678abe1",
        )
        .expect("precise typed digest");
        let prepared_lease =
            prepare_runtime_artifact_quiescence_lease(&state_home, "publish:asp", &precise_digest)
                .await
                .expect("prepare mismatched lease");
        let lease_path =
            crate::runtime_artifact_quiescence::runtime_artifact_quiescence_lease_path(&state_home);
        let lease_before = tokio::fs::read(&lease_path).await.expect("lease before");

        let error = publish_runtime_artifact(&state_home, &next_source, &target, "dev")
            .await
            .expect_err("mismatched typed digest must fail before slot mutation");
        assert!(error.contains("reasonKind=runtime-artifact-quiescence-identity-mismatch"));
        assert_eq!(
            slots.active_target().await.expect("active after"),
            active_before
        );
        assert_eq!(
            slots.healthy_target().await.expect("healthy after"),
            healthy_before
        );
        assert_eq!(
            tokio::fs::read(&lease_path).await.expect("lease after"),
            lease_before
        );

        let consumed = prepared_lease
            .consume_under_artifact_guard()
            .expect("preserved lease remains consumable exactly once");
        prepared_lease
            .finish_consumption(&consumed)
            .expect("finalize preserved lease");
        assert!(prepared_lease.consume_under_artifact_guard().is_err());
    }

    #[tokio::test]
    async fn malformed_active_receipt_restores_lease_and_discards_prepared_candidate() {
        let temporary = tempfile::tempdir().expect("malformed active receipt fixture");
        let state_home = temporary.path().join("state");
        let initial_source = temporary.path().join("asp-initial");
        let next_source = temporary.path().join("asp-next");
        let target = state_home.join("runtime/bin/asp");
        write_executable(&initial_source, "#!/bin/sh\nexit 0\n");
        write_executable(&next_source, "#!/bin/sh\nexit 1\n");

        publish_runtime_artifact(&state_home, &initial_source, &target, "release")
            .await
            .expect("publish initial activation");
        let initial = read_runtime_artifact_activation_event(&state_home)
            .await
            .unwrap()
            .unwrap();
        commit_runtime_artifact_activation(&state_home, &initial, None)
            .await
            .expect("commit initial activation");
        let slots = RuntimeArtifactSlotAuthority::new(state_home.join("runtime/resident"));
        let active_before = slots.active_target().await.unwrap();
        let healthy_before = slots.healthy_target().await.unwrap();
        let client_before = std::fs::canonicalize(&target).unwrap();
        let active_receipt_path = initial.candidate_slot_path.join("activation.json");
        let active_receipt_before = std::fs::read(&active_receipt_path).unwrap();
        std::fs::write(&active_receipt_path, b"{malformed").unwrap();
        let next_digest = runtime_artifact_candidate_digest(&next_source)
            .await
            .unwrap();
        let next_artifact = state_home
            .join("runtime/artifacts/blake3-256")
            .join(next_digest.content_digest().as_str())
            .join("asp");

        let error = publish_runtime_artifact(&state_home, &next_source, &target, "release")
            .await
            .expect_err("malformed active receipt must fail closed");
        assert!(error.contains("decode active Runtime artifact activation receipt"));
        assert_eq!(slots.active_target().await.unwrap(), active_before);
        assert_eq!(slots.healthy_target().await.unwrap(), healthy_before);
        assert_eq!(std::fs::canonicalize(&target).unwrap(), client_before);
        assert!(!next_artifact.exists());
        assert!(
            crate::runtime_artifact_quiescence::runtime_artifact_quiescence_lease_path(&state_home)
                .is_file(),
            "consumed lease must be restored after a typed derivation failure"
        );
        let guard =
            RuntimeArtifactMutationGuard::try_acquire(&state_home.join("runtime/artifacts"))
                .expect("the failed writer must release the mutation guard");
        drop(guard);

        std::fs::write(&active_receipt_path, active_receipt_before).unwrap();
        publish_runtime_artifact(&state_home, &next_source, &target, "release")
            .await
            .expect("a later writer can publish after canonical state is restored");
    }

    #[tokio::test]
    async fn dangling_and_missing_active_receipts_fail_closed_without_hashing_bytes() {
        let temporary = tempfile::tempdir().expect("active half-state fixture");
        let state_home = temporary.path().join("state");
        let initial_source = temporary.path().join("asp-initial");
        let next_source = temporary.path().join("asp-next");
        let target = state_home.join("runtime/bin/asp");
        write_executable(&initial_source, "#!/bin/sh\nexit 0\n");
        write_executable(&next_source, "#!/bin/sh\nexit 1\n");
        publish_runtime_artifact(&state_home, &initial_source, &target, "release")
            .await
            .unwrap();
        let initial = read_runtime_artifact_activation_event(&state_home)
            .await
            .unwrap()
            .unwrap();
        commit_runtime_artifact_activation(&state_home, &initial, None)
            .await
            .unwrap();
        let slots = RuntimeArtifactSlotAuthority::new(state_home.join("runtime/resident"));
        let active_path = slots.active_path();
        let active_target = std::fs::read_link(&active_path).unwrap();
        let active_receipt_path = active_target.parent().unwrap().join("activation.json");
        let active_receipt = std::fs::read(&active_receipt_path).unwrap();
        std::fs::remove_file(&active_receipt_path).unwrap();

        let missing = publish_runtime_artifact(&state_home, &next_source, &target, "release")
            .await
            .expect_err("active slot without its bound receipt must fail closed");
        assert!(missing.contains("reasonKind=active-receipt-unreadable"));
        std::fs::write(&active_receipt_path, active_receipt).unwrap();

        std::fs::remove_file(&active_path).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(
            state_home.join("runtime/resident/candidates/asp/missing/asp"),
            &active_path,
        )
        .unwrap();
        let dangling = publish_runtime_artifact(&state_home, &next_source, &target, "release")
            .await
            .expect_err("dangling active slot must fail closed");
        assert!(
            dangling.contains("reasonKind=active-applied-identity-mismatch")
                || dangling.contains("reasonKind=active-slot-dangling")
                || dangling.contains("reasonKind=active-receipt-unreadable")
        );

        std::fs::remove_file(&active_path).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(active_target, &active_path).unwrap();
        publish_runtime_artifact(&state_home, &next_source, &target, "release")
            .await
            .expect("restored canonical state admits the next writer");
    }

    #[tokio::test]
    async fn applied_receipt_commit_failure_rolls_back_slots_alias_and_receipt() {
        let temporary = tempfile::tempdir().expect("activation rollback fixture");
        let state_home = temporary.path().join("state");
        let initial_source = temporary.path().join("asp-initial");
        let next_source = temporary.path().join("asp-next");
        let target = state_home.join("runtime/bin/asp");
        write_executable(&initial_source, "#!/bin/sh\nexit 0\n");
        write_executable(&next_source, "#!/bin/sh\nexit 1\n");
        publish_runtime_artifact(&state_home, &initial_source, &target, "release")
            .await
            .unwrap();
        let initial = read_runtime_artifact_activation_event(&state_home)
            .await
            .unwrap()
            .unwrap();
        commit_runtime_artifact_activation(&state_home, &initial, None)
            .await
            .unwrap();
        publish_runtime_artifact(&state_home, &next_source, &target, "release")
            .await
            .unwrap();
        let pending = read_runtime_artifact_activation_event(&state_home)
            .await
            .unwrap()
            .unwrap();
        let slots = RuntimeArtifactSlotAuthority::new(state_home.join("runtime/resident"));
        let active_before = slots.active_target().await.unwrap();
        let healthy_before = slots.healthy_target().await.unwrap();
        let client_before = std::fs::read_link(&target).unwrap();
        let applied_path = state_home.join("runtime/activation/applied.json");
        let applied_before = std::fs::read(&applied_path).unwrap();
        let staged = applied_path
            .parent()
            .unwrap()
            .join(format!(".applied-{}.json.tmp", pending.publication_nonce));
        std::fs::create_dir(&staged).expect("create deterministic applied stage conflict");

        let error = commit_runtime_artifact_activation(
            &state_home,
            &pending,
            Some(&initial.artifact_digest),
        )
        .await
        .expect_err("applied receipt stage failure must roll back the transaction");
        assert!(error.contains("create Runtime activation acknowledgement stage"));
        assert_eq!(slots.active_target().await.unwrap(), active_before);
        assert_eq!(slots.healthy_target().await.unwrap(), healthy_before);
        assert_eq!(std::fs::read_link(&target).unwrap(), client_before);
        assert_eq!(std::fs::read(&applied_path).unwrap(), applied_before);
        assert_eq!(
            read_runtime_artifact_activation_event(&state_home)
                .await
                .unwrap()
                .unwrap(),
            pending
        );
        let guard =
            RuntimeArtifactMutationGuard::try_acquire(&state_home.join("runtime/artifacts"))
                .expect("rollback releases the mutation guard");
        drop(guard);

        std::fs::remove_dir(staged).unwrap();
        commit_runtime_artifact_activation(&state_home, &pending, Some(&initial.artifact_digest))
            .await
            .expect("the next actor commit succeeds after the conflict is removed");
    }
}
