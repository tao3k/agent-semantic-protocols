//! Millisecond artifact publication and typed Runtime activation receipts.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::blake3_content_digest::Blake3ContentDigest;

use crate::runtime_artifact_catalog::{
    RuntimeArtifactSlotAuthority, discard_prepared_runtime_artifact,
    prepare_runtime_artifact_candidate, publish_resident_runtime_alias,
    runtime_artifact_candidate_digest,
};
use crate::runtime_artifact_quiescence::prepare_runtime_artifact_quiescence_lease;
use crate::runtime_artifact_retention::RuntimeArtifactMutationGuard;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeArtifactActivationEvent {
    pub artifact_digest: Blake3ContentDigest,
    pub artifact_path: PathBuf,
    pub previous_artifact_digest: Option<Blake3ContentDigest>,
    pub artifact_mode: String,
    pub published_at_unix_millis: u128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeArtifactPublicationReceipt {
    pub path: PathBuf,
    pub status: &'static str,
    pub artifact_digest: Blake3ContentDigest,
    pub activation_event_path: PathBuf,
    pub lock_elapsed_micros: u128,
    pub lock_acquisition_count: u8,
    pub quiescence_operation: String,
    pub quiescence_lease_nonce: String,
    pub lease_producer_process_id: u32,
    pub lease_consumer_process_id: u32,
}

pub fn runtime_artifact_activation_event_path(state_home: &Path) -> PathBuf {
    state_home.join("runtime/resident/active/activation.json")
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
            let event: RuntimeArtifactActivationEvent = serde_json::from_slice(&bytes)
                .map_err(|error| format!("decode Runtime artifact activation event: {error}"))?;
            let applied = state_home.join("runtime/activation/applied.json");
            if let Ok(applied_bytes) = tokio::fs::read(applied).await {
                let applied_event: RuntimeArtifactActivationEvent =
                    serde_json::from_slice(&applied_bytes).map_err(|error| {
                        format!("decode applied Runtime artifact activation event: {error}")
                    })?;
                if applied_event.artifact_digest == event.artifact_digest {
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
    previous_artifact_digest: Option<&Blake3ContentDigest>,
) -> Result<RuntimeArtifactPublicationReceipt, String> {
    let artifact_root = state_home.join("runtime/artifacts");
    let resident_root = state_home.join("runtime/resident");
    let digest = runtime_artifact_candidate_digest(source).await?;
    let token = digest.content_digest().as_str().to_owned();
    let candidate_dir = resident_root.join("candidates").join(&token);
    let slots = RuntimeArtifactSlotAuthority::new(&resident_root);

    // Immutable materialization is deliberately outside the artifact mutation lock.
    let prepared = prepare_runtime_artifact_candidate(state_home, &candidate_dir, source).await?;
    if let Err(error) = slots
        .stage_candidate_artifact(&candidate_dir, &prepared.path)
        .await
    {
        discard_prepared_runtime_artifact(&prepared).await?;
        return Err(error);
    }
    publish_resident_runtime_alias(target, &resident_root).await?;

    let activation_event_path = runtime_artifact_activation_event_path(state_home);
    let event = RuntimeArtifactActivationEvent {
        artifact_digest: prepared.content_digest.clone(),
        artifact_path: prepared.path.clone(),
        previous_artifact_digest: previous_artifact_digest.cloned(),
        artifact_mode: artifact_mode.to_owned(),
        published_at_unix_millis: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("Runtime artifact publication clock failed: {error}"))?
            .as_millis(),
    };
    let event_bytes = serde_json::to_vec_pretty(&event)
        .map_err(|error| format!("encode Runtime artifact activation event: {error}"))?;
    let candidate_event_path = candidate_dir.join("activation.json");
    tokio::fs::write(&candidate_event_path, &event_bytes)
        .await
        .map_err(|error| format!("stage Runtime artifact activation event: {error}"))?;

    let binary_name = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "Runtime artifact target has no binary name".to_owned())?;
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

    // The lock protects only the two-slot swap and its durable activation receipt.
    let lock_started = std::time::Instant::now();
    let guard = RuntimeArtifactMutationGuard::try_acquire(&artifact_root)?;
    let consumed_lease = match quiescence.consume_under_artifact_guard() {
        Ok(consumed) => consumed,
        Err(error) => {
            drop(guard);
            discard_prepared_runtime_artifact(&prepared).await?;
            return Err(error);
        }
    };
    let commit = slots.commit_ready(&candidate_dir).await;
    drop(guard);
    let lock_elapsed_micros = lock_started.elapsed().as_micros();

    if let Err(error) = commit {
        quiescence.restore_after_failed_commit(&consumed_lease)?;
        discard_prepared_runtime_artifact(&prepared).await?;
        return Err(error);
    }
    quiescence.finish_consumption(&consumed_lease)?;

    notify_runtime_artifact_activation(state_home, &event_bytes).await?;

    Ok(RuntimeArtifactPublicationReceipt {
        path: target.to_path_buf(),
        status: "published-activation-pending",
        artifact_digest: prepared.content_digest,
        activation_event_path,
        lock_elapsed_micros,
        lock_acquisition_count: 1,
        quiescence_operation,
        quiescence_lease_nonce: quiescence.lease.lease_nonce,
        lease_producer_process_id: quiescence.lease.producer_process_id,
        lease_consumer_process_id: std::process::id(),
    })
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

        let receipt = publish_runtime_artifact(&state_home, &source, &target, "dev", None)
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

        let initial = publish_runtime_artifact(&state_home, &initial_source, &target, "dev", None)
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

        let error = publish_runtime_artifact(
            &state_home,
            &next_source,
            &target,
            "dev",
            Some(&initial.artifact_digest),
        )
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
}
