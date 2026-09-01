use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use super::{
    publish_runtime_artifact, publish_runtime_artifact_bundle,
    publish_runtime_artifact_with_before_guard,
};
use crate::blake3_content_digest::Blake3ContentDigest;
use crate::runtime_artifact_activation::{
    RuntimeArtifactActivationEvent, RuntimeArtifactCandidateIdentityReceipt,
    canonicalize_legacy_activation_receipts_under_guard, commit_runtime_artifact_activation,
    decode_runtime_artifact_activation_event, publish_applied_runtime_artifact_activation,
    read_applied_runtime_artifact_activation_event, read_runtime_artifact_activation_event,
    rollback_runtime_artifact_activation, runtime_artifact_activation_event_path,
};
use crate::runtime_artifact_quiescence::prepare_runtime_artifact_quiescence_lease;
use crate::runtime_artifact_retention::RuntimeArtifactMutationGuard;
use crate::runtime_artifact_slots::{
    RuntimeArtifactSlotAuthority, runtime_artifact_bundle_digest, runtime_artifact_candidate_digest,
};

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

    assert_eq!(receipt.status, "published-active-awaiting-health");
    assert!(receipt.activation_event_path.is_file());
    let artifact_root = state_home.join("runtime/artifacts");
    let guard = RuntimeArtifactMutationGuard::try_acquire(&artifact_root)
        .expect("artifact lock must be released when publication returns");
    drop(guard);
}

#[tokio::test]
async fn production_publication_atomically_switches_active_and_retains_previous_healthy() {
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

    let active_after = slots.active_target().await.unwrap();
    assert_eq!(active_after, Some(pending.candidate_slot_path.clone()));
    assert_eq!(
        std::fs::canonicalize(&target).unwrap(),
        std::fs::canonicalize(pending.candidate_slot_path.join("asp")).unwrap()
    );
    assert_ne!(active_after, active_before);
    assert_eq!(slots.healthy_target().await.unwrap(), healthy_before);
    assert_eq!(
        pending.previous_artifact_digest,
        Some(previous.artifact_digest)
    );
}

#[tokio::test]
async fn runtime_and_hook_launchers_follow_one_active_bundle_selector() {
    let temporary = tempfile::tempdir().expect("bundle publication fixture");
    let state_home = temporary.path().join("state");
    let asp_source = temporary.path().join("asp");
    let hook_source = temporary.path().join("asp-hook");
    let asp_target = state_home.join("runtime/bin/asp");
    let hook_target = state_home.join("runtime/bin/asp-hook");
    write_executable(&asp_source, "#!/bin/sh\nexit 0\n");
    write_executable(&hook_source, "#!/bin/sh\nexit 0\n");

    let publication =
        publish_runtime_artifact_bundle(&state_home, &asp_source, &asp_target, "dev", &hook_source)
            .await
            .expect("publish one immutable Runtime/Hook bundle");

    let slots = RuntimeArtifactSlotAuthority::new(state_home.join("runtime/resident"));
    let active = slots.active_target().await.unwrap().expect("active bundle");
    assert_eq!(
        std::fs::canonicalize(&asp_target).unwrap(),
        std::fs::canonicalize(active.join("asp")).unwrap()
    );
    assert_eq!(
        std::fs::canonicalize(&hook_target).unwrap(),
        std::fs::canonicalize(active.join("asp-hook")).unwrap()
    );
    assert!(active.join("bundle.json").is_file());
    assert!(slots.healthy_target().await.unwrap().is_none());

    let pending = read_runtime_artifact_activation_event(&state_home)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(pending.bundle_digest, publication.bundle_digest);
    assert_ne!(pending.bundle_digest, pending.artifact_digest);
    let pending_bytes = std::fs::read(&publication.activation_event_path).unwrap();
    let mut legacy_generation: serde_json::Value = serde_json::from_slice(&pending_bytes).unwrap();
    legacy_generation["activationGeneration"] = serde_json::json!(54);
    assert!(
        decode_runtime_artifact_activation_event(
            &serde_json::to_vec(&legacy_generation).unwrap(),
            "legacy numeric generation fixture",
        )
        .is_err()
    );
    let mut missing_bundle: serde_json::Value = serde_json::from_slice(&pending_bytes).unwrap();
    missing_bundle
        .as_object_mut()
        .unwrap()
        .remove("bundleDigest");
    assert!(
        decode_runtime_artifact_activation_event(
            &serde_json::to_vec(&missing_bundle).unwrap(),
            "missing bundle identity fixture",
        )
        .is_err()
    );
    commit_runtime_artifact_activation(&state_home, &pending, None)
        .await
        .expect("mark the same active bundle healthy");
    assert_eq!(slots.active_target().await.unwrap(), Some(active.clone()));
    assert_eq!(slots.healthy_target().await.unwrap(), Some(active));
}

