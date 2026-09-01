//! Artifacts-package acceptance for the active/healthy retention invariant.

#[cfg(unix)]
#[tokio::test]
async fn resident_active_and_healthy_are_the_only_retained_artifacts() {
    let temporary = tempfile::tempdir().expect("temporary Runtime state");
    let runtime_root = temporary.path().join("runtime");
    let artifact_root = runtime_root.join("artifacts");
    let algorithm_root = artifact_root.join("blake3-256");
    let active_digest = "a".repeat(64);
    let healthy_digest = "b".repeat(64);
    let orphan_digest = "c".repeat(64);
    let active_artifact = algorithm_root.join(&active_digest).join("asp");
    let healthy_artifact = algorithm_root.join(&healthy_digest).join("asp");
    let orphan_artifact = algorithm_root.join(&orphan_digest).join("asp");
    for artifact in [&active_artifact, &healthy_artifact, &orphan_artifact] {
        std::fs::create_dir_all(artifact.parent().expect("artifact parent"))
            .expect("artifact directory");
        std::fs::write(artifact, b"runtime").expect("artifact bytes");
    }

    let publications = runtime_root.join("resident/publications");
    let active_publication = publications.join("active-generation");
    let healthy_publication = publications.join("healthy-generation");
    std::fs::create_dir_all(&active_publication).expect("active publication");
    std::fs::create_dir_all(&healthy_publication).expect("healthy publication");
    std::os::unix::fs::symlink(&active_artifact, active_publication.join("asp"))
        .expect("active artifact link");
    std::os::unix::fs::symlink(&healthy_artifact, healthy_publication.join("asp"))
        .expect("healthy artifact link");
    std::fs::create_dir_all(runtime_root.join("resident")).expect("resident root");
    std::os::unix::fs::symlink(&active_publication, runtime_root.join("resident/active"))
        .expect("active slot");
    std::os::unix::fs::symlink(&healthy_publication, runtime_root.join("resident/healthy"))
        .expect("healthy slot");

    agent_semantic_artifacts::runtime_artifact_retention::prune_unreachable_runtime_artifacts(
        &artifact_root,
    )
    .await
    .expect("reachability retention");

    assert!(active_artifact.exists());
    assert!(healthy_artifact.exists());
    assert!(!orphan_artifact.exists());
}

#[cfg(unix)]
#[tokio::test]
async fn resident_publication_slots_retain_at_most_active_and_healthy() {
    let temporary = tempfile::tempdir().expect("temporary resident publications");
    let resident_root = temporary.path().join("resident");
    let publications = resident_root.join("publications");
    let first = publications.join("first");
    let second = publications.join("second");
    let third = publications.join("third");
    for publication in [&first, &second, &third] {
        std::fs::create_dir_all(publication).expect("publication directory");
        std::fs::write(publication.join("asp"), b"runtime").expect("publication artifact");
    }
    let slots = agent_semantic_artifacts::runtime_artifact_slots::RuntimeArtifactSlotAuthority::new(
        &resident_root,
    );

    slots.commit_ready(&first).await.expect("initial commit");
    assert_eq!(slots.active_target().await.unwrap().unwrap(), first);
    assert_eq!(slots.healthy_target().await.unwrap().unwrap(), first);
    slots.commit_ready(&second).await.expect("second commit");
    slots.commit_ready(&third).await.expect("third commit");
    slots
        .prune_unreachable_publications()
        .await
        .expect("prune superseded publication");

    assert_eq!(slots.active_target().await.unwrap().unwrap(), third);
    assert_eq!(slots.healthy_target().await.unwrap().unwrap(), second);
    assert!(!first.exists());
    assert!(second.exists());
    assert!(third.exists());
}
