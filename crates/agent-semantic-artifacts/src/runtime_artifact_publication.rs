//! Millisecond immutable bundle publication and launcher switching.

use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use serde::Deserialize;
use serde::Serialize;

use crate::blake3_content_digest::Blake3ContentDigest;
use crate::runtime_artifact_activation::RuntimeArtifactActivationEvent;
use crate::runtime_artifact_activation::RuntimeArtifactCandidateIdentityReceipt;
use crate::runtime_artifact_activation::commit_staged_pending_runtime_artifact_activation;
use crate::runtime_artifact_activation::decode_runtime_artifact_activation_event;
use crate::runtime_artifact_activation::prepare_active_slot_snapshot;
use crate::runtime_artifact_activation::prepare_runtime_artifact_serving_snapshot;
use crate::runtime_artifact_activation::read_optional_symlink;
use crate::runtime_artifact_activation::restore_pending_runtime_artifact_activation;
use crate::runtime_artifact_activation::restore_runtime_artifact_symlink;
use crate::runtime_artifact_activation::runtime_artifact_activation_event_path;
use crate::runtime_artifact_activation::stage_pending_runtime_artifact_activation;
use crate::runtime_artifact_activation::validate_current_activation_receipts_content;
use crate::runtime_artifact_quiescence::prepare_runtime_artifact_quiescence_lease;
use crate::runtime_artifact_retention::RuntimeArtifactCandidatePreparationLease;
use crate::runtime_artifact_retention::RuntimeArtifactMutationGuard;
use crate::runtime_artifact_retention::prune_unreachable_runtime_artifacts;
use crate::runtime_artifact_slots::PreparedRuntimeArtifact;
use crate::runtime_artifact_slots::RuntimeArtifactBundleBinding;
use crate::runtime_artifact_slots::RuntimeArtifactSlotAuthority;
use crate::runtime_artifact_slots::discard_prepared_runtime_artifact;
use crate::runtime_artifact_slots::prepare_runtime_artifact_candidate_for_kind;
use crate::runtime_artifact_slots::runtime_artifact_bound_bundle_digest;
use crate::runtime_artifact_slots::runtime_artifact_bundle_digest;
use crate::runtime_artifact_slots::runtime_artifact_candidate_digest;
use crate::runtime_artifact_slots::stage_runtime_artifact_bound_bundle_manifest;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeArtifactBundleMemberSource<'a> {
    pub name: &'a str,
    pub source: &'a Path,
}

#[derive(Debug)]
struct PreparedRuntimeArtifactBundleMember {
    name: String,
    artifact: PreparedRuntimeArtifact,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeArtifactPublicationReceipt {
    pub path: PathBuf,
    pub status: &'static str,
    pub artifact_digest: Blake3ContentDigest,
    pub bundle_digest: Blake3ContentDigest,
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
    pub authority_read_micros: u128,
    pub candidate_activation_staging_micros: u128,
    pub pending_publication_micros: u128,
    pub client_launcher_switch_micros: u128,
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
        &[],
        None,
        || async {},
    )
    .await
}

/// Publish the Runtime client and Hook evaluator as one immutable activation
/// candidate. The active/healthy directory slot is the only serving selector.
pub async fn publish_runtime_artifact_bundle(
    state_home: &Path,
    source: &Path,
    target: &Path,
    artifact_mode: &str,
    hook_source: &Path,
) -> Result<RuntimeArtifactPublicationReceipt, String> {
    let members = [RuntimeArtifactBundleMemberSource {
        name: "asp-hook",
        source: hook_source,
    }];
    publish_runtime_artifact_bundle_members(state_home, source, target, artifact_mode, &members)
        .await
}

