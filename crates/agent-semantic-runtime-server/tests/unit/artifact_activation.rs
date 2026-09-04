use agent_semantic_artifacts::runtime_artifact_publication::publish_runtime_artifact;
use std::os::unix::fs::PermissionsExt;

use super::RuntimeArtifactActivationDisposition;
use super::activate_and_acknowledge;
use super::read_runtime_artifact_activation_event;

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

#[tokio::test]
async fn developer_link_activation_is_event_bound_and_failure_preserves_slots() {
    let temporary = tempfile::tempdir().expect("temporary state");
    let state_home = temporary.path().join("state");
    let mutable_source = temporary.path().join("target/debug/asp");
    let developer_link = temporary.path().join("dev-bin/asp");
    let stable_target = temporary.path().join("bin/asp");
    tokio::fs::create_dir_all(mutable_source.parent().unwrap())
        .await
        .expect("create mutable source parent");
    tokio::fs::create_dir_all(developer_link.parent().unwrap())
        .await
        .expect("create developer link parent");
    tokio::fs::write(&mutable_source, b"#!/bin/sh\nexit 77\n")
        .await
        .expect("write mutable Developer artifact");
    std::fs::set_permissions(&mutable_source, std::fs::Permissions::from_mode(0o755))
        .expect("Developer artifact permissions");
    std::os::unix::fs::symlink(&mutable_source, &developer_link)
        .expect("create real Developer symlink");

    let profiles = state_home.join("runtime/profiles/asp");
    let old_active = temporary.path().join("artifacts/old-active/asp");
    let old_healthy = temporary.path().join("artifacts/old-healthy/asp");
    tokio::fs::create_dir_all(old_active.parent().unwrap())
        .await
        .expect("create active artifact parent");
    tokio::fs::create_dir_all(old_healthy.parent().unwrap())
        .await
        .expect("create healthy artifact parent");
    tokio::fs::write(&old_active, b"old active")
        .await
        .expect("write old active");
    tokio::fs::write(&old_healthy, b"old healthy")
        .await
        .expect("write old healthy");
    tokio::fs::create_dir_all(&profiles)
        .await
        .expect("create profiles");
    std::os::unix::fs::symlink(&old_active, profiles.join("active")).expect("seed active slot");
    std::os::unix::fs::symlink(&old_healthy, profiles.join("healthy")).expect("seed healthy slot");
    let first_publication =
        publish_runtime_artifact(&state_home, &developer_link, &stable_target, "dev")
            .await
            .expect("materialize first immutable event-bound candidate");
    let first_event =
        agent_semantic_artifacts::runtime_artifact_activation::read_runtime_artifact_activation_event(
            &state_home,
        )
        .await
        .expect("read first pending activation")
        .expect("first pending content-bound publication");
    assert_eq!(
        first_event.artifact_digest,
        first_publication.artifact_digest
    );
    agent_semantic_artifacts::runtime_artifact_activation::commit_runtime_artifact_activation(
        &state_home,
        &first_event,
        None,
    )
    .await
    .expect("commit first content-bound publication");
    let active_before = tokio::fs::read_link(profiles.join("active"))
        .await
        .expect("read active before failed newer claim");
    let healthy_before = tokio::fs::read_link(profiles.join("healthy"))
        .await
        .expect("read healthy before failed newer claim");

    let publication = publish_runtime_artifact(&state_home, &developer_link, &stable_target, "dev")
        .await
        .expect("materialize newer same-digest event-bound candidate");
    let expected_digest = publication.artifact_digest.clone();
    let event = read_runtime_artifact_activation_event(&state_home)
        .await
        .expect("read failed activation")
        .expect("failed activation exists");
    let failure = activate_and_acknowledge(
        &state_home,
        &move |event| {
            let expected_digest = expected_digest.clone();
            async move {
                assert_eq!(event.artifact_digest, expected_digest);
                let metadata = tokio::fs::symlink_metadata(&event.artifact_path)
                    .await
                    .expect("candidate metadata");
                assert!(metadata.file_type().is_file());
                assert!(!metadata.file_type().is_symlink());
                Err("candidate-server-start-failed".to_owned())
            }
        },
        event,
    )
    .await
    .expect_err("failed activation is typed and rolls back");
    assert!(failure.contains("candidate-server-start-failed"));
    assert_eq!(
        tokio::fs::read_link(profiles.join("active"))
            .await
            .expect("read active after"),
        active_before
    );
    assert_eq!(
        tokio::fs::read_link(profiles.join("healthy"))
            .await
            .expect("read healthy after"),
        healthy_before
    );
    let pending = agent_semantic_artifacts::runtime_artifact_activation::read_runtime_artifact_activation_event(
        &state_home,
    )
    .await
    .expect("read pending activation after failed claim")
    .expect("failed activation must preserve its pending publication");
    assert_eq!(pending.artifact_digest, publication.artifact_digest);
    assert_ne!(pending.publication_nonce, first_event.publication_nonce);
    assert!(pending.artifact_path.is_file());
}
