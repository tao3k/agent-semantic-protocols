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
            evidence_context: None,
        }],
    };

    let error = validate_selector(&owner, &selector)
        .expect_err("signature text must not satisfy the shared projection schema");
    assert!(error.contains("not shared-schema JSON"));
}

#[test]
fn referenced_projection_requires_runtime_evidence_context() {
    let (owner, selector) = callable_fixture();
    let error = validate_selector(&owner, &selector)
        .expect_err("a reference without its Runtime catalog context must fail closed");
    assert!(error.contains("missing its evidence context"));
}

#[test]
fn referenced_projection_rejects_context_identity_drift() {
    let (owner, mut selector) = callable_fixture();
    let projection = &mut selector.derived_projections[0];
    let mut context =
        agent_semantic_content_identity::projection_evidence_context::ProjectionEvidenceContext {
            schema_id: "agent.semantic-protocols.projection-evidence-context".to_owned(),
            schema_version: "1".to_owned(),
            evidence_context_ref: format!("blake3-256:{}", "e".repeat(64)),
            language_id: "rust".to_owned(),
            provider_id: "asp-rust".to_owned(),
            generation_identity_digest: format!("blake3-256:{}", "0".repeat(64)),
            parser_identity_digest: format!("blake3-256:{}", "1".repeat(64)),
            query_pack_digest: format!("blake3-256:{}", "2".repeat(64)),
        };
    context.provider_id = "asp-other".to_owned();
    projection.evidence_context = Some(context);

    let error = validate_selector(&owner, &selector)
        .expect_err("a catalog context whose content no longer matches its ref must fail closed");
    assert!(error.contains("evidence context failed validation"));
}

#[test]
fn inline_projection_rejects_runtime_only_context() {
    let (owner, mut selector) = callable_fixture();
    let projection = &mut selector.derived_projections[0];
    projection.evidence_context = Some(
        agent_semantic_content_identity::projection_evidence_context::ProjectionEvidenceContext {
            schema_id: "agent.semantic-protocols.projection-evidence-context".to_owned(),
            schema_version: "1".to_owned(),
            evidence_context_ref: format!("blake3-256:{}", "e".repeat(64)),
            language_id: "rust".to_owned(),
            provider_id: "asp-rust".to_owned(),
            generation_identity_digest: format!("blake3-256:{}", "0".repeat(64)),
            parser_identity_digest: format!("blake3-256:{}", "1".repeat(64)),
            query_pack_digest: format!("blake3-256:{}", "2".repeat(64)),
        },
    );

    let error = validate_selector(&owner, &selector)
        .expect_err("provider input cannot smuggle a Runtime-only catalog context");
    assert!(error.contains("unexpectedly carries an evidence context"));
}

fn callable_fixture() -> (WorkspaceOwnerSnapshot, WorkspaceSelectorSnapshot) {
    let owner_path = "src/lib.rs";
    let structural_selector = "rust://src/lib.rs#item/function/run";
    let bytes = b"fn run() {}".to_vec();
    let digest = "0".repeat(64);
    let projection = WorkspaceDerivedProjectionSnapshot {
        projection_kind: ExactProjectionKind::CallableSkeleton,
        bytes: serde_json::to_vec(&serde_json::json!({
            "schemaId": format!(
                "agent.semantic-protocols.callable-skeleton-{}projection",
                ""
            ),
            "schemaVersion": "1",
            "projectionKind": "callable-skeleton",
            "languageId": "rust",
            "providerId": "asp-rust",
            "rootSelector": {
                "schemaId": "asp.exact-structural-selector.v1",
                "schemaVersion": "1",
                "languageId": "rust",
                "ownerPath": owner_path,
                "selector": structural_selector,
                "generationIdentityDigest": digest,
                "parserIdentityDigest": "1".repeat(64),
                "queryPackDigest": "2".repeat(64),
                "rootItemSelector": {
                    "schemaId": "asp.canonical-item-selector.v1",
                    "schemaVersion": "1",
                    "languageId": "rust",
                    "kind": "function",
                    "symbol": "run",
                    "scopes": [],
                    "structuralSelector": structural_selector
                },
                "segments": []
            },
            "rootNodeId": "callable:root",
            "callable": { "kind": "function", "displayName": "run", "signature": "run" },
            "nodes": [{
                "nodeId": "callable:root",
                "kind": "callable",
                "label": "run",
                "order": 0,
                "queryable": false
            }],
            "relations": [],
            "cost": { "sourceBytes": 0, "projectedBytes": 0, "omittedBytes": 0 }
        }))
        .expect("callable fixture bytes"),
        evidence_context: None,
    };
    let selector = WorkspaceSelectorSnapshot {
        selector: structural_selector.to_owned(),
        byte_start: 0,
        byte_end: bytes.len(),
        derived_projections: vec![projection],
    };
    let owner = WorkspaceOwnerSnapshot {
        owner_path: owner_path.to_owned(),
        content_digest: "blake3-256:fixture".to_owned(),
        bytes,
        selectors: Vec::new(),
    };
    (owner, selector)
}
