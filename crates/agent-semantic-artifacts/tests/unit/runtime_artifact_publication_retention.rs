use std::collections::BTreeSet;

use super::publish_runtime_artifact;
use crate::runtime_artifact_retention::RuntimeArtifactCandidatePreparationLease;
use crate::runtime_artifact_retention::prune_unreachable_runtime_artifacts;

#[tokio::test]
async fn canonical_publication_retains_only_active_and_healthy_artifact_digests() {
    let temp = tempfile::tempdir().expect("create temporary State Home");
    let state_home = temp.path();
    let target = state_home.join("runtime/bin/asp");

    let first_source = state_home.join("asp-first");
    let second_source = state_home.join("asp-second");
    let third_source = state_home.join("asp-third");
    std::fs::write(&first_source, b"first-runtime").expect("write first Runtime candidate");
    std::fs::write(&second_source, b"second-runtime").expect("write second Runtime candidate");
    std::fs::write(&third_source, b"third-runtime").expect("write third Runtime candidate");

    let first = publish_runtime_artifact(state_home, &first_source, &target, "test")
        .await
        .expect("publish first Runtime candidate");
    let second = publish_runtime_artifact(state_home, &second_source, &target, "test")
        .await
        .expect("publish second Runtime candidate");
    let third = publish_runtime_artifact(state_home, &third_source, &target, "test")
        .await
        .expect("publish third Runtime candidate");

    let artifact_root = state_home.join("runtime/artifacts/blake3-256");
    let retained = std::fs::read_dir(&artifact_root)
        .expect("read immutable Runtime artifact store")
        .map(|entry| {
            entry
                .expect("read immutable Runtime artifact entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect::<BTreeSet<_>>();
    let expected = BTreeSet::from([
        first.artifact_digest.content_digest().as_str().to_owned(),
        third.artifact_digest.content_digest().as_str().to_owned(),
    ]);

    assert_eq!(retained, expected);
    assert!(
        !artifact_root
            .join(second.artifact_digest.content_digest().as_str())
            .exists()
    );

    let resident_candidates = state_home.join("runtime/artifacts/bundles/asp");
    let retained_candidates = std::fs::read_dir(&resident_candidates)
        .expect("read resident Runtime candidates")
        .count();
    assert_eq!(retained_candidates, 2);
}

#[tokio::test]
async fn live_preparation_lease_preserves_candidate_and_dead_owner_is_reclaimed() {
    let temp = tempfile::tempdir().expect("create temporary State Home");
    let state_home = temp.path();
    let digest = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let candidate = state_home
        .join("runtime/artifacts/bundles/asp")
        .join(digest);
    let lease = RuntimeArtifactCandidatePreparationLease::acquire(state_home, "asp", digest)
        .expect("acquire candidate preparation lease");
    std::fs::create_dir_all(&candidate).expect("create in-flight candidate");
    std::fs::write(candidate.join("bundle.json"), b"in-flight")
        .expect("write in-flight candidate marker");

    let artifact_root = state_home.join("runtime/artifacts");
    let live = prune_unreachable_runtime_artifacts(&artifact_root)
        .await
        .expect("retain candidate owned by live publisher");
    assert_eq!(live.scanned_candidate_count, 1);
    assert_eq!(live.removed_candidate_count, 0);
    assert!(candidate.is_dir());

    drop(lease);
    let dead = prune_unreachable_runtime_artifacts(&artifact_root)
        .await
        .expect("reclaim candidate after producer lease release");
    assert_eq!(dead.scanned_candidate_count, 1);
    assert_eq!(dead.removed_candidate_count, 1);
    assert!(!candidate.exists());
    assert!(
        !state_home
            .join("runtime/artifacts/leases/candidates/asp")
            .join(format!("{digest}.lock"))
            .exists()
    );
}