#[tokio::test]
async fn repeated_bundle_publication_moves_only_active_and_keeps_launchers_fixed() {
    let temporary = tempfile::tempdir().expect("fixed bundle launcher fixture");
    let state_home = temporary.path().join("state");
    let first_asp = temporary.path().join("asp-first");
    let first_hook = temporary.path().join("asp-hook-first");
    let next_asp = temporary.path().join("asp-next");
    let next_hook = temporary.path().join("asp-hook-next");
    let asp_target = state_home.join("runtime/bin/asp");
    let hook_target = state_home.join("runtime/bin/asp-hook");
    write_executable(&first_asp, "#!/bin/sh\nexit 10\n");
    write_executable(&first_hook, "#!/bin/sh\nexit 11\n");
    write_executable(&next_asp, "#!/bin/sh\nexit 20\n");
    write_executable(&next_hook, "#!/bin/sh\nexit 21\n");

    publish_runtime_artifact_bundle(&state_home, &first_asp, &asp_target, "dev", &first_hook)
        .await
        .expect("publish first bundle while Runtime is absent");
    let first = read_runtime_artifact_activation_event(&state_home)
        .await
        .unwrap()
        .unwrap();
    commit_runtime_artifact_activation(&state_home, &first, None)
        .await
        .expect("mark first bundle healthy");
    let fixed_asp_launcher = std::fs::read_link(&asp_target).unwrap();
    let fixed_hook_launcher = std::fs::read_link(&hook_target).unwrap();
    let slots = RuntimeArtifactSlotAuthority::new(state_home.join("runtime/resident"));
    let healthy_before = slots.healthy_target().await.unwrap();

    publish_runtime_artifact_bundle(&state_home, &next_asp, &asp_target, "dev", &next_hook)
        .await
        .expect("publish next bundle while Runtime is absent");

    let active_after = slots.active_target().await.unwrap();
    assert_ne!(active_after, healthy_before);
    assert_eq!(slots.healthy_target().await.unwrap(), healthy_before);
    assert_eq!(std::fs::read_link(&asp_target).unwrap(), fixed_asp_launcher);
    assert_eq!(
        std::fs::read_link(&hook_target).unwrap(),
        fixed_hook_launcher
    );
}

