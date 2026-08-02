use super::{
    WorkspaceDerivedProjectionSnapshot, WorkspaceOwnerSnapshot, WorkspaceSelectorSnapshot,
    validate_selector,
};

#[test]
fn signature_text_cannot_masquerade_as_callable_skeleton_json() {
    let owner = WorkspaceOwnerSnapshot {
        owner_path: "src/lib.rs".to_owned(),
        content_digest: "blake3-256:unused-by-selector-validation".to_owned(),
        bytes: b"fn f() {}".to_vec(),
        selectors: Vec::new(),
    };
    let selector = WorkspaceSelectorSnapshot {
        selector: "rust://src/lib.rs#item/function/f".to_owned(),
        byte_start: 0,
        byte_end: owner.bytes.len(),
        derived_projections: vec![WorkspaceDerivedProjectionSnapshot {
            projection_kind: "callable-skeleton".to_owned(),
            bytes: b"fn f()".to_vec(),
        }],
    };

    let error = validate_selector(&owner, &selector)
        .expect_err("signature text must not satisfy the shared projection schema");
    assert!(error.contains("not shared-schema JSON"));
}
