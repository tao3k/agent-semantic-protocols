#[path = "../../src/server/runtime_server_artifact.rs"]
mod implementation;

use implementation::{RuntimeServerArtifactAction, runtime_server_artifact_action};
use std::path::Path;

#[test]
fn same_path_and_digest_uses_status() {
    assert_eq!(
        runtime_server_artifact_action(
            Path::new("/runtime/asp"),
            Path::new("/runtime/asp"),
            "digest-current",
            "digest-current",
        ),
        RuntimeServerArtifactAction::Status
    );
}

#[test]
fn same_path_with_changed_digest_requires_restart() {
    assert_eq!(
        runtime_server_artifact_action(
            Path::new("/runtime/asp"),
            Path::new("/runtime/asp"),
            "digest-new",
            "digest-running",
        ),
        RuntimeServerArtifactAction::Restart
    );
}

#[test]
fn changed_path_requires_restart() {
    assert_eq!(
        runtime_server_artifact_action(
            Path::new("/runtime/asp"),
            Path::new("/legacy/asp"),
            "digest-current",
            "digest-current",
        ),
        RuntimeServerArtifactAction::Restart
    );
}