#[tokio::test]
async fn activation_failure_restores_active_from_healthy_without_moving_healthy() {
    let temporary = tempfile::tempdir().expect("bundle rollback fixture");
    let state_home = temporary.path().join("state");
    let previous_source = temporary.path().join("asp-previous");
    let candidate_source = temporary.path().join("asp-candidate");
    let target = state_home.join("runtime/bin/asp");
    write_executable(&previous_source, "#!/bin/sh\nexit 0\n");
    write_executable(&candidate_source, "#!/bin/sh\nexit 1\n");

    publish_runtime_artifact(&state_home, &previous_source, &target, "dev")
        .await
        .expect("publish previous bundle");
    let previous = read_runtime_artifact_activation_event(&state_home)
        .await
        .unwrap()
        .unwrap();
    commit_runtime_artifact_activation(&state_home, &previous, None)
        .await
        .expect("mark previous bundle healthy");

    publish_runtime_artifact(&state_home, &candidate_source, &target, "dev")
        .await
        .expect("publish active candidate bundle");
    let candidate = read_runtime_artifact_activation_event(&state_home)
        .await
        .unwrap()
        .unwrap();
    let slots = RuntimeArtifactSlotAuthority::new(state_home.join("runtime/resident"));
    let healthy_before = slots.healthy_target().await.unwrap();
    assert_eq!(
        slots.active_target().await.unwrap(),
        Some(candidate.candidate_slot_path.clone())
    );

    rollback_runtime_artifact_activation(&state_home, &candidate)
        .await
        .expect("restore previous healthy bundle");
    assert_eq!(slots.active_target().await.unwrap(), healthy_before);
    assert_eq!(slots.healthy_target().await.unwrap(), healthy_before);

    rollback_runtime_artifact_activation(&state_home, &candidate)
        .await
        .expect("replaying the same rollback is an idempotent no-op");
    assert_eq!(slots.active_target().await.unwrap(), healthy_before);
    assert_eq!(slots.healthy_target().await.unwrap(), healthy_before);
    assert_eq!(
        std::fs::canonicalize(&target).unwrap(),
        std::fs::canonicalize(
            healthy_before
                .as_ref()
                .expect("previous healthy bundle")
                .join("asp")
        )
        .unwrap()
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
    let new_members =
        std::collections::BTreeMap::from([("asp".to_owned(), new_serving_digest.clone())]);
    let new_bundle_digest = runtime_artifact_bundle_digest(&new_members);
    let new_token = new_bundle_digest.content_digest().as_str();
    let new_candidate_dir = state_home
        .join("runtime/resident/candidates/asp")
        .join(new_token);
    let new_prepared = crate::runtime_artifact_slots::prepare_runtime_artifact_candidate_for_kind(
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
    std::fs::write(
        new_candidate_dir.join("bundle.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "schemaId": "agent.semantic-protocols.runtime-binary-bundle",
            "schemaVersion": 1,
            "bundleDigest": new_bundle_digest.clone(),
            "members": new_members,
        }))
        .unwrap(),
    )
    .expect("stage newer bundle identity");
    let new_nonce = format!(
        "publication-new-{}",
        new_serving_digest.content_digest().as_str()
    );
    let new_applied = RuntimeArtifactActivationEvent {
        schema_id: "agent.semantic-protocols.runtime-artifact-activation".to_owned(),
        schema_version: 1,
        bundle_digest: new_bundle_digest,
        artifact_digest: new_serving_digest.clone(),
        artifact_path: new_prepared.path,
        candidate_slot_path: new_candidate_dir.clone(),
        previous_artifact_digest: Some(old.artifact_digest.clone()),
        artifact_mode: "release".to_owned(),
        published_at_unix_millis: old.published_at_unix_millis + 1,
        publication_nonce: new_nonce.clone(),
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
            None,
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
        .publish_active_candidate(&new_candidate_dir)
        .await
        .expect("actor publishes the newer active identity under the guard");
    slots
        .mark_active_healthy(&new_candidate_dir)
        .await
        .expect("actor marks the newer active identity healthy under the guard");
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

    std::fs::write(
        runtime_artifact_activation_event_path(&state_home),
        b"{malformed",
    )
    .unwrap();
    let malformed = publish_runtime_artifact(&state_home, &next_source, &target, "release")
        .await
        .expect_err("malformed pending must fail closed");
    assert!(malformed.contains("reasonKind=legacy-activation-receipt-invalid"));
    assert_eq!(std::fs::canonicalize(&target).unwrap(), client_before);
    assert_eq!(slots.active_target().await.unwrap(), active_before);
    assert_eq!(slots.healthy_target().await.unwrap(), healthy_before);
}

#[tokio::test]
async fn stale_actor_commit_cannot_overwrite_newer_pending_publication() {
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
    assert!(stale.contains("reasonKind=stale-active-bundle"));
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
    assert_eq!(applied_after.artifact_digest, newer.artifact_digest);
    assert_eq!(applied_after.publication_nonce, newer.publication_nonce);
}

#[tokio::test]
async fn applied_receipt_retires_only_the_matching_pending_publication() {
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
        .expect("pending content-bound publication");
    assert!(
        read_applied_runtime_artifact_activation_event(&state_home)
            .await
            .expect("read absent applied activation")
            .is_none()
    );

    let committed = commit_runtime_artifact_activation(&state_home, &pending, None)
        .await
        .expect("commit matching content-bound publication");
    assert_eq!(committed.state, "applied");
    assert_eq!(committed.artifact_digest, pending.artifact_digest);
    let applied = read_applied_runtime_artifact_activation_event(&state_home)
        .await
        .expect("read applied activation")
        .expect("applied content-bound publication");
    assert_eq!(applied.artifact_digest, pending.artifact_digest);
    assert_eq!(applied.publication_nonce, pending.publication_nonce);
    assert!(
        read_runtime_artifact_activation_event(&state_home)
            .await
            .expect("read retired pending activation")
            .is_none()
    );

    publish_runtime_artifact(&state_home, &source, &target, "dev")
        .await
        .expect("republish the same content with a distinct publication nonce");
    let republished = read_runtime_artifact_activation_event(&state_home)
        .await
        .expect("read republished activation")
        .expect("an applied nonce must not hide a distinct pending nonce");
    assert_eq!(republished.artifact_digest, pending.artifact_digest);
    assert_ne!(republished.publication_nonce, pending.publication_nonce);
    commit_runtime_artifact_activation(&state_home, &republished, Some(&pending.artifact_digest))
        .await
        .expect("commit the matching republished nonce");
    assert!(
        read_runtime_artifact_activation_event(&state_home)
            .await
            .expect("read second retired pending activation")
            .is_none()
    );

    std::fs::write(&source, b"#!/bin/sh\nexit 98\n").expect("write distinct fixture content");
    publish_runtime_artifact(&state_home, &source, &target, "dev")
        .await
        .expect("publish distinct content as a new bundle");
    let distinct = read_runtime_artifact_activation_event(&state_home)
        .await
        .expect("read distinct pending activation")
        .expect("an applied digest must not hide distinct pending content");
    assert_ne!(distinct.artifact_digest, republished.artifact_digest);
    assert_ne!(distinct.publication_nonce, republished.publication_nonce);
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
    let artifact_root = state_home.join("runtime/artifacts");
    let guard = RuntimeArtifactMutationGuard::try_acquire(&artifact_root)
        .expect("acquire guard for competing lease fixture");
    let prepared_lease = prepare_runtime_artifact_quiescence_lease(
        &state_home,
        "publish:asp",
        &precise_digest,
        &guard,
    )
    .expect("prepare mismatched lease");
    drop(guard);
    let lease_path =
        crate::runtime_artifact_quiescence::runtime_artifact_quiescence_lease_path(&state_home);
    let lease_before = tokio::fs::read(&lease_path).await.expect("lease before");

    let error = publish_runtime_artifact(&state_home, &next_source, &target, "dev")
        .await
        .expect_err("mismatched typed digest must fail before slot mutation");
    assert!(error.contains("reasonKind=runtime-artifact-quiescence-live-owner-conflict"));
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

    let guard = RuntimeArtifactMutationGuard::try_acquire(&artifact_root)
        .expect("reacquire guard for lease cleanup");
    let consumed = prepared_lease
        .consume_under_artifact_guard(&guard)
        .expect("preserved lease remains consumable exactly once");
    prepared_lease
        .finish_consumption(&consumed, &guard)
        .expect("finalize preserved lease");
    assert!(prepared_lease.consume_under_artifact_guard(&guard).is_err());
}

#[tokio::test]
async fn pre_candidate_slot_receipt_is_content_proven_then_replaced() {
    let temporary = tempfile::tempdir().expect("legacy activation fixture");
    let state_home = temporary.path().join("state");
    let initial_source = temporary.path().join("asp-initial");
    let next_source = temporary.path().join("asp-next");
    let target = state_home.join("runtime/bin/asp");
    write_executable(&initial_source, "#!/bin/sh\nexit 0\n");
    write_executable(&next_source, "#!/bin/sh\nexit 1\n");

    publish_runtime_artifact(&state_home, &initial_source, &target, "dev")
        .await
        .expect("publish initial activation");
    let initial = read_runtime_artifact_activation_event(&state_home)
        .await
        .unwrap()
        .unwrap();
    commit_runtime_artifact_activation(&state_home, &initial, None)
        .await
        .expect("commit initial activation");
    let legacy_path = initial.candidate_slot_path.join("activation.json");
    std::fs::write(
        &legacy_path,
        serde_json::to_vec(&serde_json::json!({
            "artifactDigest": initial.artifact_digest,
            "artifactPath": initial.artifact_path,
            "previousArtifactDigest": initial.previous_artifact_digest,
            "artifactMode": initial.artifact_mode,
            "publishedAtUnixMillis": initial.published_at_unix_millis,
        }))
        .unwrap(),
    )
    .unwrap();

    publish_runtime_artifact(&state_home, &next_source, &target, "dev")
        .await
        .expect("content-proven legacy state must not block current publication");
    let pending = read_runtime_artifact_activation_event(&state_home)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        pending.previous_artifact_digest,
        Some(initial.artifact_digest)
    );
    assert_ne!(pending.candidate_slot_path, initial.candidate_slot_path);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(
            &std::fs::read(pending.candidate_slot_path.join("activation.json")).unwrap()
        )
        .unwrap()["candidateSlotPath"],
        pending.candidate_slot_path.to_string_lossy().as_ref()
    );
}

