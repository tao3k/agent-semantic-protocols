use super::{
    ExactProjectionKind, WorkspaceDerivedProjectionSnapshot, WorkspaceOwnerSnapshot,
    WorkspaceSelectorSnapshot, projection_validation::validate_selector, typed_digest,
};

#[test]
fn streaming_digest_preserves_the_existing_json_digest_contract() {
    let value = (
        "workspace-a",
        vec![0_u8, 1, 2, 127, 128, 254, 255],
        vec!["selector-a", "selector-b"],
    );
    let encoded = serde_json::to_vec(&value).expect("encode digest fixture");
    let expected = format!("blake3-256:{}", blake3::hash(&encoded).to_hex());

    assert_eq!(typed_digest(&value).expect("streaming digest"), expected);
}

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
            projection_kind: ExactProjectionKind::CallableSkeleton,
            bytes: b"fn f()".to_vec(),
        }],
    };

    let error = validate_selector(&owner, &selector)
        .expect_err("signature text must not satisfy the shared projection schema");
    assert!(error.contains("not shared-schema JSON"));
}
