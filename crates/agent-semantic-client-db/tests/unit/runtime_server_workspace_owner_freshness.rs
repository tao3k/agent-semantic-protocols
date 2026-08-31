use super::resident_owner_projection_is_complete;
use crate::runtime_server_workspace::{
    WorkspaceDerivedProjectionSnapshot, WorkspaceOwnerSnapshot, WorkspaceSelectorSnapshot,
};

fn owner(selectors: Vec<WorkspaceSelectorSnapshot>) -> WorkspaceOwnerSnapshot {
    WorkspaceOwnerSnapshot {
        authority: None,
        owner_path: "src/lib.rs".to_owned(),
        content_digest: format!("blake3-256:{}", "a".repeat(64)),
        bytes: b"fn run() {}\n".to_vec(),
        selectors,
    }
}

#[test]
fn unchanged_owner_is_warm_after_selector_discovery_without_derived_projection() {
    assert!(!resident_owner_projection_is_complete(&owner(Vec::new())));
    assert!(resident_owner_projection_is_complete(&owner(vec![
        WorkspaceSelectorSnapshot {
            selector: "rust://src/lib.rs#item/function/run".to_owned(),
            byte_start: 0,
            byte_end: 11,
            query_keys: Vec::new(),
            derived_projections: Vec::new(),
        },
    ])));
    assert!(resident_owner_projection_is_complete(&owner(vec![
        WorkspaceSelectorSnapshot {
            selector: "rust://src/lib.rs#item/function/run".to_owned(),
            byte_start: 0,
            byte_end: 11,
            query_keys: Vec::new(),
            derived_projections: vec![WorkspaceDerivedProjectionSnapshot {
                projection_kind: "callable-skeleton".to_owned(),
                bytes: b"fn run()".to_vec(),
                evidence_context: None,
            }],
        },
    ])));
}
