// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_artifacts::runtime_artifact_publication::publish_runtime_artifact;
use std::os::unix::fs::PermissionsExt;

use super::RuntimeArtifactActivationDisposition;
use super::activate_and_acknowledge;
use super::read_runtime_artifact_activation_event;

#[test]
fn runtime_artifact_activation_has_no_socket_authority() {
    let runtime_actor = include_str!("../../src/artifact_activation.rs");
    let artifact_event =
        include_str!("../../../agent-semantic-artifacts/src/runtime_artifact_activation.rs");
    let artifact_publication =
        include_str!("../../../agent-semantic-artifacts/src/runtime_artifact_publication.rs");

    for (owner, source) in [
        ("runtime actor", runtime_actor),
        ("artifact event", artifact_event),
        ("artifact publication", artifact_publication),
    ] {
        assert!(
            !source.contains("runtime_artifact_activation_socket_path")
                && !source.contains("asp-activation")
                && !source.contains("UnixDatagram"),
            "{owner} retains a second socket activation authority"
        );
    }
}

#[tokio::test]
async fn resident_actor_reconciles_an_online_durable_publication() {
    let temporary = tempfile::tempdir().expect("online activation fixture");
    let state_home = temporary.path().join("state");
    let source = temporary.path().join("asp");
    let target = temporary.path().join("bin/asp");
    tokio::fs::write(&source, b"#!/bin/sh\nexit 0\n")
        .await
        .expect("write Runtime artifact");
    std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o755))
        .expect("Runtime artifact permissions");
    let activation_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed_count = activation_count.clone();
    let actor = super::spawn_runtime_artifact_activation_actor(state_home.clone(), move |_event| {
        let observed_count = observed_count.clone();
        async move {
            observed_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(RuntimeArtifactActivationDisposition::Committed)
        }
    })
    .await
    .expect("mount resident activation reconciler");
    let mut receipts = actor.receipts();

    publish_runtime_artifact(&state_home, &source, &target, "dev")
        .await
        .expect("publish durable activation event");
    tokio::time::timeout(std::time::Duration::from_secs(2), receipts.changed())
        .await
        .expect("online activation terminal deadline")
        .expect("online activation terminal channel");
    let terminal = receipts
        .borrow()
        .clone()
        .expect("online activation terminal");
    assert_eq!(terminal.state, "ready");
    assert_eq!(
        activation_count.load(std::sync::atomic::Ordering::SeqCst),
        1
    );
    actor
        .shutdown()
        .await
        .expect("shutdown activation reconciler");
}

#[tokio::test]
async fn daemon_activation_transaction_consumes_startup_and_online_publications() {
    let temporary = tempfile::tempdir().expect("temporary state");
    let state_home = temporary.path().join("state");
    let source = temporary.path().join("asp");
    let target = temporary.path().join("bin/asp");
    tokio::fs::write(&source, b"#!/bin/sh\nexit 77\n")
        .await
        .expect("write artifact");
    std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o755))
        .expect("artifact permissions");

    let startup_publication = publish_runtime_artifact(&state_home, &source, &target, "dev")
        .await
        .expect("publication succeeds without Runtime actor");
    let startup_event = read_runtime_artifact_activation_event(&state_home)
        .await
        .expect("read startup activation")
        .expect("startup activation exists");
    let startup_terminal = activate_and_acknowledge(
        &state_home,
        &|_event| async { Ok(RuntimeArtifactActivationDisposition::Committed) },
        startup_event,
    )
    .await
    .expect("commit startup activation");
    assert_eq!(startup_terminal.state, "ready");
    assert_eq!(
        startup_terminal.artifact_digest,
        startup_publication.artifact_digest
    );

    tokio::fs::write(&source, b"#!/bin/sh\nexit 78\n")
        .await
        .expect("update artifact");
    let online_publication = publish_runtime_artifact(&state_home, &source, &target, "dev")
        .await
        .expect("online publication");
    let online_event = read_runtime_artifact_activation_event(&state_home)
        .await
        .expect("read online activation")
        .expect("online activation exists");
    let online_terminal = activate_and_acknowledge(
        &state_home,
        &|_event| async { Ok(RuntimeArtifactActivationDisposition::Committed) },
        online_event,
    )
    .await
    .expect("commit online activation");
    assert_eq!(online_terminal.state, "ready");
    assert_eq!(
        online_terminal.artifact_digest,
        online_publication.artifact_digest
    );
    assert!(
        read_runtime_artifact_activation_event(&state_home)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn successor_required_preserves_pending_without_acknowledge_or_rollback() {
    let temporary = tempfile::tempdir().expect("temporary state");
    let state_home = temporary.path().join("state");
    let source = temporary.path().join("asp");
    let target = temporary.path().join("bin/asp");
    tokio::fs::write(&source, b"successor")
        .await
        .expect("write successor artifact");
    std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o755))
        .expect("successor permissions");
    let publication = publish_runtime_artifact(&state_home, &source, &target, "dev")
        .await
        .expect("publish pending successor");
    let event = read_runtime_artifact_activation_event(&state_home)
        .await
        .expect("read pending successor")
        .expect("pending successor exists");

    let receipt = activate_and_acknowledge(
        &state_home,
        &|_event| async { Ok(RuntimeArtifactActivationDisposition::SuccessorRequired) },
        event.clone(),
    )
    .await
    .expect("successor-required is a typed non-mutating terminal");

    assert_eq!(receipt.state, "successor-required");
    assert_eq!(receipt.artifact_digest, publication.artifact_digest);
    let pending = read_runtime_artifact_activation_event(&state_home)
        .await
        .expect("read preserved pending successor")
        .expect("successor-required preserves pending");
    assert_eq!(pending.publication_nonce, event.publication_nonce);
    assert!(
        agent_semantic_artifacts::runtime_artifact_activation::
            read_applied_runtime_artifact_activation_event(&state_home)
            .await
            .expect("read applied activation")
            .is_none(),
        "successor-required must not forge an applied activation"
    );
}
