// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Millisecond immutable bundle publication and launcher switching.

#[path = "runtime_artifact_provider_publication.rs"]
mod provider_publication;
#[path = "runtime_artifact_publication_transaction.rs"]
mod transaction;

pub use provider_publication::publish_runtime_artifact_bound_provider_member_from_active;

use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

use crate::blake3_content_digest::Blake3ContentDigest;
use crate::runtime_artifact_activation::commit_staged_pending_runtime_artifact_activation;
use crate::runtime_artifact_activation::decode_runtime_artifact_activation_event;
use crate::runtime_artifact_activation::next_runtime_artifact_activation_generation_sync_under_guard;
use crate::runtime_artifact_activation::prepare_active_slot_snapshot;
use crate::runtime_artifact_activation::prepare_runtime_artifact_serving_snapshot;
use crate::runtime_artifact_activation::read_optional_symlink;
use crate::runtime_artifact_activation::restore_pending_runtime_artifact_activation;
use crate::runtime_artifact_activation::restore_runtime_artifact_symlink;
use crate::runtime_artifact_activation::runtime_artifact_activation_event_path;
use crate::runtime_artifact_activation::validate_current_activation_receipts_content;
use crate::runtime_artifact_activation::validate_current_activation_receipts_for_preverified_predecessor;
use crate::runtime_artifact_publication_support::{
    discard_prepared_runtime_artifact_bundle_members, publish_runtime_bundle_member_launcher,
    repair_empty_artifact_selector_under_guard, validate_runtime_bundle_launcher,
};
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
use transaction::RuntimeArtifactActivationStagingRequest;
use transaction::stage_runtime_artifact_activation_transaction;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeArtifactBundleMemberSource<'a> {
    pub name: &'a str,
    pub source: &'a Path,
}

fn client_bundle_stable_launchers<'a>(
    members: &'a [RuntimeArtifactBundleMemberSource<'a>],
) -> Vec<&'a str> {
    members
        .iter()
        .filter_map(|member| (member.name == "asp-hook").then_some(member.name))
        .collect()
}

