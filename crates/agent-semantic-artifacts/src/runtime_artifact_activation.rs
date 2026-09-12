// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Runtime artifact activation event validation, commit, acknowledgement, and rollback.

use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

use crate::blake3_content_digest::Blake3ContentDigest;
use crate::runtime_artifact_retention::RuntimeArtifactMutationGuard;
use crate::runtime_artifact_slots::RuntimeArtifactSlotAuthority;
use crate::runtime_artifact_slots::runtime_artifact_candidate_bundle_digest;
use crate::runtime_artifact_slots::runtime_artifact_candidate_digest;

#[path = "runtime_artifact_pending_receipt.rs"]
mod pending_receipt;

pub(crate) use pending_receipt::commit_staged_pending_runtime_artifact_activation;
pub(crate) use pending_receipt::publish_pending_runtime_artifact_activation;
pub(crate) use pending_receipt::stage_pending_runtime_artifact_activation;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeArtifactCandidateIdentityReceipt {
    pub artifact_digest: Blake3ContentDigest,
    pub artifact_path: PathBuf,
    pub stable_path: PathBuf,
    pub artifact_mode: String,
    pub publication_nonce: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeArtifactActivationEvent {
    pub schema_id: String,
    pub schema_version: u64,
    /// Monotonic transaction sequence used only for publication fencing and
    /// ordering. Runtime product identity remains content-addressed.
    pub activation_generation: u64,
    pub bundle_digest: Blake3ContentDigest,
    pub artifact_digest: Blake3ContentDigest,
    pub artifact_path: PathBuf,
    pub candidate_slot_path: PathBuf,
    pub previous_artifact_digest: Option<Blake3ContentDigest>,
    pub artifact_mode: String,
    pub published_at_unix_millis: u128,
    pub publication_nonce: String,
    pub candidate_identity: RuntimeArtifactCandidateIdentityReceipt,
}

impl RuntimeArtifactActivationEvent {
    /// Validate the receipt identity without reopening mutable artifact paths.
    pub fn validate_identity(&self) -> Result<(), String> {
        validate_runtime_artifact_activation_event(self, "Runtime artifact activation event")
    }

    /// Canonical content identity of the complete activation receipt.
    /// `activationGeneration` remains an observed fence inside the receipt; it
    /// is never accepted as a replacement for this digest.
    #[must_use]
    pub fn content_digest(&self) -> Blake3ContentDigest {
        Blake3ContentDigest::from_bytes(
            &serde_json::to_vec(self).expect("Runtime activation event is serializable"),
        )
    }
}

pub fn runtime_artifact_activation_event_path(state_home: &Path) -> PathBuf {
    crate::RuntimeArtifactStateLayout::new(state_home).pending_activation()
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
                && runtime_artifact_activation_is_applied(&event, &applied_event)
            {
                return Ok(None);
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
    pending.bundle_digest == applied.bundle_digest
        && pending.artifact_digest == applied.artifact_digest
        && pending.publication_nonce == applied.publication_nonce
}

pub async fn read_applied_runtime_artifact_activation_event(
    state_home: &Path,
) -> Result<Option<RuntimeArtifactActivationEvent>, String> {
    let path = crate::RuntimeArtifactStateLayout::new(state_home).applied_activation();
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

/// Allocate the next monotonic activation transaction sequence while the
/// caller holds the Runtime artifact publication guard. The sequence is a
/// stale-actor fence only; content equality is always digest based.
pub async fn next_runtime_artifact_activation_generation_under_guard(
    state_home: &Path,
) -> Result<u64, String> {
    let pending = read_runtime_artifact_activation_event(state_home).await?;
    let applied = read_applied_runtime_artifact_activation_event(state_home).await?;
    pending
        .iter()
        .chain(applied.iter())
        .map(|event| event.activation_generation)
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| "Runtime artifact activation generation overflow".to_owned())
}

pub(crate) fn decode_runtime_artifact_activation_event(
    bytes: &[u8],
    context: &str,
) -> Result<RuntimeArtifactActivationEvent, String> {
    let event: RuntimeArtifactActivationEvent =
        serde_json::from_slice(bytes).map_err(|error| format!("decode {context}: {error}"))?;
    validate_runtime_artifact_activation_event(&event, context)?;
    Ok(event)
}

pub(crate) async fn validate_current_activation_receipts_content(
    state_home: &Path,
) -> Result<(), String> {
    for (is_pending, current) in read_current_activation_receipts(state_home)? {
        if is_pending && is_retired_rollback_pending(state_home, &current)? {
            continue;
        }
        validate_current_activation_content(&current).await?;
    }
    Ok(())
}

pub(crate) async fn validate_current_activation_receipts_for_preverified_predecessor(
    state_home: &Path,
    predecessor: &Path,
    bundle_digest: &Blake3ContentDigest,
) -> Result<(), String> {
    let predecessor = std::fs::canonicalize(predecessor).map_err(|error| {
        format!(
            "state=runtime-artifact-publication-failed reasonKind=predecessor-unavailable path={} error={error}",
            predecessor.display()
        )
    })?;
    for (is_pending, current) in read_current_activation_receipts(state_home)? {
        if is_pending && is_retired_rollback_pending(state_home, &current)? {
            continue;
        }
        let candidate = std::fs::canonicalize(&current.candidate_slot_path).map_err(|error| {
            format!(
                "state=runtime-artifact-publication-failed reasonKind=activation-candidate-invalid path={} error={error}",
                current.candidate_slot_path.display()
            )
        })?;
        if candidate != predecessor || &current.bundle_digest != bundle_digest {
            return Err(
                "state=runtime-artifact-publication-failed reasonKind=activation-predecessor-binding-drift"
                    .to_owned(),
            );
        }
        validate_current_activation_member_content(&current).await?;
    }
    Ok(())
}

fn read_current_activation_receipts(
    state_home: &Path,
) -> Result<Vec<(bool, RuntimeArtifactActivationEvent)>, String> {
    let mut receipts = Vec::new();
    for (is_pending, path) in [
        (true, runtime_artifact_activation_event_path(state_home)),
        (
            false,
            crate::RuntimeArtifactStateLayout::new(state_home).applied_activation(),
        ),
    ] {
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(format!(
                    "state=runtime-artifact-publication-failed reasonKind=activation-content-validation-read-failed path={} error={error}",
                    path.display()
                ));
            }
        };
        let current = decode_runtime_artifact_activation_event(
            &bytes,
            "current Runtime artifact activation receipt",
        )
        .map_err(|error| {
            format!(
                "state=runtime-artifact-publication-failed reasonKind=activation-receipt-invalid path={} error={error}",
                path.display()
            )
        })?;
        receipts.push((is_pending, current));
    }
    Ok(receipts)
}

