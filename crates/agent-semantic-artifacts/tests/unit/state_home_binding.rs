//! State Home binding tests.

use std::path::Path;

use crate::HostProjectReference;
use crate::ProjectBinding;

#[test]
fn host_reference_is_optional_and_never_becomes_workspace_identity() {
    let root = Path::new("/tmp/project-binding-fixture");
    let projectless = ProjectBinding::resolve(None, "git-remote:example/repo", root).unwrap();
    let host_bound = ProjectBinding::resolve(
        Some(HostProjectReference {
            platform: "codex".to_string(),
            project_id: "local-host-id".to_string(),
            project_kind: "local".to_string(),
            host_id: Some("local".to_string()),
        }),
        "git-remote:example/repo",
        root,
    )
    .unwrap();

    assert_eq!(projectless.repo, host_bound.repo);
    assert_eq!(projectless.workspace, host_bound.workspace);
    assert_ne!(projectless.binding_digest, host_bound.binding_digest);
    projectless.validate().unwrap();
    host_bound.validate().unwrap();
}

#[test]
fn full_content_identity_is_not_a_truncated_path_token() {
    let binding = ProjectBinding::resolve(None, "repo", "/tmp/workspace").unwrap();
    assert_eq!(binding.repo.digest.as_str().len(), "blake3-256:".len() + 64);
    assert_eq!(
        binding.workspace.digest.as_str().len(),
        "blake3-256:".len() + 64
    );
    assert_eq!(
        binding.binding_digest.as_str().len(),
        "blake3-256:".len() + 64
    );
}