#[derive(Debug)]
pub(super) struct PreparedRuntimeArtifactBundleMember {
    pub(super) artifact: PreparedRuntimeArtifact,
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

#[derive(Clone, Copy)]
enum PredecessorReceiptAdmission<'a> {
    Strict,
    PreverifiedProviderReplacement(&'a Blake3ContentDigest),
    PreverifiedBinaryReplacement(&'a Blake3ContentDigest),
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
        None,
        PredecessorReceiptAdmission::Strict,
        &[],
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
    let stable_launchers = client_bundle_stable_launchers(member_sources);
    publish_runtime_artifact_with_before_guard(
        state_home,
        source,
        target,
        artifact_mode,
        member_sources,
        None,
        None,
        PredecessorReceiptAdmission::Strict,
        &stable_launchers,
        || async {},
    )
    .await
}

/// Replace the Runtime client cohort while preserving every other verified
/// member of the current active generation.
///
/// This is the binary-refresh transaction. Provider executables already
/// admitted by the active bundle remain members of the successor generation;
/// callers cannot reconstruct the active manifest or republish providers
/// through a second catalog. A concurrent active switch fails the final CAS.
pub async fn publish_runtime_artifact_bundle_successor_from_active(
    state_home: &Path,
    source: &Path,
    target: &Path,
    artifact_mode: &str,
    replacement_members: &[RuntimeArtifactBundleMemberSource<'_>],
) -> Result<RuntimeArtifactPublicationReceipt, String> {
    let layout = crate::RuntimeArtifactStateLayout::new(state_home);
    let active_bundle = match std::fs::canonicalize(layout.active_slot()) {
        Ok(active) => active,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return publish_runtime_artifact_bundle_members(
                state_home,
                source,
                target,
                artifact_mode,
                replacement_members,
            )
            .await;
        }
        Err(error) => {
            return Err(format!(
                "reasonKind=runtime-active-generation-unavailable path={} error={error}",
                layout.active_slot().display()
            ));
        }
    };
    let manifest_bytes = tokio::fs::read(active_bundle.join("bundle.json"))
        .await
        .map_err(|error| format!("read active Runtime bundle identity: {error}"))?;
    let manifest_value: serde_json::Value = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| format!("decode active Runtime bundle identity: {error}"))?;
    if manifest_value.get("executionBinding").is_some() {
        return Err(
            "reasonKind=runtime-bound-binary-refresh-requires-rebound-execution-binding".to_owned(),
        );
    }
    let verified =
        crate::runtime_artifact_slots::verify_runtime_artifact_bundle(&active_bundle).await?;
    if !verified.members().contains_key("asp") {
        return Err("reasonKind=runtime-active-generation-primary-missing member=asp".to_owned());
    }
    let replacement_names = replacement_members
        .iter()
        .map(|member| member.name)
        .collect::<std::collections::BTreeSet<_>>();
    if replacement_names.len() != replacement_members.len() {
        return Err("duplicate Runtime replacement bundle member".to_owned());
    }
    let mut owned_members = verified
        .members()
        .keys()
        .filter(|name| name.as_str() != "asp" && !replacement_names.contains(name.as_str()))
        .map(|name| (name.clone(), active_bundle.join(name)))
        .collect::<Vec<_>>();
    owned_members.extend(
        replacement_members
            .iter()
            .map(|member| (member.name.to_owned(), member.source.to_path_buf())),
    );
    owned_members.sort_by(|left, right| left.0.cmp(&right.0));
    let successor_members = owned_members
        .iter()
        .map(|(name, member_source)| RuntimeArtifactBundleMemberSource {
            name: name.as_str(),
            source: member_source,
        })
        .collect::<Vec<_>>();
    let stable_launchers = client_bundle_stable_launchers(replacement_members);
    publish_runtime_artifact_with_before_guard(
        state_home,
        source,
        target,
        artifact_mode,
        &successor_members,
        None,
        Some(&active_bundle),
        PredecessorReceiptAdmission::Strict,
        &stable_launchers,
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
    let stable_launchers = client_bundle_stable_launchers(member_sources);
    publish_runtime_artifact_with_before_guard(
        state_home,
        source,
        target,
        artifact_mode,
        member_sources,
        Some(execution_binding),
        None,
        PredecessorReceiptAdmission::Strict,
        &stable_launchers,
        || async {},
    )
    .await
}

/// Publish an atomic protocol/Provider successor from an installation-only
/// preverified predecessor. Strict serving admission still applies to the
/// complete successor before the active selector moves.
#[expect(
    clippy::too_many_arguments,
    reason = "the public replacement transaction keeps every CAS identity input explicit"
)]
pub async fn publish_runtime_artifact_bound_bundle_members_for_binary_replacement(
    state_home: &Path,
    source: &Path,
    target: &Path,
    artifact_mode: &str,
    member_sources: &[RuntimeArtifactBundleMemberSource<'_>],
    execution_binding: &RuntimeArtifactBundleBinding,
    predecessor_bundle: &Path,
    predecessor_bundle_digest: &Blake3ContentDigest,
) -> Result<RuntimeArtifactPublicationReceipt, String> {
    let stable_launchers = client_bundle_stable_launchers(member_sources);
    publish_runtime_artifact_with_before_guard(
        state_home,
        source,
        target,
        artifact_mode,
        member_sources,
        Some(execution_binding),
        Some(predecessor_bundle),
        PredecessorReceiptAdmission::PreverifiedBinaryReplacement(predecessor_bundle_digest),
        &stable_launchers,
        || async {},
    )
    .await
}

/// Replace one executable capability by composing and publishing a complete
/// successor of the current active generation. The observed active bundle is
/// a CAS input: a concurrent activation makes this operation fail closed
/// instead of silently rebasing the provider onto different Runtime content.
pub async fn publish_runtime_artifact_bundle_member_from_active(
    state_home: &Path,
    member_name: &str,
    source: &Path,
    artifact_mode: &str,
) -> Result<RuntimeArtifactPublicationReceipt, String> {
    let member_path = Path::new(member_name);
    if member_path.components().count() != 1
        || member_name == "."
        || member_name == ".."
        || member_name == "asp"
    {
        return Err(format!(
            "invalid Runtime replacement bundle member `{member_name}`"
        ));
    }
    let layout = crate::RuntimeArtifactStateLayout::new(state_home);
    let active_bundle = std::fs::canonicalize(layout.active_slot()).map_err(|error| {
        format!(
            "reasonKind=runtime-active-generation-unavailable path={} error={error}",
            layout.active_slot().display()
        )
    })?;
    let manifest_bytes = tokio::fs::read(active_bundle.join("bundle.json"))
        .await
        .map_err(|error| format!("read active Runtime bundle identity: {error}"))?;
    let manifest_value: serde_json::Value = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| format!("decode active Runtime bundle identity: {error}"))?;
    if manifest_value.get("executionBinding").is_some() {
        return Err(
            "reasonKind=runtime-bound-provider-refresh-requires-rebound-execution-binding"
                .to_owned(),
        );
    }
    let verified =
        crate::runtime_artifact_slots::verify_runtime_artifact_bundle(&active_bundle).await?;
    let members = verified.members().clone();
    if !members.contains_key("asp") {
        return Err("reasonKind=runtime-active-generation-primary-missing member=asp".to_owned());
    }
    let asp_source = active_bundle.join("asp");
    let mut owned_members = members
        .keys()
        .filter(|name| name.as_str() != "asp" && name.as_str() != member_name)
        .map(|name| (name.clone(), active_bundle.join(name)))
        .collect::<Vec<_>>();
    owned_members.push((member_name.to_owned(), source.to_path_buf()));
    owned_members.sort_by(|left, right| left.0.cmp(&right.0));
    let member_sources = owned_members
        .iter()
        .map(|(name, source)| RuntimeArtifactBundleMemberSource {
            name: name.as_str(),
            source,
        })
        .collect::<Vec<_>>();
    let target = state_home.join("runtime/bin/asp");
    publish_runtime_artifact_with_before_guard(
        state_home,
        &asp_source,
        &target,
        artifact_mode,
        &member_sources,
        None,
        Some(&active_bundle),
        PredecessorReceiptAdmission::Strict,
        &[member_name],
        || async {},
    )
    .await
}