fn is_retired_rollback_pending(
    state_home: &Path,
    pending: &RuntimeArtifactActivationEvent,
) -> Result<bool, String> {
    if pending.candidate_slot_path.exists() {
        return Ok(false);
    }
    let Some(applied) = read_applied_runtime_artifact_activation_event_blocking(state_home)? else {
        return Ok(false);
    };
    if pending.previous_artifact_digest.as_ref() != Some(&applied.artifact_digest) {
        return Ok(false);
    }
    let layout = crate::RuntimeArtifactStateLayout::new(state_home);
    let applied_candidate = std::fs::canonicalize(&applied.candidate_slot_path).map_err(|error| {
        format!(
            "state=runtime-artifact-publication-failed reasonKind=applied-candidate-invalid path={} error={error}",
            applied.candidate_slot_path.display()
        )
    })?;
    let active = std::fs::canonicalize(layout.active_slot()).map_err(|error| {
        format!(
            "state=runtime-artifact-publication-failed reasonKind=active-slot-invalid error={error}"
        )
    })?;
    let healthy = std::fs::canonicalize(layout.healthy_slot()).map_err(|error| {
        format!(
            "state=runtime-artifact-publication-failed reasonKind=healthy-slot-invalid error={error}"
        )
    })?;
    Ok(active == applied_candidate && healthy == applied_candidate)
}

