use std::path::{Path, PathBuf};

use super::{
    WorkspaceSearchIdentityErrorV1, WorkspaceSearchIdentityInputV1, WorkspaceSearchIdentityV1,
    WorkspaceSearchScopeKindV1, resolve_workspace_member_root_v1,
};

fn package_input() -> WorkspaceSearchIdentityInputV1 {
    WorkspaceSearchIdentityInputV1 {
        language_id: "rust".to_string(),
        provider_id: "rs-harness".to_string(),
        scope_kind: WorkspaceSearchScopeKindV1::Package,
        requested_discovery_root: PathBuf::from("/repo/crates/client-db"),
        cargo_workspace_root: PathBuf::from("/repo"),
        selected_package_root: Some(PathBuf::from("/repo/crates/client-db")),
        provider_tool_root: PathBuf::from("/providers/rs-harness"),
        envelope_root: PathBuf::from("/repo/crates/client-db"),
        root_count: 1,
        owner_count: 84,
        leaf_count: 84,
        selector_count: 512,
        source_snapshot_root_digest: [7; 32],
    }
}

#[test]
fn package_scope_keeps_provider_tool_root_separate() {
    let identity = WorkspaceSearchIdentityV1::admit(package_input()).expect("admitted");
    assert_eq!(
        identity.provider_tool_root(),
        Path::new("/providers/rs-harness")
    );
    assert_eq!(
        identity.selected_package_root(),
        Some(Path::new("/repo/crates/client-db"))
    );
}

#[test]
fn broader_workspace_selection_is_rejected_for_package_scope() {
    let mut input = package_input();
    input.selected_package_root = Some(PathBuf::from("/repo"));
    let error = WorkspaceSearchIdentityV1::admit(input).expect_err("scope mismatch");
    assert!(matches!(
        error,
        WorkspaceSearchIdentityErrorV1::PackageScopeMismatch { .. }
    ));
}

#[test]
fn empty_owner_envelope_is_not_a_frontier() {
    let mut input = package_input();
    input.root_count = 0;
    input.owner_count = 0;
    input.leaf_count = 0;
    input.selector_count = 0;
    let error = WorkspaceSearchIdentityV1::admit(input).expect_err("empty envelope");
    assert_eq!(
        error,
        WorkspaceSearchIdentityErrorV1::IncompleteOwnerEnvelope {
            root_count: 0,
            owner_count: 0,
            leaf_count: 0,
            selector_count: 0,
        }
    );
}

#[test]
fn workspace_scope_has_no_selected_package() {
    let input = WorkspaceSearchIdentityInputV1 {
        language_id: "rust".to_string(),
        provider_id: "rs-harness".to_string(),
        scope_kind: WorkspaceSearchScopeKindV1::Workspace,
        requested_discovery_root: PathBuf::from("/repo"),
        cargo_workspace_root: PathBuf::from("/repo"),
        selected_package_root: None,
        provider_tool_root: PathBuf::from("/providers/rs-harness"),
        envelope_root: PathBuf::from("/repo"),
        root_count: 1,
        owner_count: 84,
        leaf_count: 84,
        selector_count: 512,
        source_snapshot_root_digest: [7; 32],
    };
    WorkspaceSearchIdentityV1::admit(input).expect("workspace admitted");
}

#[test]
fn absolute_package_root_is_not_joined_to_workspace_again() {
    let workspace = Path::new("/Users/guangtao/ghq/github.com/tao3k/repo");
    let package =
        Path::new("/Users/guangtao/ghq/github.com/tao3k/repo/languages/rust-lang-project-harness");
    assert_eq!(
        resolve_workspace_member_root_v1(workspace, package).expect("absolute package"),
        package
    );
}

#[test]
fn relative_package_root_is_resolved_once() {
    let workspace = Path::new("/repo");
    assert_eq!(
        resolve_workspace_member_root_v1(
            workspace,
            Path::new("languages/rust-lang-project-harness"),
        )
        .expect("relative package"),
        PathBuf::from("/repo/languages/rust-lang-project-harness")
    );
}

#[test]
fn relative_package_root_cannot_escape_workspace() {
    let error = resolve_workspace_member_root_v1(Path::new("/repo"), Path::new("../other"))
        .expect_err("workspace escape");
    assert!(matches!(
        error,
        WorkspaceSearchIdentityErrorV1::PackageRootOutsideWorkspace { .. }
    ));
}