#[expect(
    clippy::too_many_arguments,
    reason = "the internal publication seam keeps guard and CAS identity inputs explicit"
)]
async fn publish_runtime_artifact_with_before_guard<BeforeGuard, BeforeGuardFuture>(
    state_home: &Path,
    source: &Path,
    target: &Path,
    artifact_mode: &str,
    member_sources: &[RuntimeArtifactBundleMemberSource<'_>],
    execution_binding: Option<&RuntimeArtifactBundleBinding>,
    expected_active_bundle: Option<&Path>,
    predecessor_receipt_admission: PredecessorReceiptAdmission<'_>,
    stable_member_launchers: &[&str],
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
    for member in stable_member_launchers {
        let path = Path::new(member);
        if path.components().count() != 1
            || *member == "."
            || *member == ".."
            || *member == binary_name
        {
            return Err(format!(
                "invalid or duplicate Runtime stable member launcher `{member}`"
            ));
        }
    }
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
    let candidate_dir = layout.generation_store().join(token);
    let _candidate_preparation_lease =
        RuntimeArtifactCandidatePreparationLease::acquire(state_home, binary_name, token)?;
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
        return Err(format!(
            "state=runtime-artifact-publication-failed reasonKind=candidate-admission-failed error={error}"
        ));
    }
    let activation_event_path = runtime_artifact_activation_event_path(state_home);

    let quiescence_operation = format!("publish:{binary_name}");
    match predecessor_receipt_admission {
        PredecessorReceiptAdmission::Strict => {
            validate_current_activation_receipts_content(state_home).await?
        }
        PredecessorReceiptAdmission::PreverifiedProviderReplacement(bundle_digest) => {
            let predecessor = expected_active_bundle.ok_or_else(|| {
                "state=runtime-artifact-publication-failed reasonKind=provider-replacement-predecessor-missing"
                    .to_owned()
            })?;
            validate_current_activation_receipts_for_preverified_predecessor(
                state_home,
                predecessor,
                bundle_digest,
            )
            .await?;
        }
        PredecessorReceiptAdmission::PreverifiedBinaryReplacement(bundle_digest) => {
            let predecessor = expected_active_bundle.ok_or_else(|| {
                "state=runtime-artifact-publication-failed reasonKind=binary-replacement-predecessor-missing"
                    .to_owned()
            })?;
            validate_current_activation_receipts_for_preverified_predecessor(
                state_home,
                predecessor,
                bundle_digest,
            )
            .await?;
        }
    }
    before_guard().await;

    let mut phase_trace = RuntimeArtifactPublicationPhaseTrace::default();
    let repair_guard = match RuntimeArtifactMutationGuard::try_acquire(&artifact_root) {
        Ok(guard) => guard,
        Err(error) => {
            discard_prepared_runtime_artifact(&prepared).await?;
            discard_prepared_runtime_artifact_bundle_members(&prepared_members).await?;
            return Err(error);
        }
    };
    repair_empty_artifact_selector_under_guard(&layout.active_slot())?;
    repair_empty_artifact_selector_under_guard(&layout.healthy_slot())?;
    drop(repair_guard);

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

    // Serialize and durably stage the transaction before acquiring the global
    // mutation guard. The guard admits the lease and generation fence, then
    // performs only bounded receipt reads and atomic namespace switches.
    let phase_started = std::time::Instant::now();
    let activation_generation = match crate::runtime_artifact_activation::next_runtime_artifact_activation_generation_under_guard(state_home).await {
        Ok(generation) => generation,
        Err(error) => {
            discard_prepared_runtime_artifact(&prepared).await?;
            discard_prepared_runtime_artifact_bundle_members(&prepared_members).await?;
            return Err(error);
        }
    };
    let staged = match stage_runtime_artifact_activation_transaction(
        RuntimeArtifactActivationStagingRequest {
            state_home,
            activation_event_path: &activation_event_path,
            candidate_dir: &candidate_dir,
            artifact_path: &prepared.path,
            stable_path: target,
            artifact_mode,
            quiescence_operation: &quiescence_operation,
            activation_generation,
            bundle_digest: &bundle_digest,
            artifact_digest: &prepared.content_digest,
            previous_artifact_digest: serving_snapshot.artifact_digest.clone(),
        },
    ) {
        Ok(staged) => staged,
        Err(error) => {
            discard_prepared_runtime_artifact(&prepared).await?;
            discard_prepared_runtime_artifact_bundle_members(&prepared_members).await?;
            return Err(error);
        }
    };
    let event = staged.event;
    let staged_pending = staged.pending_path;
    let mut quiescence = staged.quiescence;
    phase_trace.candidate_activation_staging_micros = phase_started.elapsed().as_micros();

    let phase_started = std::time::Instant::now();
    let guard = match RuntimeArtifactMutationGuard::try_acquire(&artifact_root) {
        Ok(guard) => guard,
        Err(error) => {
            let _ = std::fs::remove_file(&staged_pending);
            discard_prepared_runtime_artifact(&prepared).await?;
            discard_prepared_runtime_artifact_bundle_members(&prepared_members).await?;
            return Err(error);
        }
    };
    phase_trace.guard_acquisition_micros = phase_started.elapsed().as_micros();
    let lock_started = std::time::Instant::now();
    if let Err(error) = quiescence.admit_under_artifact_guard(&guard) {
        drop(guard);
        let _ = std::fs::remove_file(&staged_pending);
        discard_prepared_runtime_artifact(&prepared).await?;
        discard_prepared_runtime_artifact_bundle_members(&prepared_members).await?;
        return Err(error);
    }
    let admitted_generation =
        match next_runtime_artifact_activation_generation_sync_under_guard(state_home) {
            Ok(generation) => generation,
            Err(error) => {
                drop(guard);
                let _ = std::fs::remove_file(&staged_pending);
                discard_prepared_runtime_artifact(&prepared).await?;
                discard_prepared_runtime_artifact_bundle_members(&prepared_members).await?;
                return Err(error);
            }
        };
    if admitted_generation != event.activation_generation {
        let lease_cleanup = quiescence
            .consume_under_artifact_guard(&guard)
            .and_then(|consumed| quiescence.finish_consumption(&consumed, &guard));
        drop(guard);
        let _ = std::fs::remove_file(&staged_pending);
        discard_prepared_runtime_artifact(&prepared).await?;
        discard_prepared_runtime_artifact_bundle_members(&prepared_members).await?;
        lease_cleanup?;
        return Err(format!(
            "state=runtime-artifact-publication-failed reasonKind=activation-generation-cas-mismatch expectedGeneration={} actualGeneration={admitted_generation}",
            event.activation_generation
        ));
    }

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
    if expected_active_bundle.is_some_and(|expected| {
        !observed_active.as_ref().is_some_and(|observed| {
            std::fs::canonicalize(observed).is_ok_and(|identity| identity == expected)
        })
    }) {
        let _ = std::fs::remove_file(&staged_pending);
        quiescence.restore_after_failed_commit(&consumed_lease, &guard)?;
        drop(guard);
        discard_prepared_runtime_artifact(&prepared).await?;
        discard_prepared_runtime_artifact_bundle_members(&prepared_members).await?;
        return Err(
            "state=runtime-artifact-publication-failed reasonKind=active-generation-cas-mismatch"
                .to_owned(),
        );
    }
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
        Some(healthy) => match std::fs::canonicalize(healthy.join(binary_name)) {
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
    let launcher_directory = target.parent().ok_or_else(|| {
        format!(
            "Runtime stable launcher has no parent: {}",
            target.display()
        )
    })?;
    let additional_launcher_before = stable_member_launchers
        .iter()
        .map(|member| {
            let launcher = launcher_directory.join(member);
            read_optional_symlink(&launcher).map(|previous| (launcher, previous))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let runtime_launcher_directory = state_home.join("runtime/bin");
    let runtime_primary_alias = runtime_launcher_directory.join(binary_name);
    let runtime_compatibility_aliases = if runtime_primary_alias != target {
        std::iter::once((runtime_primary_alias, target.to_path_buf()))
            .chain(stable_member_launchers.iter().map(|member| {
                (
                    runtime_launcher_directory.join(member),
                    launcher_directory.join(member),
                )
            }))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let runtime_compatibility_before = runtime_compatibility_aliases
        .iter()
        .map(|(alias, _)| read_optional_symlink(alias).map(|previous| (alias.clone(), previous)))
        .collect::<Result<Vec<_>, String>>()?;
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
        for member in stable_member_launchers {
            let launcher = launcher_directory.join(member);
            publish_runtime_bundle_member_launcher(
                &launcher,
                &layout.active_slot().join(member),
                &event.publication_nonce,
                member,
            )?;
            validate_runtime_bundle_launcher(&launcher, &candidate_dir.join(member))?;
        }
        for (alias, public_entry) in &runtime_compatibility_aliases {
            let artifact_kind = alias
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| "Runtime compatibility alias has no binary name".to_owned())?;
            publish_runtime_bundle_member_launcher(
                alias,
                public_entry,
                &event.publication_nonce,
                artifact_kind,
            )?;
            validate_runtime_bundle_launcher(alias, &candidate_dir.join(artifact_kind))?;
        }
        phase_trace.client_launcher_switch_micros = phase_started.elapsed().as_micros();
        validate_runtime_bundle_launcher(target, &candidate_dir.join(binary_name))?;
        Ok(())
    })();
    match transaction {
        Ok(()) => {}
        Err(error) => {
            let _ = std::fs::remove_file(&staged_pending);
            let mut rollback =
                restore_pending_runtime_artifact_activation(
                    &activation_event_path,
                    previous_pending.as_deref(),
                    &quiescence.lease.lease_nonce,
                )
                .and(slots.restore_targets_under_guard(
                    active_before.as_deref(),
                    healthy_before.as_deref(),
                ));
            // Restore compatibility aliases before the public entry. A legacy
            // predecessor may have pointed the public entry back at the
            // Runtime namespace; reversing this order would transiently
            // recreate a two-node loop during rollback.
            for (alias, previous) in &runtime_compatibility_before {
                rollback = rollback.and(restore_runtime_artifact_symlink(
                    alias,
                    previous.as_deref(),
                    &quiescence.lease.lease_nonce,
                ));
            }
            for (launcher, previous) in &additional_launcher_before {
                rollback = rollback.and(restore_runtime_artifact_symlink(
                    launcher,
                    previous.as_deref(),
                    &quiescence.lease.lease_nonce,
                ));
            }
            rollback = rollback.and(restore_runtime_artifact_symlink(
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
        quiescence_lease_nonce: quiescence.lease.lease_nonce.clone(),
        lease_producer_process_id: quiescence.lease.producer_process_id,
        lease_consumer_process_id: std::process::id(),
    })
}

#[cfg(test)]
#[path = "../tests/unit/runtime_artifact_publication.rs"]
mod tests;

#[cfg(test)]
#[path = "../tests/unit/runtime_artifact_publication_retention.rs"]
mod retention_tests;
