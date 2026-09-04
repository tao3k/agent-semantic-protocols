use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use super::publish_runtime_artifact;
use crate::blake3_content_digest::Blake3ContentDigest;
use crate::runtime_artifact_activation::RuntimeArtifactActivationEvent;
use crate::runtime_artifact_activation::canonicalize_legacy_activation_receipts_under_guard;
use crate::runtime_artifact_activation::commit_runtime_artifact_activation;
use crate::runtime_artifact_activation::decode_runtime_artifact_activation_event;
use crate::runtime_artifact_activation::read_runtime_artifact_activation_event;
use crate::runtime_artifact_activation::runtime_artifact_activation_event_path;
use crate::runtime_artifact_retention::RuntimeArtifactMutationGuard;
use crate::runtime_artifact_slots::RuntimeArtifactSlotAuthority;
use crate::runtime_artifact_slots::runtime_artifact_bundle_digest;
use crate::runtime_artifact_slots::runtime_artifact_candidate_digest;

fn write_executable(path: &Path, body: &str) {
    std::fs::write(path, body).expect("write fixture executable");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .expect("fixture executable permissions");
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