/// Publish the Runtime client and a declarative set of optional executable
/// members as one immutable active/healthy bundle. Member names are the only
/// lookup keys; callers cannot publish a second descriptor or stable selector.
pub async fn publish_runtime_artifact_bundle_members(
    state_home: &Path,
    source: &Path,
    target: &Path,
    artifact_mode: &str,
    member_sources: &[RuntimeArtifactBundleMemberSource<'_>],
) -> Result<RuntimeArtifactPublicationReceipt, String> {
    publish_runtime_artifact_with_before_guard(
        state_home,
        source,
        target,
        artifact_mode,
        member_sources,
        None,
        || async {},
    )
    .await
}

/// Publish a Runtime bundle whose complete execution closure is part of the
/// immutable product identity. New serving callers must use this entry point;
/// mutable State Home catalog files are not binding inputs.
pub async fn publish_runtime_artifact_bound_bundle_members(
    state_home: &Path,
    source: &Path,
    target: &Path,
    artifact_mode: &str,
    member_sources: &[RuntimeArtifactBundleMemberSource<'_>],
    execution_binding: &RuntimeArtifactBundleBinding,
) -> Result<RuntimeArtifactPublicationReceipt, String> {
    publish_runtime_artifact_with_before_guard(
        state_home,
        source,
        target,
        artifact_mode,
        member_sources,
        Some(execution_binding),
        || async {},
    )
    .await
}

async fn publish_runtime_artifact_with_before_guard<BeforeGuard, BeforeGuardFuture>(
    state_home: &Path,
    source: &Path,
    target: &Path,
    artifact_mode: &str,
    member_sources: &[RuntimeArtifactBundleMemberSource<'_>],
    execution_binding: Option<&RuntimeArtifactBundleBinding>,
    before_guard: BeforeGuard,
) -> Result<RuntimeArtifactPublicationReceipt, String>
where
    BeforeGuard: FnOnce() -> BeforeGuardFuture,
    BeforeGuardFuture: std::future::Future<Output = ()>,
{
    let layout = crate::RuntimeArtifactStateLayout::new(state_home);
    let artifact_root = layout.root().to_path_buf();
    let digest = runtime_artifact_candidate_digest(source).await?;
    let binary_name = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "Runtime artifact target has no binary name".to_owned())?;
    let mut members = std::collections::BTreeMap::from([(binary_name.to_owned(), digest.clone())]);
    for member in member_sources {
        let member_path = Path::new(member.name);
        if member_path.components().count() != 1
            || member.name == "."
            || member.name == ".."
            || member.name == binary_name
            || members.contains_key(member.name)
        {
            return Err(format!(
                "invalid or duplicate Runtime artifact bundle member `{}`",
                member.name
            ));
        }
        members.insert(
            member.name.to_owned(),
            runtime_artifact_candidate_digest(member.source).await?,
        );
    }
    let bundle_digest = execution_binding.map_or_else(
        || runtime_artifact_bundle_digest(&members),
        |binding| runtime_artifact_bound_bundle_digest(&members, binding),
    );
    let token = bundle_digest.content_digest().as_str();
    let candidate_dir = layout.bundle_store().join(binary_name).join(&token);
    let _candidate_preparation_lease =
        RuntimeArtifactCandidatePreparationLease::acquire(state_home, binary_name, &token)?;
    let slots = RuntimeArtifactSlotAuthority::for_artifact(layout.root(), binary_name);

    // Immutable materialization is deliberately outside the artifact mutation lock.
    let prepared = prepare_runtime_artifact_candidate_for_kind(
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
    let mut prepared_members = Vec::with_capacity(member_sources.len());
    for member in member_sources {
        let member_prepared = prepare_runtime_artifact_candidate_for_kind(
            state_home,
            &candidate_dir,
            member.source,
            member.name,
        )
        .await?;
        if let Err(error) = slots
            .stage_candidate_member(&candidate_dir, member.name, &member_prepared.path)
            .await
        {
            discard_prepared_runtime_artifact(&member_prepared).await?;
            discard_prepared_runtime_artifact_bundle_members(&prepared_members).await?;
            discard_prepared_runtime_artifact(&prepared).await?;
            return Err(error);
        }
        prepared_members.push(PreparedRuntimeArtifactBundleMember {
            name: member.name.to_owned(),
            artifact: member_prepared,
        });
    }
    if let Some(binding) = execution_binding {
        let staged_digest =
            stage_runtime_artifact_bound_bundle_manifest(&candidate_dir, &members, binding)?;
        if staged_digest != bundle_digest {
            return Err("Runtime bound bundle staging digest drift".to_owned());
        }
    } else {
        let bundle_identity = serde_json::json!({
            "schemaId": "agent.semantic-protocols.runtime-binary-bundle",
            "schemaVersion": 1,
            "bundleDigest": bundle_digest.clone(),
            "members": members,
        });
        std::fs::write(
            candidate_dir.join("bundle.json"),
            serde_json::to_vec_pretty(&bundle_identity)
                .map_err(|error| format!("encode Runtime binary bundle identity: {error}"))?,
        )
        .map_err(|error| format!("stage Runtime binary bundle identity: {error}"))?;
    }
    if let Err(error) = slots.validate_candidate(&candidate_dir).await {
        discard_prepared_runtime_artifact(&prepared).await?;
        discard_prepared_runtime_artifact_bundle_members(&prepared_members).await?;
        return Err(error);
    }
    let activation_event_path = runtime_artifact_activation_event_path(state_home);

    let quiescence_operation = format!("publish:{binary_name}");
    validate_current_activation_receipts_content(state_home).await?;
    before_guard().await;

    let mut phase_trace = RuntimeArtifactPublicationPhaseTrace::default();
    let phase_started = std::time::Instant::now();
    let serving_snapshot =
        match prepare_runtime_artifact_serving_snapshot(&slots, binary_name).await {
            Ok(snapshot) => snapshot,
            Err(error) => {
                discard_prepared_runtime_artifact(&prepared).await?;
                discard_prepared_runtime_artifact_bundle_members(&prepared_members).await?;
                return Err(error);
            }
        };
    let active_snapshot = match prepare_active_slot_snapshot(&slots, binary_name).await {
        Ok(snapshot) => snapshot,
        Err(error) => {
            discard_prepared_runtime_artifact(&prepared).await?;
            discard_prepared_runtime_artifact_bundle_members(&prepared_members).await?;
            return Err(error);
        }
    };
    phase_trace.authority_read_micros = phase_started.elapsed().as_micros();

    // Lease recovery, pending publication, stable launchers, and the sole active
    // directory selector are one Artifacts-owned transaction. A dead producer
    // may be recovered only while this canonical mutation guard is held.
    let phase_started = std::time::Instant::now();
    let guard = match RuntimeArtifactMutationGuard::try_acquire(&artifact_root) {
        Ok(guard) => guard,
        Err(error) => {
            discard_prepared_runtime_artifact(&prepared).await?;
            discard_prepared_runtime_artifact_bundle_members(&prepared_members).await?;
            return Err(error);
        }
    };
    phase_trace.guard_acquisition_micros = phase_started.elapsed().as_micros();
    let lock_started = std::time::Instant::now();
    let quiescence = match prepare_runtime_artifact_quiescence_lease(
        state_home,
        &quiescence_operation,
        &prepared.content_digest,
        &guard,
    ) {
        Ok(quiescence) => quiescence,
        Err(error) => {
            drop(guard);
            discard_prepared_runtime_artifact(&prepared).await?;
            discard_prepared_runtime_artifact_bundle_members(&prepared_members).await?;
            return Err(error);
        }
    };
    let publication_nonce = quiescence.lease.lease_nonce.clone();
    let published_at_unix_millis = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_millis(),
        Err(error) => {
            drop(guard);
            discard_prepared_runtime_artifact(&prepared).await?;
            discard_prepared_runtime_artifact_bundle_members(&prepared_members).await?;
            return Err(format!(
                "Runtime artifact publication clock failed: {error}"
            ));
        }
    };
    let event = RuntimeArtifactActivationEvent {
        schema_id: "agent.semantic-protocols.runtime-artifact-activation".to_owned(),
        schema_version: 1,
        activation_generation:
            crate::runtime_artifact_activation::next_runtime_artifact_activation_generation_under_guard(
                state_home,
            )
            .await?,
        bundle_digest: bundle_digest.clone(),
        artifact_digest: prepared.content_digest.clone(),
        artifact_path: prepared.path.clone(),
        candidate_slot_path: candidate_dir.clone(),
        previous_artifact_digest: serving_snapshot.artifact_digest.clone(),
        artifact_mode: artifact_mode.to_owned(),
        published_at_unix_millis,
        publication_nonce: publication_nonce.clone(),
        candidate_identity: RuntimeArtifactCandidateIdentityReceipt {
            artifact_digest: prepared.content_digest.clone(),
            artifact_path: prepared.path.clone(),
            stable_path: target.to_path_buf(),
            artifact_mode: artifact_mode.to_owned(),
            publication_nonce,
        },
    };
    let phase_started = std::time::Instant::now();
    let event_bytes = match serde_json::to_vec_pretty(&event) {
        Ok(bytes) => bytes,
        Err(error) => {
            drop(guard);
            discard_prepared_runtime_artifact(&prepared).await?;
            discard_prepared_runtime_artifact_bundle_members(&prepared_members).await?;
            return Err(format!("encode Runtime artifact activation event: {error}"));
        }
    };
    if let Err(error) = std::fs::write(candidate_dir.join("activation.json"), &event_bytes) {
        drop(guard);
        discard_prepared_runtime_artifact(&prepared).await?;
        discard_prepared_runtime_artifact_bundle_members(&prepared_members).await?;
        return Err(format!("stage Runtime artifact activation event: {error}"));
    }
    let staged_pending = match stage_pending_runtime_artifact_activation(
        &activation_event_path,
        &event_bytes,
        &event.publication_nonce,
    ) {
        Ok(staged) => staged,
        Err(error) => {
            drop(guard);
            discard_prepared_runtime_artifact(&prepared).await?;
            discard_prepared_runtime_artifact_bundle_members(&prepared_members).await?;
            return Err(error);
        }
    };
    phase_trace.candidate_activation_staging_micros = phase_started.elapsed().as_micros();

    let phase_started = std::time::Instant::now();
    let consumed_lease = match quiescence.consume_under_artifact_guard(&guard) {
        Ok(consumed) => consumed,
        Err(error) => {
            drop(guard);
            let _ = std::fs::remove_file(&staged_pending);
            discard_prepared_runtime_artifact(&prepared).await?;
            discard_prepared_runtime_artifact_bundle_members(&prepared_members).await?;
            return Err(error);
        }
    };
    phase_trace.lease_consume_micros = phase_started.elapsed().as_micros();
    let phase_started = std::time::Instant::now();
    let observed_active = match slots.active_target_under_guard() {
        Ok(target) => target,
        Err(error) => {
            let _ = std::fs::remove_file(&staged_pending);
            quiescence.restore_after_failed_commit(&consumed_lease, &guard)?;
            drop(guard);
            discard_prepared_runtime_artifact(&prepared).await?;
            discard_prepared_runtime_artifact_bundle_members(&prepared_members).await?;
            return Err(error);
        }
    };
    let observed_healthy = match slots.healthy_target_under_guard() {
        Ok(target) => target,
        Err(error) => {
            let _ = std::fs::remove_file(&staged_pending);
            quiescence.restore_after_failed_commit(&consumed_lease, &guard)?;
            drop(guard);
            discard_prepared_runtime_artifact(&prepared).await?;
            discard_prepared_runtime_artifact_bundle_members(&prepared_members).await?;
            return Err(error);
        }
    };
    let observed_healthy_artifact = match observed_healthy.as_ref() {
        Some(healthy) => match std::fs::read_link(healthy.join(binary_name)) {
            Ok(target) => Some(target),
            Err(error) => {
                let _ = std::fs::remove_file(&staged_pending);
                quiescence.restore_after_failed_commit(&consumed_lease, &guard)?;
                drop(guard);
                discard_prepared_runtime_artifact(&prepared).await?;
                discard_prepared_runtime_artifact_bundle_members(&prepared_members).await?;
                return Err(format!(
                    "state=runtime-artifact-publication-failed reasonKind=healthy-member-snapshot-unreadable path={} error={error}",
                    healthy.join(binary_name).display()
                ));
            }
        },
        None => None,
    };
    if observed_active != active_snapshot
        || observed_healthy != serving_snapshot.healthy_target
        || observed_healthy_artifact != serving_snapshot.artifact_target
    {
        let _ = std::fs::remove_file(&staged_pending);
        quiescence.restore_after_failed_commit(&consumed_lease, &guard)?;
        drop(guard);
        discard_prepared_runtime_artifact(&prepared).await?;
        discard_prepared_runtime_artifact_bundle_members(&prepared_members).await?;
        return Err(
            "state=runtime-artifact-publication-failed reasonKind=resident-slot-snapshot-drift"
                .to_owned(),
        );
    }
    phase_trace.receipt_read_validate_micros = phase_started.elapsed().as_micros();
    let prior_authority = (|| {
        Ok::<_, String>((
            observed_active,
            observed_healthy,
            read_optional_symlink(target)?,
        ))
    })();
    let (active_before, healthy_before, stable_before) = match prior_authority {
        Ok(authority) => authority,
        Err(error) => {
            let _ = std::fs::remove_file(&staged_pending);
            quiescence.restore_after_failed_commit(&consumed_lease, &guard)?;
            drop(guard);
            discard_prepared_runtime_artifact(&prepared).await?;
            discard_prepared_runtime_artifact_bundle_members(&prepared_members).await?;
            return Err(error);
        }
    };
    let previous_pending_result = match std::fs::read(&activation_event_path) {
        Ok(bytes) => {
            decode_runtime_artifact_activation_event(&bytes, "Runtime artifact activation event")
                .map(|_| Some(bytes))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!(
            "read previous Runtime artifact activation event {}: {error}",
            activation_event_path.display()
        )),
    };
    let previous_pending = match previous_pending_result {
        Ok(previous) => previous,
        Err(error) => {
            let _ = std::fs::remove_file(&staged_pending);
            quiescence.restore_after_failed_commit(&consumed_lease, &guard)?;
            drop(guard);
            discard_prepared_runtime_artifact(&prepared).await?;
            discard_prepared_runtime_artifact_bundle_members(&prepared_members).await?;
            return Err(error);
        }
    };
    let transaction = (|| -> Result<(), String> {
        let phase_started = std::time::Instant::now();
        let commit = commit_staged_pending_runtime_artifact_activation(
            &staged_pending,
            &activation_event_path,
        );
        phase_trace.pending_publication_micros = phase_started.elapsed().as_micros();
        let phase_started = std::time::Instant::now();
        commit?;
        // `active` is the single bundle selector, not a directory. Publish it
        // before installing the stable launcher that resolves through
        // `active/<binary>`. Reversing this order causes launcher preparation
        // to create `active` as a directory and makes the slot CAS fail with
        // EINVAL on its first symlink read.
        slots.publish_active_candidate_under_guard(&candidate_dir)?;
        publish_runtime_bundle_member_launcher(
            target,
            &layout.active_slot().join(binary_name),
            &event.publication_nonce,
            binary_name,
        )?;
        phase_trace.client_launcher_switch_micros = phase_started.elapsed().as_micros();
        validate_runtime_bundle_launcher(target, &candidate_dir.join(binary_name))?;
        Ok(())
    })();
    match transaction {
        Ok(()) => {}
        Err(error) => {
            let _ = std::fs::remove_file(&staged_pending);
            let rollback =
                restore_pending_runtime_artifact_activation(
                    &activation_event_path,
                    previous_pending.as_deref(),
                    &quiescence.lease.lease_nonce,
                )
                .and(slots.restore_targets_under_guard(
                    active_before.as_deref(),
                    healthy_before.as_deref(),
                ))
                .and(restore_runtime_artifact_symlink(
                    target,
                    stable_before.as_deref(),
                    &quiescence.lease.lease_nonce,
                ));
            quiescence.restore_after_failed_commit(&consumed_lease, &guard)?;
            drop(guard);
            discard_prepared_runtime_artifact(&prepared).await?;
            discard_prepared_runtime_artifact_bundle_members(&prepared_members).await?;
            rollback?;
            return Err(error);
        }
    }
    quiescence.finish_consumption(&consumed_lease, &guard)?;
    let lock_elapsed_micros = lock_started.elapsed().as_micros();
    drop(guard);
    prune_unreachable_runtime_artifacts(&artifact_root)
        .await
        .map_err(|error| {
        format!(
            "state=runtime-artifact-publication-committed reasonKind=runtime-state-retention-finalization-failed error={error}"
        )
    })?;

    Ok(RuntimeArtifactPublicationReceipt {
        path: target.to_path_buf(),
        status: "published-active-awaiting-health",
        artifact_digest: prepared.content_digest,
        bundle_digest,
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

async fn discard_prepared_runtime_artifact_bundle_members(
    members: &[PreparedRuntimeArtifactBundleMember],
) -> Result<(), String> {
    for member in members {
        discard_prepared_runtime_artifact(&member.artifact).await?;
    }
    Ok(())
}

fn publish_runtime_bundle_member_launcher(
    target: &Path,
    candidate: &Path,
    publication_nonce: &str,
    artifact_kind: &str,
) -> Result<(), String> {
    let parent = target.parent().ok_or_else(|| {
        format!(
            "Runtime bundle launcher has no parent: {}",
            target.display()
        )
    })?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("create Runtime bundle launcher directory: {error}"))?;
    match std::fs::read_link(target) {
        Ok(current) if current == candidate => return Ok(()),
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "read Runtime bundle launcher {}: {error}",
                target.display()
            ));
        }
    }
    let staged = parent.join(format!(".{artifact_kind}.{publication_nonce}.tmp"));
    match std::fs::remove_file(&staged) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("remove stale Runtime bundle launcher: {error}")),
    }
    stage_runtime_bundle_launcher(candidate, &staged)?;
    let observed = std::fs::read_link(&staged)
        .map_err(|error| format!("read staged Runtime bundle launcher: {error}"))?;
    if observed != candidate {
        let _ = std::fs::remove_file(&staged);
        return Err("reasonKind=runtime-bundle-launcher-candidate-mismatch".to_owned());
    }
    std::fs::rename(&staged, target).map_err(|error| {
        let _ = std::fs::remove_file(&staged);
        format!(
            "reasonKind=runtime-bundle-launcher-publication-failed target={} error={error}",
            target.display()
        )
    })
}

