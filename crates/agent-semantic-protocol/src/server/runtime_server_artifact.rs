use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimeServerArtifactAction {
    Status,
    Restart,
}

pub(crate) fn runtime_server_artifact_action(
    canonical_artifact: &Path,
    running_artifact: &Path,
    canonical_digest: &str,
    running_digest: &str,
) -> RuntimeServerArtifactAction {
    if canonical_artifact == running_artifact && canonical_digest == running_digest {
        RuntimeServerArtifactAction::Status
    } else {
        RuntimeServerArtifactAction::Restart
    }
}
