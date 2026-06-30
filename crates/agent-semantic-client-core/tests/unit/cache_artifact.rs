use std::path::Path;

use crate::{
    CacheArtifactId, replay_artifact_path, replay_artifacts_root, structured_evidence_artifact_path,
};

#[test]
fn replay_artifact_path_resolves_from_live_client_to_artifacts_root() {
    let cache_root = Path::new("/state/projects/by-id/repo/workspaces/ws/live/client");

    assert_eq!(
        replay_artifact_path(
            cache_root,
            &CacheArtifactId::from("search/result.json"),
            "search/",
            ".json",
        )
        .expect("artifact path"),
        Path::new("/state/projects/by-id/repo/workspaces/ws/artifacts/search/result.json")
    );
    assert_eq!(
        replay_artifacts_root(cache_root).expect("artifacts root"),
        Path::new("/state/projects/by-id/repo/workspaces/ws/artifacts")
    );
}

#[test]
fn replay_artifact_path_rejects_untrusted_ids() {
    let cache_root = Path::new("/state/projects/by-id/repo/workspaces/ws/live/client");

    for artifact_id in [
        "../search/result.json",
        "search/../result.json",
        "/search/result.json",
        "prompt-output/result.txt",
        "search/result.txt",
    ] {
        assert!(
            replay_artifact_path(
                cache_root,
                &CacheArtifactId::from(artifact_id),
                "search/",
                ".json",
            )
            .is_none(),
            "{artifact_id}"
        );
    }
}

#[test]
fn structured_evidence_artifact_path_accepts_schema_owned_families() {
    let cache_root = Path::new("/state/projects/by-id/repo/workspaces/ws/live/client");

    assert!(
        structured_evidence_artifact_path(
            cache_root,
            &CacheArtifactId::from("relation-plan/plan.json"),
        )
        .is_some()
    );
    assert!(
        structured_evidence_artifact_path(
            cache_root,
            &CacheArtifactId::from("prompt-output/source.txt"),
        )
        .is_none()
    );
}
