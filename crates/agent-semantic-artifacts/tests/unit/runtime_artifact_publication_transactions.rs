// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::os::unix::fs::PermissionsExt;

use super::{
    Blake3ContentDigest, RuntimeArtifactActivationEvent, RuntimeArtifactCandidateIdentityReceipt,
    RuntimeArtifactMutationGuard, RuntimeArtifactSlotAuthority, commit_runtime_artifact_activation,
    prepare_runtime_artifact_quiescence_lease, publish_applied_runtime_artifact_activation,
    publish_runtime_artifact, publish_runtime_artifact_with_before_guard,
    read_applied_runtime_artifact_activation_event, read_runtime_artifact_activation_event,
    rollback_runtime_artifact_activation, runtime_artifact_activation_event_path,
    runtime_artifact_bundle_digest, runtime_artifact_candidate_digest, write_executable,
};
use crate::runtime_artifact_publication::PredecessorReceiptAdmission;

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
    let slots = RuntimeArtifactSlotAuthority::new(state_home.join("runtime/artifacts"));
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
    assert!(
        !candidate.candidate_slot_path.exists(),
        "failed candidate must be retired after rollback"
    );

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
    publish_runtime_artifact(&state_home, &candidate_source, &target, "dev")
        .await
        .expect("a retired rollback receipt must not block a new publication");
    let republished = read_runtime_artifact_activation_event(&state_home)
        .await
        .unwrap()
        .expect("new pending publication replaces the retired rollback marker");
    assert!(republished.candidate_slot_path.exists());
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
        .join("runtime/artifacts/generations")
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
        RuntimeArtifactSlotAuthority::for_artifact(state_home.join("runtime/artifacts"), "asp");
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
        activation_generation: 2,
        schema_id: "agent.semantic-protocols.runtime-artifact-activation".to_owned(),
        schema_version: 1,
        bundle_digest: new_bundle_digest,
        artifact_digest: new_serving_digest.clone(),
        artifact_path: new_prepared.path.clone(),
        candidate_slot_path: new_candidate_dir.clone(),
        previous_artifact_digest: Some(old.artifact_digest.clone()),
        artifact_mode: "release".to_owned(),
        published_at_unix_millis: old.published_at_unix_millis + 1,
        publication_nonce: new_nonce.clone(),
        candidate_identity: RuntimeArtifactCandidateIdentityReceipt {
            artifact_digest: new_serving_digest.clone(),
            artifact_path: new_prepared.path,
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
            &[],
            None,
            None,
            PredecessorReceiptAdmission::Strict,
            &[],
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
async fn publication_repairs_an_empty_selector_directory_inside_the_artifact_transaction() {
    let temporary = tempfile::tempdir().expect("publication repair fixture");
    let state_home = temporary.path().join("state");
    let source = temporary.path().join("asp");
    let target = state_home.join("bin/asp");
    write_executable(&source, "#!/bin/sh\nexit 0\n");
    let active = state_home.join("runtime/artifacts/active");
    std::fs::create_dir_all(&active).expect("empty invalid selector directory");

    let receipt = publish_runtime_artifact(&state_home, &source, &target, "release")
        .await
        .expect("Artifacts publication repairs its own empty selector");

    let metadata = std::fs::symlink_metadata(&active).expect("active selector metadata");
    assert!(metadata.file_type().is_symlink());
    assert_eq!(
        std::fs::canonicalize(active).expect("active generation"),
        std::fs::canonicalize(
            state_home
                .join("runtime/artifacts/generations")
                .join(receipt.bundle_digest.content_digest().as_str())
        )
        .expect("published generation")
    );
}

#[tokio::test]
async fn publication_rejects_a_populated_selector_directory_without_deleting_it() {
    let temporary = tempfile::tempdir().expect("publication conflict fixture");
    let state_home = temporary.path().join("state");
    let source = temporary.path().join("asp");
    let target = state_home.join("bin/asp");
    write_executable(&source, "#!/bin/sh\nexit 0\n");
    let active = state_home.join("runtime/artifacts/active");
    std::fs::create_dir_all(&active).expect("invalid selector directory");
    std::fs::write(active.join("do-not-delete"), b"owned").expect("conflict marker");

    let error = publish_runtime_artifact(&state_home, &source, &target, "release")
        .await
        .expect_err("populated selector directory must fail closed");

    assert!(error.contains("artifact-selector-directory-conflict"));
    assert_eq!(
        std::fs::read(active.join("do-not-delete")).unwrap(),
        b"owned"
    );
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
        RuntimeArtifactSlotAuthority::for_artifact(&state_home.join("runtime/artifacts"), "asp");
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
    assert!(malformed.contains("reasonKind=activation-receipt-invalid"));
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
    let slots = RuntimeArtifactSlotAuthority::new(state_home.join("runtime/artifacts"));
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