#[tokio::test]
async fn active_and_healthy_legacy_outer_slot_resolves_only_a_content_proven_nested_candidate() {
    let temporary = tempfile::tempdir().expect("nested legacy slot fixture");
    let state_home = temporary.path().join("state");
    let initial_source = temporary.path().join("asp-initial");
    let next_source = temporary.path().join("asp-next");
    let target = state_home.join("runtime/bin/asp");
    write_executable(&initial_source, "#!/bin/sh\nexit 0\n");
    write_executable(&next_source, "#!/bin/sh\nexit 1\n");

    publish_runtime_artifact(&state_home, &initial_source, &target, "release")
        .await
        .expect("publish legacy candidate fixture");
    let initial = read_runtime_artifact_activation_event(&state_home)
        .await
        .unwrap()
        .unwrap();
    commit_runtime_artifact_activation(&state_home, &initial, None)
        .await
        .expect("commit legacy candidate fixture");

    let transitional = serde_json::json!({
        "artifactDigest": initial.artifact_digest,
        "artifactPath": initial.artifact_path,
        "candidateSlotPath": initial.candidate_slot_path,
        "previousArtifactDigest": initial.previous_artifact_digest,
        "artifactMode": initial.artifact_mode,
        "publishedAtUnixMillis": initial.published_at_unix_millis,
        "publicationNonce": initial.publication_nonce,
        "activationGeneration": 85,
        "candidateIdentity": initial.candidate_identity,
    });
    let nested_receipt_path = initial.candidate_slot_path.join("activation.json");
    std::fs::write(
        &nested_receipt_path,
        serde_json::to_vec_pretty(&transitional).unwrap(),
    )
    .unwrap();

    let legacy_outer = state_home.join("runtime/resident/candidates/legacy-outer");
    std::fs::create_dir_all(&legacy_outer).unwrap();
    std::os::unix::fs::symlink(
        initial.candidate_slot_path.join("asp"),
        legacy_outer.join("asp"),
    )
    .unwrap();
    std::fs::write(
        legacy_outer.join("activation.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "artifactDigest": initial.artifact_digest,
            "artifactPath": initial.artifact_path,
            "previousArtifactDigest": null,
            "artifactMode": "release",
            "publishedAtUnixMillis": initial.published_at_unix_millis,
        }))
        .unwrap(),
    )
    .unwrap();
    let slots = RuntimeArtifactSlotAuthority::new(state_home.join("runtime/resident"));
    let guard = RuntimeArtifactMutationGuard::try_acquire(&state_home.join("runtime/artifacts"))
        .expect("acquire selector mutation guard");
    slots
        .restore_targets_under_guard(Some(&legacy_outer), Some(&legacy_outer))
        .expect("mount exact active+healthy legacy topology");
    drop(guard);

    let mut mismatched = transitional.clone();
    mismatched["candidateIdentity"]["artifactDigest"] =
        serde_json::Value::String(format!("blake3-256:{}", "f".repeat(64)));
    std::fs::write(
        &nested_receipt_path,
        serde_json::to_vec_pretty(&mismatched).unwrap(),
    )
    .unwrap();
    let error = publish_runtime_artifact(&state_home, &next_source, &target, "release")
        .await
        .expect_err("mismatched nested identity must fail closed");
    assert!(error.contains("reasonKind=legacy-candidate-identity-invalid"));
    assert_eq!(
        slots.active_target().await.unwrap(),
        Some(legacy_outer.clone())
    );
    assert_eq!(
        slots.healthy_target().await.unwrap(),
        Some(legacy_outer.clone())
    );

    std::fs::write(
        &nested_receipt_path,
        serde_json::to_vec_pretty(&transitional).unwrap(),
    )
    .unwrap();
    publish_runtime_artifact(&state_home, &next_source, &target, "release")
        .await
        .expect("content-proven nested candidate admits current publication");
    let pending = read_runtime_artifact_activation_event(&state_home)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        pending.previous_artifact_digest,
        Some(initial.artifact_digest)
    );
    assert_ne!(
        slots.active_target().await.unwrap(),
        Some(legacy_outer.clone())
    );
    assert_eq!(slots.healthy_target().await.unwrap(), Some(legacy_outer));
}