fn validate_runtime_bundle_launcher(target: &Path, candidate: &Path) -> Result<(), String> {
    let expected = std::fs::canonicalize(candidate)
        .map_err(|error| format!("resolve active Runtime bundle member: {error}"))?;
    let observed = std::fs::canonicalize(target)
        .map_err(|error| format!("resolve Runtime bundle launcher: {error}"))?;
    if observed != expected {
        return Err(format!(
            "reasonKind=runtime-bundle-launcher-active-mismatch target={} candidate={}",
            target.display(),
            candidate.display()
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn stage_runtime_bundle_launcher(candidate: &Path, staged: &Path) -> Result<(), String> {
    std::os::unix::fs::symlink(candidate, staged)
        .map_err(|error| format!("stage Runtime client launcher: {error}"))
}

#[cfg(not(unix))]
fn stage_runtime_bundle_launcher(candidate: &Path, staged: &Path) -> Result<(), String> {
    std::fs::copy(candidate, staged)
        .map(|_| ())
        .map_err(|error| format!("stage Runtime client launcher: {error}"))
}

#[cfg(test)]
#[path = "../tests/unit/runtime_artifact_publication.rs"]
mod tests;

#[cfg(test)]
#[path = "../tests/unit/runtime_artifact_publication_retention.rs"]
mod retention_tests;
