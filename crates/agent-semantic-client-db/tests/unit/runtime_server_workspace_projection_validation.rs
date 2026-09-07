// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::{
    ExactProjectionKind, WorkspaceOwnerSnapshot, WorkspaceSelectorSnapshot,
    projection_validation::validate_selector, typed_digest,
};
use crate::runtime_server_workspace::WorkspaceDerivedProjectionSnapshot;

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
fn selector_owner_validation_uses_the_shared_canonical_owner_codec() {
    let owner = WorkspaceOwnerSnapshot {
        authority: None,
        owner_path: "src/genport#.scm".to_owned(),
        content_digest: "blake3-256:fixture".to_owned(),
        native_syntax_diagnostic: None,
        bytes: b"(defstruct genport ())".to_vec(),
        selectors: Vec::new(),
    };
    let selector = WorkspaceSelectorSnapshot {
        selector: "gerbil-scheme://src/genport%23.scm#item/type/genport".to_owned(),
        byte_start: 0,
        byte_end: owner.bytes.len(),
        query_keys: Vec::new(),
        derived_projections: Vec::new(),
    };

    validate_selector(&owner, &selector).expect("encoded owner path must match decoded owner");

    let raw_selector = WorkspaceSelectorSnapshot {
        selector: "gerbil-scheme://src/genport#.scm#item/type/genport".to_owned(),
        ..selector
    };
    let error = validate_selector(&owner, &raw_selector)
        .expect_err("an unescaped owner delimiter must fail canonical validation");
    assert!(error.contains("not canonical"));
}

#[test]
fn signature_text_cannot_masquerade_as_callable_skeleton_json() {
    let owner = WorkspaceOwnerSnapshot {
        authority: None,
        owner_path: "src/lib.rs".to_owned(),
        content_digest: "blake3-256:unused-by-selector-validation".to_owned(),
        native_syntax_diagnostic: None,
        bytes: b"fn f() {}".to_vec(),
        selectors: Vec::new(),
    };
    let selector = WorkspaceSelectorSnapshot {
        selector: "rust://src/lib.rs#item/function/f".to_owned(),
        byte_start: 0,
        byte_end: owner.bytes.len(),
        query_keys: Vec::new(),
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
    assert!(
        error.contains("missing its evidence context"),
        "error={error}"
    );
}

#[test]
fn referenced_projection_rejects_context_identity_drift() {
    let (owner, mut selector) = callable_fixture();
    let projection = &mut selector.derived_projections[0];
    let mut context =
        agent_semantic_content_identity::projection_evidence_context::ProjectionEvidenceContext {
            schema_id: "agent.semantic-protocols.projection-evidence-context".to_owned(),
            schema_version: "1".to_owned(),
            evidence_context_ref: fixture_evidence_context_ref().into(),
            language_id: "rust".into(),
            provider_id: "asp-rust".into(),
            generation_identity_digest: "0".repeat(64),
            parser_identity_digest: "1".repeat(64),
            query_pack_digest: "2".repeat(64),
        };
    context.provider_id = "asp-other".into();
    projection.evidence_context = Some(context);

    let error = validate_selector(&owner, &selector)
        .expect_err("a catalog context whose content no longer matches its ref must fail closed");
    assert!(
        error.contains("evidence context failed validation"),
        "error={error}"
    );
}

#[test]
fn runtime_projection_accepts_its_bound_evidence_context() {
    let (owner, mut selector) = callable_fixture();
    let projection = &mut selector.derived_projections[0];
    projection.evidence_context = Some(
        agent_semantic_content_identity::projection_evidence_context::ProjectionEvidenceContext {
            schema_id: "agent.semantic-protocols.projection-evidence-context".to_owned(),
            schema_version: "1".to_owned(),
            evidence_context_ref: fixture_evidence_context_ref().into(),
            language_id: "rust".into(),
            provider_id: "asp-rust".into(),
            generation_identity_digest: "0".repeat(64),
            parser_identity_digest: "1".repeat(64),
            query_pack_digest: "2".repeat(64),
        },
    );

    validate_selector(&owner, &selector)
        .expect("Runtime-materialized projection must retain its bound evidence context");
}

fn callable_fixture() -> (WorkspaceOwnerSnapshot, WorkspaceSelectorSnapshot) {
    let owner_path = "src/lib.rs";
    let structural_selector = "rust://src/lib.rs#item/function/run";
    let bytes = b"fn run() {}".to_vec();
    let payload = serde_json::json!({
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
    });
    let typed_payload: agent_semantic_content_identity::callable_skeleton_projection::CallableSkeletonPayload =
        serde_json::from_value(payload.clone()).expect("decode typed callable payload");
    let payload_digest = format!(
        "blake3-256:{}",
        blake3::hash(
            &serde_json::to_vec(&typed_payload).expect("canonical callable payload bytes"),
        )
        .to_hex()
    );
    let projection = WorkspaceDerivedProjectionSnapshot {
        projection_kind: ExactProjectionKind::CallableSkeleton,
        bytes: serde_json::to_vec(&serde_json::json!({
            "schemaId": "agent.semantic-protocols.semantic-projection",
            "schemaVersion": "1",
            "projectionKind": "callable-skeleton",
            "languageId": "rust",
            "providerId": "asp-rust",
            "rootSelector": structural_selector,
            "evidenceContextRef": fixture_evidence_context_ref(),
            "payloadSchemaId": "agent.semantic-protocols.callable-skeleton",
            "payloadDigest": payload_digest,
            "payload": payload
        }))
        .expect("callable fixture bytes"),
        evidence_context: None,
    };
    let selector = WorkspaceSelectorSnapshot {
        selector: structural_selector.to_owned(),
        byte_start: 0,
        byte_end: bytes.len(),
        query_keys: Vec::new(),
        derived_projections: vec![projection],
    };
    let owner = WorkspaceOwnerSnapshot {
        authority: None,
        owner_path: owner_path.to_owned(),
        content_digest: "blake3-256:fixture".to_owned(),
        native_syntax_diagnostic: None,
        bytes,
        selectors: Vec::new(),
    };
    (owner, selector)
}

fn fixture_evidence_context_ref() -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"asp.projection-evidence-context.v1\0");
    for component in [
        "rust".to_owned(),
        "asp-rust".to_owned(),
        "0".repeat(64),
        "1".repeat(64),
        "2".repeat(64),
    ] {
        hasher.update(&(component.len() as u64).to_le_bytes());
        hasher.update(component.as_bytes());
    }
    format!("blake3-256:{}", hasher.finalize().to_hex())
}