#[tokio::test]
async fn pending_and_applied_legacy_receipts_are_atomically_canonicalized() {
    let temporary = tempfile::tempdir().expect("legacy activation fixture");
    let state_home = temporary.path().join("state");
    let source = temporary.path().join("asp");
    let target = state_home.join("runtime/bin/asp");
    write_executable(&source, "#!/bin/sh\nexit 0\n");
    publish_runtime_artifact(&state_home, &source, &target, "dev")
        .await
        .expect("publish source fixture");
    let current = read_runtime_artifact_activation_event(&state_home)
        .await
        .unwrap()
        .unwrap();
    std::fs::remove_file(current.candidate_slot_path.join("bundle.json"))
        .expect("mount exact pre-bundle candidate");
    let legacy = serde_json::json!({
        "activationGeneration": 0,
        "artifactDigest": current.artifact_digest,
        "artifactMode": current.artifact_mode,
        "artifactPath": current.artifact_path,
        "candidateIdentity": current.candidate_identity,
        "candidateSlotPath": current.candidate_slot_path,
        "previousArtifactDigest": current.previous_artifact_digest,
        "publicationNonce": current.publication_nonce,
        "publishedAtUnixMillis": current.published_at_unix_millis,
    });
    let pending_bytes = serde_json::to_vec_pretty(&legacy).unwrap();
    let mut applied_legacy = legacy.clone();
    applied_legacy["activationGeneration"] = serde_json::json!("discarded-old-counter");
    let applied_bytes = serde_json::to_vec_pretty(&applied_legacy).unwrap();
    let pending_path = runtime_artifact_activation_event_path(&state_home);
    let applied_path = state_home.join("runtime/activation/applied.json");
    std::fs::write(&pending_path, &pending_bytes).unwrap();
    std::fs::write(&applied_path, &applied_bytes).unwrap();

    let guard = RuntimeArtifactMutationGuard::try_acquire(&state_home.join("runtime/artifacts"))
        .expect("acquire canonicalizer guard");
    canonicalize_legacy_activation_receipts_under_guard(&state_home, &guard)
        .await
        .expect("canonicalize pending and applied together");
    drop(guard);

    for path in [&pending_path, &applied_path] {
        let canonical_bytes = std::fs::read(path).unwrap();
        let canonical = decode_runtime_artifact_activation_event(
            &canonical_bytes,
            "canonicalized legacy receipt",
        )
        .expect("decode hard-cut receipt");
        assert_eq!(canonical.artifact_digest, current.artifact_digest);
        assert_eq!(canonical.publication_nonce, current.publication_nonce);
        let value: serde_json::Value = serde_json::from_slice(&canonical_bytes).unwrap();
        assert!(value.get("activationGeneration").is_none());
        assert!(value.get("bundleDigest").is_some());
    }
    assert_eq!(
        std::fs::read(&pending_path).unwrap(),
        std::fs::read(&applied_path).unwrap()
    );
}