fn read_applied_runtime_artifact_activation_event_blocking(
    state_home: &Path,
) -> Result<Option<RuntimeArtifactActivationEvent>, String> {
    let path = crate::RuntimeArtifactStateLayout::new(state_home).applied_activation();
    match std::fs::read(&path) {
        Ok(bytes) => decode_runtime_artifact_activation_event(
            &bytes,
            "applied Runtime artifact activation receipt",
        )
        .map(Some),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!(
            "state=runtime-artifact-publication-failed reasonKind=applied-receipt-unreadable path={} error={error}",
            path.display()
        )),
    }
}

async fn validate_current_activation_content(
    event: &RuntimeArtifactActivationEvent,
) -> Result<(), String> {
    let actual_bundle =
        runtime_artifact_candidate_bundle_digest(&event.candidate_slot_path).await?;
    if actual_bundle != event.bundle_digest {
        return Err(format!(
            "state=runtime-artifact-publication-failed reasonKind=activation-receipt-content-drift expectedBundleDigest={} actualBundleDigest={actual_bundle}",
            event.bundle_digest
        ));
    }
    validate_current_activation_member_content(event).await
}

async fn validate_current_activation_member_content(
    event: &RuntimeArtifactActivationEvent,
) -> Result<(), String> {
    let artifact_kind = event
        .candidate_identity
        .stable_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            "state=runtime-artifact-publication-failed reasonKind=activation-receipt-identity-invalid"
                .to_owned()
        })?;
    let member = event.candidate_slot_path.join(artifact_kind);
    let member_target = std::fs::canonicalize(&member).map_err(|error| {
        format!(
            "state=runtime-artifact-publication-failed reasonKind=activation-candidate-member-invalid path={} error={error}",
            member.display()
        )
    })?;
    let actual_digest = runtime_artifact_candidate_digest(&member_target).await?;
    let recorded_artifact = std::fs::canonicalize(&event.artifact_path).map_err(|error| {
        format!(
            "state=runtime-artifact-publication-failed reasonKind=activation-artifact-invalid path={} error={error}",
            event.artifact_path.display()
        )
    })?;
    if member_target != recorded_artifact || actual_digest != event.artifact_digest {
        return Err(format!(
            "state=runtime-artifact-publication-failed reasonKind=activation-receipt-content-drift expectedArtifactDigest={} actualArtifactDigest={actual_digest}",
            event.artifact_digest
        ));
    }
    Ok(())
}

