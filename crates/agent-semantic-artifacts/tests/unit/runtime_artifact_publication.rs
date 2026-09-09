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
async fn bundle_publication_replaces_a_legacy_regular_provider_launcher() {
    let temporary = tempfile::tempdir().expect("legacy provider launcher fixture");
    let state_home = temporary.path().join("state");
    let source_root = temporary.path().join("sources");
    std::fs::create_dir_all(state_home.join("runtime/bin")).expect("runtime bin directory");
    std::fs::create_dir_all(&source_root).expect("source directory");
    let asp_source = source_root.join("asp");
    let hook_source = source_root.join("asp-hook");
    let rust_source = source_root.join("asp-rust");
    let asp_target = state_home.join("runtime/bin/asp");
    let legacy_rust_launcher = state_home.join("runtime/bin/asp-rust");
    write_executable(&asp_source, "#!/bin/sh\nexit 0\n");
    write_executable(&hook_source, "#!/bin/sh\nexit 1\n");
    write_executable(&rust_source, "#!/bin/sh\nexit 2\n");
    write_executable(&legacy_rust_launcher, "#!/bin/sh\nexit 99\n");

    publish_runtime_artifact_bundle(&state_home, &asp_source, &asp_target, "dev", &hook_source)
        .await
        .expect("publish predecessor Runtime/Hook bundle");
    publish_runtime_artifact_bundle_member_from_active(
        &state_home,
        "asp-rust",
        &rust_source,
        "dev",
    )
    .await
    .expect("migrate legacy provider launcher through provider successor publication");

    assert!(
        std::fs::symlink_metadata(&legacy_rust_launcher)
            .expect("provider launcher metadata")
            .file_type()
            .is_symlink(),
        "legacy direct executable must not remain a second launcher authority"
    );
    assert_eq!(
        std::fs::canonicalize(&legacy_rust_launcher).unwrap(),
        std::fs::canonicalize(
            RuntimeArtifactSlotAuthority::new(state_home.join("runtime/artifacts"))
                .active_target()
                .await
                .unwrap()
                .unwrap()
                .join("asp-rust")
        )
        .unwrap()
    );
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

#[path = "runtime_artifact_publication_transactions.rs"]
mod transaction_tests;
