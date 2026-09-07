// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use super::RuntimeArtifactBundleMemberSource;
use super::publish_runtime_artifact;
use super::publish_runtime_artifact_bound_bundle_members;
use super::publish_runtime_artifact_bound_provider_member_from_active;
use super::publish_runtime_artifact_bundle;
use super::publish_runtime_artifact_bundle_member_from_active;
use super::publish_runtime_artifact_bundle_members;
use super::publish_runtime_artifact_bundle_successor_from_active;
use super::publish_runtime_artifact_with_before_guard;
use crate::blake3_content_digest::Blake3ContentDigest;
use crate::runtime_artifact_activation::RuntimeArtifactActivationEvent;
use crate::runtime_artifact_activation::RuntimeArtifactCandidateIdentityReceipt;
use crate::runtime_artifact_activation::commit_runtime_artifact_activation;
use crate::runtime_artifact_activation::decode_runtime_artifact_activation_event;
use crate::runtime_artifact_activation::publish_applied_runtime_artifact_activation;
use crate::runtime_artifact_activation::read_applied_runtime_artifact_activation_event;
use crate::runtime_artifact_activation::read_runtime_artifact_activation_event;
use crate::runtime_artifact_activation::rollback_runtime_artifact_activation;
use crate::runtime_artifact_activation::runtime_artifact_activation_event_path;
use crate::runtime_artifact_quiescence::prepare_runtime_artifact_quiescence_lease;
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
        RuntimeArtifactSlotAuthority::for_artifact(&state_home.join("runtime/artifacts"), "asp");
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
    let hook_target = state_home.join("runtime/artifacts/active/asp-hook");
    write_executable(&asp_source, "#!/bin/sh\nexit 0\n");
    write_executable(&hook_source, "#!/bin/sh\nexit 0\n");

    let publication =
        publish_runtime_artifact_bundle(&state_home, &asp_source, &asp_target, "dev", &hook_source)
            .await
            .expect("publish one immutable Runtime/Hook bundle");

    let slots = RuntimeArtifactSlotAuthority::new(state_home.join("runtime/artifacts"));
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
    let mut unexpected_generation: serde_json::Value =
        serde_json::from_slice(&pending_bytes).unwrap();
    unexpected_generation["activationGeneration"] = serde_json::json!(54);
    assert!(
        decode_runtime_artifact_activation_event(
            &serde_json::to_vec(&unexpected_generation).unwrap(),
            "unexpected numeric generation fixture",
        )
        .is_ok()
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
async fn optional_capability_is_an_exact_member_of_the_same_active_bundle() {
    let temporary = tempfile::tempdir().expect("optional bundle member fixture");
    let state_home = temporary.path().join("state");
    let asp_source = temporary.path().join("asp");
    let hook_source = temporary.path().join("asp-hook");
    let graphs_source = temporary.path().join("asp-python-graphs");
    let asp_target = state_home.join("runtime/bin/asp");
    write_executable(&asp_source, "#!/bin/sh\nexit 0\n");
    write_executable(&hook_source, "#!/bin/sh\nexit 0\n");
    write_executable(&graphs_source, "#!/bin/sh\nexit 0\n");
    let members = [
        RuntimeArtifactBundleMemberSource {
            name: "asp-hook",
            source: &hook_source,
        },
        RuntimeArtifactBundleMemberSource {
            name: "asp-python-graphs",
            source: &graphs_source,
        },
    ];

    publish_runtime_artifact_bundle_members(&state_home, &asp_source, &asp_target, "dev", &members)
        .await
        .expect("publish one bundle with optional graph capability");

    let slots = RuntimeArtifactSlotAuthority::new(state_home.join("runtime/artifacts"));
    let active = slots.active_target().await.unwrap().expect("active bundle");
    assert_eq!(
        std::fs::read(active.join("asp-python-graphs")).unwrap(),
        std::fs::read(&graphs_source).unwrap()
    );
    assert!(!state_home.join("runtime/bin/asp-python-graphs").exists());
    crate::runtime_artifact_slots::verify_runtime_artifact_bundle(&active)
        .await
        .expect("content-proven bundle");
}

#[tokio::test]
async fn provider_refresh_republishes_the_complete_active_generation() {
    let temporary = tempfile::tempdir().expect("provider refresh bundle fixture");
    let state_home = temporary.path().join("state");
    let asp_source = temporary.path().join("asp");
    let hook_source = temporary.path().join("asp-hook");
    let first_provider = temporary.path().join("asp-rust-first");
    let next_provider = temporary.path().join("asp-rust-next");
    let asp_target = state_home.join("runtime/bin/asp");
    write_executable(&asp_source, "#!/bin/sh\nexit 0\n");
    write_executable(&hook_source, "#!/bin/sh\nexit 1\n");
    write_executable(&first_provider, "#!/bin/sh\nexit 2\n");
    write_executable(&next_provider, "#!/bin/sh\nexit 3\n");

    let members = [
        RuntimeArtifactBundleMemberSource {
            name: "asp-hook",
            source: &hook_source,
        },
        RuntimeArtifactBundleMemberSource {
            name: "asp-rust",
            source: &first_provider,
        },
    ];
    publish_runtime_artifact_bundle_members(&state_home, &asp_source, &asp_target, "dev", &members)
        .await
        .expect("publish initial complete Runtime generation");
    let slots = RuntimeArtifactSlotAuthority::new(state_home.join("runtime/artifacts"));
    let first_active = slots.active_target().await.unwrap().unwrap();

    publish_runtime_artifact_bundle_member_from_active(
        &state_home,
        "asp-rust",
        &next_provider,
        "dev",
    )
    .await
    .expect("replace provider through the complete generation authority");

    let next_active = slots.active_target().await.unwrap().unwrap();
    assert_ne!(next_active, first_active);
    assert_eq!(
        std::fs::read(next_active.join("asp")).unwrap(),
        std::fs::read(&asp_source).unwrap()
    );
    assert_eq!(
        std::fs::read(next_active.join("asp-hook")).unwrap(),
        std::fs::read(&hook_source).unwrap()
    );
    assert_eq!(
        std::fs::read(next_active.join("asp-rust")).unwrap(),
        std::fs::read(&next_provider).unwrap()
    );
    assert_eq!(
        std::fs::canonicalize(state_home.join("runtime/bin/asp-rust")).unwrap(),
        std::fs::canonicalize(next_active.join("asp-rust")).unwrap()
    );
    assert!(!state_home.join("runtime/artifacts/blake3-256").exists());
    assert!(!state_home.join("runtime/artifacts/bundles").exists());
}

#[tokio::test]
async fn bound_provider_refresh_regenerates_registration_and_artifact_closure_atomically() {
    use crate::runtime_artifact_execution_closure::{
        LanguageSchemaClosureEntry, NamedRuntimeDigestClosureEntry, RuntimeArtifactExecutionClosure,
    };

    let temporary = tempfile::tempdir().expect("bound provider refresh fixture");
    let state_home = temporary.path().join("state");
    let sources = temporary.path().join("sources");
    std::fs::create_dir_all(&sources).expect("source root");
    let asp_source = sources.join("asp");
    let hook_source = sources.join("asp-hook");
    let rust_source = sources.join("asp-rust");
    write_executable(&asp_source, "#!/bin/sh\nexit 0\n");
    write_executable(&hook_source, "#!/bin/sh\nexit 1\n");
    write_executable(&rust_source, "#!/bin/sh\nexit 2\n");
    let base_members = std::collections::BTreeMap::from([
        (
            "asp".to_owned(),
            runtime_artifact_candidate_digest(&asp_source)
                .await
                .unwrap(),
        ),
        (
            "asp-hook".to_owned(),
            runtime_artifact_candidate_digest(&hook_source)
                .await
                .unwrap(),
        ),
    ]);
    let closure = RuntimeArtifactExecutionClosure::from_runtime_bundle_members(
        &base_members,
        vec![NamedRuntimeDigestClosureEntry {
            id: "query-admission".into(),
            digest: Blake3ContentDigest::from_bytes(b"policy"),
        }],
        vec![NamedRuntimeDigestClosureEntry {
            id: "query-playbook-v1".into(),
            digest: Blake3ContentDigest::from_bytes(b"abi"),
        }],
        vec![LanguageSchemaClosureEntry {
            language_id: "rust".into(),
            schema_digest: Blake3ContentDigest::from_bytes(b"schemas"),
        }],
    )
    .expect("bootstrap closure");
    let closure_sources = closure
        .materialized_members()
        .unwrap()
        .into_iter()
        .map(|(name, bytes)| {
            let path = sources.join(name);
            std::fs::write(&path, bytes).unwrap();
            (name, path)
        })
        .collect::<Vec<_>>();
    let mut publication_members = vec![RuntimeArtifactBundleMemberSource {
        name: "asp-hook",
        source: &hook_source,
    }];
    publication_members.extend(
        closure_sources
            .iter()
            .map(|(name, path)| RuntimeArtifactBundleMemberSource { name, source: path }),
    );
    publish_runtime_artifact_bound_bundle_members(
        &state_home,
        &asp_source,
        &state_home.join("runtime/bin/asp"),
        "dev",
        &publication_members,
        &closure.binding().unwrap(),
    )
    .await
    .expect("publish empty-provider bound bundle");

    publish_runtime_artifact_bound_provider_member_from_active(
        &state_home,
        "asp-rust",
        &rust_source,
        "dev",
    )
    .await
    .expect("publish provider and rebound closure");

    let active = RuntimeArtifactSlotAuthority::new(state_home.join("runtime/artifacts"))
        .active_target()
        .await
        .unwrap()
        .unwrap();
    let verified = crate::runtime_artifact_slots::verify_runtime_artifact_bound_bundle(&active)
        .await
        .expect("strict successor admission");
    let closure =
        RuntimeArtifactExecutionClosure::from_materialized_members(&active, verified.members())
            .expect("successor closure");
    assert_eq!(closure.provider_registration.entries.len(), 1);
    assert_eq!(closure.provider_artifact_set.entries.len(), 1);
    assert_eq!(
        closure.provider_artifact_set.entries[0].artifact_digest,
        *verified.member_digest("asp-rust").expect("Rust member")
    );
}

#[tokio::test]
async fn repeated_bundle_publication_moves_only_the_shared_active_selector() {
    let temporary = tempfile::tempdir().expect("fixed bundle launcher fixture");
    let state_home = temporary.path().join("state");
    let first_asp = temporary.path().join("asp-first");
    let first_hook = temporary.path().join("asp-hook-first");
    let next_asp = temporary.path().join("asp-next");
    let next_hook = temporary.path().join("asp-hook-next");
    let asp_target = state_home.join("runtime/bin/asp");
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
    let slots = RuntimeArtifactSlotAuthority::new(state_home.join("runtime/artifacts"));
    let healthy_before = slots.healthy_target().await.unwrap();

    publish_runtime_artifact_bundle(&state_home, &next_asp, &asp_target, "dev", &next_hook)
        .await
        .expect("publish next bundle while Runtime is absent");

    let active_after = slots.active_target().await.unwrap();
    assert_ne!(active_after, healthy_before);
    assert_eq!(slots.healthy_target().await.unwrap(), healthy_before);
    assert_eq!(std::fs::read_link(&asp_target).unwrap(), fixed_asp_launcher);
}

#[tokio::test]
async fn binary_refresh_preserves_providers_and_health_commit_prunes_the_predecessor() {
    let temporary = tempfile::tempdir().expect("binary refresh fixture");
    let state_home = temporary.path().join("state");
    let first_asp = temporary.path().join("asp-first");
    let first_hook = temporary.path().join("asp-hook-first");
    let provider = temporary.path().join("asp-rust");
    let next_asp = temporary.path().join("asp-next");
    let next_hook = temporary.path().join("asp-hook-next");
    let target = state_home.join("runtime/bin/asp");
    write_executable(&first_asp, "#!/bin/sh\nexit 10\n");
    write_executable(&first_hook, "#!/bin/sh\nexit 11\n");
    write_executable(&provider, "#!/bin/sh\nexit 12\n");
    write_executable(&next_asp, "#!/bin/sh\nexit 20\n");
    write_executable(&next_hook, "#!/bin/sh\nexit 21\n");

    publish_runtime_artifact_bundle_members(
        &state_home,
        &first_asp,
        &target,
        "release",
        &[
            RuntimeArtifactBundleMemberSource {
                name: "asp-hook",
                source: &first_hook,
            },
            RuntimeArtifactBundleMemberSource {
                name: "asp-rust",
                source: &provider,
            },
        ],
    )
    .await
    .expect("publish initial complete generation");
    let first = read_runtime_artifact_activation_event(&state_home)
        .await
        .unwrap()
        .unwrap();
    commit_runtime_artifact_activation(&state_home, &first, None)
        .await
        .expect("qualify initial generation");

    publish_runtime_artifact_bundle_successor_from_active(
        &state_home,
        &next_asp,
        &target,
        "release",
        &[RuntimeArtifactBundleMemberSource {
            name: "asp-hook",
            source: &next_hook,
        }],
    )
    .await
    .expect("publish binary refresh successor");
    let pending = read_runtime_artifact_activation_event(&state_home)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        std::fs::read(pending.candidate_slot_path.join("asp-rust")).unwrap(),
        std::fs::read(&provider).unwrap(),
        "binary refresh must preserve the admitted provider member"
    );
    let generations = state_home.join("runtime/artifacts/generations");
    assert_eq!(std::fs::read_dir(&generations).unwrap().count(), 2);

    commit_runtime_artifact_activation(&state_home, &pending, Some(&first.artifact_digest))
        .await
        .expect("qualify successor generation");
    let slots = RuntimeArtifactSlotAuthority::new(state_home.join("runtime/artifacts"));
    assert_eq!(
        slots.active_target().await.unwrap(),
        slots.healthy_target().await.unwrap()
    );
    assert_eq!(
        std::fs::read_dir(&generations).unwrap().count(),
        1,
        "health commit must retire the predecessor generation"
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