fn validate_runtime_artifact_activation_event(
    event: &RuntimeArtifactActivationEvent,
    context: &str,
) -> Result<(), String> {
    let identity = &event.candidate_identity;
    if event.schema_id != "agent.semantic-protocols.runtime-artifact-activation"
        || event.schema_version != 1
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RuntimeArtifactServingSnapshot {
    pub healthy_target: Option<PathBuf>,
    pub artifact_target: Option<PathBuf>,
    pub artifact_digest: Option<Blake3ContentDigest>,
}

pub(crate) async fn prepare_runtime_artifact_serving_snapshot(
    slots: &RuntimeArtifactSlotAuthority,
    artifact_kind: &str,
) -> Result<RuntimeArtifactServingSnapshot, String> {
    let healthy_path = slots.healthy_path();
    let healthy_exists = match std::fs::symlink_metadata(&healthy_path) {
        Ok(_) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => {
            return Err(format!(
                "state=runtime-artifact-publication-failed reasonKind=healthy-slot-unreadable path={} error={error}",
                healthy_path.display()
            ));
        }
    };
    match healthy_exists {
        false => Ok(RuntimeArtifactServingSnapshot {
            healthy_target: None,
            artifact_target: None,
            artifact_digest: None,
        }),
        true => {
            let healthy = slots.healthy_target().await.map_err(|error| {
                format!(
                    "state=runtime-artifact-publication-failed reasonKind=healthy-slot-dangling error={error}"
                )
            })?;
            let healthy = healthy.ok_or_else(|| {
                "state=runtime-artifact-publication-failed reasonKind=healthy-slot-dangling"
                    .to_owned()
            })?;
            let receipt_path = healthy.join("activation.json");
            let receipt_bytes = std::fs::read(&receipt_path).map_err(|error| {
                format!(
                    "state=runtime-artifact-publication-failed reasonKind=active-receipt-unreadable path={} error={error}",
                    receipt_path.display()
                )
            })?;
            let expected = healthy.join(artifact_kind);
            let candidate_target = std::fs::canonicalize(&expected).map_err(|error| {
                format!(
                    "state=runtime-artifact-publication-failed reasonKind=applied-candidate-slot-dangling path={} error={error}",
                    expected.display()
                )
            })?;
            let applied = decode_runtime_artifact_activation_event(
                &receipt_bytes,
                "active Runtime artifact activation receipt",
            )?;
            if healthy != applied.candidate_slot_path {
                return Err(format!(
                    "state=runtime-artifact-publication-failed reasonKind=healthy-applied-identity-mismatch healthy={} applied={}",
                    healthy.display(),
                    applied.candidate_slot_path.display()
                ));
            }
            let applied_artifact = std::fs::canonicalize(&applied.artifact_path).map_err(|error| {
                format!(
                    "state=runtime-artifact-publication-failed reasonKind=applied-artifact-invalid path={} error={error}",
                    applied.artifact_path.display()
                )
            })?;
            if candidate_target != applied_artifact {
                return Err(format!(
                    "state=runtime-artifact-publication-failed reasonKind=applied-artifact-identity-mismatch candidate={} applied={}",
                    candidate_target.display(),
                    applied.artifact_path.display()
                ));
            }
            Ok(RuntimeArtifactServingSnapshot {
                healthy_target: Some(healthy),
                artifact_target: Some(candidate_target),
                artifact_digest: Some(applied.artifact_digest),
            })
        }
    }
}

pub(crate) async fn previous_serving_digest_under_guard(
    slots: &RuntimeArtifactSlotAuthority,
    artifact_kind: &str,
) -> Result<Option<Blake3ContentDigest>, String> {
    Ok(
        prepare_runtime_artifact_serving_snapshot(slots, artifact_kind)
            .await?
            .artifact_digest,
    )
}

pub(crate) async fn prepare_active_slot_snapshot(
    slots: &RuntimeArtifactSlotAuthority,
    artifact_kind: &str,
) -> Result<Option<PathBuf>, String> {
    let slots = slots.clone();
    let artifact_kind = artifact_kind.to_owned();
    tokio::task::spawn_blocking(move || {
        prepare_active_slot_snapshot_blocking(&slots, &artifact_kind)
    })
    .await
    .map_err(|error| format!("prepare active Runtime slot snapshot task failed: {error}"))?
}

fn prepare_active_slot_snapshot_blocking(
    slots: &RuntimeArtifactSlotAuthority,
    artifact_kind: &str,
) -> Result<Option<PathBuf>, String> {
    let active_path = slots.active_path();
    let active = match std::fs::read_link(&active_path) {
        Ok(target) => target,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "state=runtime-artifact-publication-failed reasonKind=active-slot-unreadable path={} error={error}",
                active_path.display()
            ));
        }
    };
    std::fs::symlink_metadata(&active).map_err(|error| {
        format!(
            "state=runtime-artifact-publication-failed reasonKind=active-slot-dangling path={} error={error}",
            active.display()
        )
    })?;
    let member_path = active.join(artifact_kind);
    let member_target = std::fs::canonicalize(&member_path).map_err(|error| {
        format!(
            "state=runtime-artifact-publication-failed reasonKind=active-member-dangling path={} error={error}",
            member_path.display()
        )
    })?;
    std::fs::symlink_metadata(&member_target).map_err(|error| {
        format!(
            "state=runtime-artifact-publication-failed reasonKind=active-member-dangling path={} error={error}",
            member_target.display()
        )
    })?;
    Ok(Some(active))
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeArtifactActivationCommitReceipt {
    pub bundle_digest: Blake3ContentDigest,
    pub artifact_digest: Blake3ContentDigest,
    pub previous_serving_digest: Option<Blake3ContentDigest>,
    pub state: String,
}