#[tokio::test]
async fn legacy_activation_identity_mismatches_fail_before_receipt_or_selector_mutation() {
    let temporary = tempfile::tempdir().expect("legacy activation rejection fixture");
    let state_home = temporary.path().join("state");
    let source = temporary.path().join("asp");
    let target = state_home.join("runtime/bin/asp");
    write_executable(&source, "#!/bin/sh\nexit 0\n");
    publish_runtime_artifact(&state_home, &source, &target, "dev")
        .await
        .expect("publish source fixture");
    let current = read_runtime_artifact_activation_event(&state_home)
        .await
        .unwrap()
        .unwrap();
    std::fs::remove_file(current.candidate_slot_path.join("bundle.json"))
        .expect("mount exact pre-bundle candidate");
    let base = serde_json::json!({
        "activationGeneration": 86,
        "artifactDigest": current.artifact_digest,
        "artifactMode": current.artifact_mode,
        "artifactPath": current.artifact_path,
        "candidateIdentity": current.candidate_identity,
        "candidateSlotPath": current.candidate_slot_path,
        "previousArtifactDigest": current.previous_artifact_digest,
        "publicationNonce": current.publication_nonce,
        "publishedAtUnixMillis": current.published_at_unix_millis,
    });
    let pending_path = runtime_artifact_activation_event_path(&state_home);
    let slots = RuntimeArtifactSlotAuthority::new(state_home.join("runtime/resident"));
    let active_before = slots.active_target().await.unwrap();
    let healthy_before = slots.healthy_target().await.unwrap();
    let bad_digest = format!("blake3-256:{}", "f".repeat(64));
    let mut variants = Vec::new();
    let mut digest = base.clone();
    digest["artifactDigest"] = serde_json::json!(bad_digest);
    variants.push(digest);
    let mut nonce = base.clone();
    nonce["publicationNonce"] = serde_json::json!("mismatched-nonce");
    variants.push(nonce);
    let mut unknown = base.clone();
    unknown["bundleDigest"] = serde_json::json!(format!("blake3-256:{}", "e".repeat(64)));
    variants.push(unknown);

    for variant in variants {
        let bytes = serde_json::to_vec_pretty(&variant).unwrap();
        std::fs::write(&pending_path, &bytes).unwrap();
        let guard =
            RuntimeArtifactMutationGuard::try_acquire(&state_home.join("runtime/artifacts"))
                .expect("acquire canonicalizer guard");
        canonicalize_legacy_activation_receipts_under_guard(&state_home, &guard)
            .await
            .expect_err("mismatched legacy receipt must fail closed");
        drop(guard);
        assert_eq!(std::fs::read(&pending_path).unwrap(), bytes);
        assert_eq!(slots.active_target().await.unwrap(), active_before);
        assert_eq!(slots.healthy_target().await.unwrap(), healthy_before);
    }

    let mut wrong_bundle = RuntimeArtifactActivationEvent {
        schema_id: "agent.semantic-protocols.runtime-artifact-activation".to_owned(),
        schema_version: 1,
        bundle_digest: runtime_artifact_bundle_digest(&std::collections::BTreeMap::from([(
            "asp".to_owned(),
            Blake3ContentDigest::parse(&format!("blake3-256:{}", "e".repeat(64))).unwrap(),
        )])),
        artifact_digest: current.artifact_digest.clone(),
        artifact_path: current.artifact_path.clone(),
        candidate_slot_path: current.candidate_slot_path.clone(),
        previous_artifact_digest: current.previous_artifact_digest.clone(),
        artifact_mode: current.artifact_mode.clone(),
        published_at_unix_millis: current.published_at_unix_millis,
        publication_nonce: current.publication_nonce.clone(),
        candidate_identity: current.candidate_identity.clone(),
    };
    if wrong_bundle.bundle_digest == current.bundle_digest {
        wrong_bundle.bundle_digest =
            Blake3ContentDigest::parse(&format!("blake3-256:{}", "d".repeat(64))).unwrap();
    }
    let bytes = serde_json::to_vec_pretty(&wrong_bundle).unwrap();
    std::fs::write(&pending_path, &bytes).unwrap();
    let guard = RuntimeArtifactMutationGuard::try_acquire(&state_home.join("runtime/artifacts"))
        .expect("acquire canonicalizer guard");
    let error = canonicalize_legacy_activation_receipts_under_guard(&state_home, &guard)
        .await
        .expect_err("mismatched hard-cut bundle must fail closed");
    drop(guard);
    assert!(error.contains("reasonKind=activation-receipt-content-drift"));
    assert_eq!(std::fs::read(&pending_path).unwrap(), bytes);
    assert_eq!(slots.active_target().await.unwrap(), active_before);
    assert_eq!(slots.healthy_target().await.unwrap(), healthy_before);
}

#[tokio::test]
async fn malformed_active_receipt_fails_before_lease_and_discards_prepared_candidate() {
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
        !crate::runtime_artifact_quiescence::runtime_artifact_quiescence_lease_path(&state_home)
            .exists(),
        "pre-transaction validation must not publish a quiescence lease"
    );
    let guard = RuntimeArtifactMutationGuard::try_acquire(&state_home.join("runtime/artifacts"))
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
    let active_receipt_path = active_target.join("activation.json");
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

    let error =
        commit_runtime_artifact_activation(&state_home, &pending, Some(&initial.artifact_digest))
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
    let guard = RuntimeArtifactMutationGuard::try_acquire(&state_home.join("runtime/artifacts"))
        .expect("rollback releases the mutation guard");
    drop(guard);

    std::fs::remove_dir(staged).unwrap();
    commit_runtime_artifact_activation(&state_home, &pending, Some(&initial.artifact_digest))
        .await
        .expect("the next actor commit succeeds after the conflict is removed");
}