pub async fn commit_runtime_artifact_activation(
    state_home: &Path,
    event: &RuntimeArtifactActivationEvent,
    serving_digest: Option<&Blake3ContentDigest>,
) -> Result<RuntimeArtifactActivationCommitReceipt, String> {
    validate_runtime_artifact_activation_event(event, "Runtime artifact activation event")?;
    let layout = crate::RuntimeArtifactStateLayout::new(state_home);
    let artifact_root = layout.root().to_path_buf();
    let artifact_kind = event
        .candidate_identity
        .stable_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "Runtime activation target has no artifact kind".to_owned())?;
    let slots = RuntimeArtifactSlotAuthority::for_artifact(layout.root(), artifact_kind);
    // Reject a superseded actor before touching its immutable candidate. A
    // newer publication is allowed to remove that unreachable candidate, so
    // filesystem availability cannot be the authority for stale admission.
    validate_current_pending_activation_identity(state_home, event)?;
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
    let candidate_bundle_digest =
        runtime_artifact_candidate_bundle_digest(&event.candidate_slot_path).await?;
    if candidate_bundle_digest != event.bundle_digest {
        return Err(format!(
            "state=runtime-artifact-activation-failed reasonKind=candidate-bundle-digest-mismatch expected={} observed={candidate_bundle_digest}",
            event.bundle_digest
        ));
    }
    let candidate_artifact_digest = runtime_artifact_candidate_digest(&candidate_artifact).await?;
    if candidate_artifact_digest != event.artifact_digest {
        return Err(format!(
            "state=runtime-artifact-activation-failed reasonKind=candidate-slot-digest-mismatch expected={} observed={candidate_artifact_digest}",
            event.artifact_digest
        ));
    }
    let guard = RuntimeArtifactMutationGuard::try_acquire(&artifact_root)?;
    let derived_serving_digest = previous_serving_digest_under_guard(&slots, artifact_kind).await?;
    // A Runtime candidate proves Healthy before the activation actor commits
    // its pending event.  At that point the healthy slot may already name the
    // candidate; that observation is candidate readiness, not the predecessor
    // identity.  The publication event remains the immutable predecessor
    // authority for this case.
    let observed_predecessor = if derived_serving_digest.as_ref() == Some(&event.artifact_digest) {
        event.previous_artifact_digest.clone()
    } else {
        derived_serving_digest.clone()
    };
    if event.previous_artifact_digest.as_ref() != observed_predecessor.as_ref() {
        return Err(format!(
            "state=runtime-artifact-activation-failed reasonKind=serving-artifact-identity-mismatch expected={:?} observed={:?}",
            event.previous_artifact_digest, observed_predecessor
        ));
    }
    if let Some(caller_digest) = serving_digest
        && Some(caller_digest) != observed_predecessor.as_ref()
    {
        return Err(format!(
            "state=runtime-artifact-activation-failed reasonKind=serving-artifact-caller-drift expected={:?} observed={caller_digest}",
            observed_predecessor
        ));
    }
    // Revalidate after acquiring the mutation guard. The first check keeps a
    // removed stale candidate from turning into a path error; this second
    // check is the linearizable CAS fence for a current actor.
    validate_current_pending_activation_identity(state_home, event)?;
    let active_before = slots.active_target().await?;
    let healthy_before = slots.healthy_target().await?;
    let applied_path = layout.applied_activation();
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
        slots
            .mark_active_healthy(&event.candidate_slot_path)
            .await?;
        publish_applied_runtime_artifact_activation(state_home, event)
    }
    .await;
    if let Err(error) = commit {
        let slot_restore = slots
            .restore_targets(active_before.as_deref(), healthy_before.as_deref())
            .await;
        let applied_restore = restore_applied_runtime_artifact_activation(
            &applied_path,
            applied_before.as_deref(),
            &event.publication_nonce,
        );
        if let Err(restore_error) = slot_restore.and(applied_restore) {
            return Err(format!(
                "{error}; state=runtime-artifact-activation-failed reasonKind=rollback-failed error={restore_error}"
            ));
        }
        return Err(error);
    }
    drop(guard);
    crate::runtime_artifact_retention::prune_unreachable_runtime_artifacts(&artifact_root)
        .await
        .map_err(|error| {
            format!(
                "state=runtime-artifact-activation-committed reasonKind=runtime-state-retention-finalization-failed error={error}"
            )
        })?;

    Ok(RuntimeArtifactActivationCommitReceipt {
        bundle_digest: event.bundle_digest.clone(),
        artifact_digest: event.artifact_digest.clone(),
        previous_serving_digest: observed_predecessor,
        state: "applied".to_owned(),
    })
}

fn validate_current_pending_activation_identity(
    state_home: &Path,
    event: &RuntimeArtifactActivationEvent,
) -> Result<(), String> {
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
    if current_pending.artifact_digest != event.artifact_digest
        || current_pending.bundle_digest != event.bundle_digest
        || current_pending.publication_nonce != event.publication_nonce
    {
        return Err(format!(
            "state=runtime-artifact-activation-failed reasonKind=stale-active-bundle attempted={} current={}",
            event.artifact_digest, current_pending.artifact_digest
        ));
    }
    Ok(())
}

/// Restore the previously healthy bundle when Runtime cannot activate the
/// currently pending candidate. The event identity and current active slot are
/// both checked under the sole artifact mutation guard, so a stale actor cannot
/// roll back a newer installation.
pub async fn rollback_runtime_artifact_activation(
    state_home: &Path,
    event: &RuntimeArtifactActivationEvent,
) -> Result<(), String> {
    validate_runtime_artifact_activation_event(event, "Runtime artifact rollback event")?;
    let layout = crate::RuntimeArtifactStateLayout::new(state_home);
    let artifact_root = layout.root().to_path_buf();
    let artifact_kind = event
        .candidate_identity
        .stable_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "Runtime rollback target has no artifact kind".to_owned())?;
    let slots = RuntimeArtifactSlotAuthority::for_artifact(layout.root(), artifact_kind);
    let guard = RuntimeArtifactMutationGuard::try_acquire(&artifact_root)?;
    let current = read_runtime_artifact_activation_event(state_home)
        .await?
        .ok_or_else(|| "Runtime rollback has no pending activation event".to_owned())?;
    if current.artifact_digest != event.artifact_digest
        || current.bundle_digest != event.bundle_digest
        || current.publication_nonce != event.publication_nonce
    {
        return Err(format!(
            "state=runtime-artifact-activation-failed reasonKind=stale-active-bundle-rollback attempted={} current={}",
            event.artifact_digest, current.artifact_digest
        ));
    }
    slots
        .restore_active_from_healthy(&event.candidate_slot_path)
        .await?;
    drop(guard);
    crate::runtime_artifact_retention::prune_unreachable_runtime_artifacts(&artifact_root)
        .await
        .map(|_| ())
        .map_err(|error| {
            format!(
                "state=runtime-artifact-rollback-committed reasonKind=runtime-state-retention-finalization-failed error={error}"
            )
        })
}

pub(crate) fn read_optional_symlink(path: &Path) -> Result<Option<PathBuf>, String> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "inspect Runtime artifact launcher {}: {error}",
                path.display()
            ));
        }
    };
    if metadata.file_type().is_symlink() {
        return std::fs::read_link(path).map(Some).map_err(|error| {
            format!("read Runtime artifact launcher {}: {error}", path.display())
        });
    }
    if metadata.is_file() {
        // Pre-slot installations copied launchers directly into runtime/bin.
        // They are not rollback authority: a failed new publication removes
        // its staged alias instead of resurrecting this retired second source.
        return Ok(None);
    }
    Err(format!(
        "state=runtime-artifact-publication-failed reasonKind=runtime-launcher-type-conflict path={}",
        path.display()
    ))
}

pub(crate) fn restore_runtime_artifact_symlink(
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

pub(crate) fn publish_applied_runtime_artifact_activation(
    state_home: &Path,
    event: &RuntimeArtifactActivationEvent,
) -> Result<(), String> {
    let applied = crate::RuntimeArtifactStateLayout::new(state_home).applied_activation();
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
        Err(error) => Err(format!("remove Runtime activation pending event: {error}")),
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
    let applied = crate::RuntimeArtifactStateLayout::new(state_home).applied_activation();
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

pub(crate) fn restore_pending_runtime_artifact_activation(
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
