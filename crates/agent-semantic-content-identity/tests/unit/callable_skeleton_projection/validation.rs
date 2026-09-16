// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::collections::BTreeMap;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

use super::CALLABLE_SKELETON_PAYLOAD_SCHEMA_ID;
use super::CallableDescriptorKindV1;
use super::CallableDescriptorV1;
use super::CallableSkeletonCostV1;
use super::CallableSkeletonNodeKindV1;
use super::CallableSkeletonNodeV1;
use super::CallableSkeletonPayload;
use super::CallableSkeletonRelationKindV1;
use super::CallableSkeletonRelationV1;
use super::CallableSkeletonValidationError;
use crate::exact_structural_selector::EXACT_STRUCTURAL_SELECTOR_SCHEMA_ID;
use crate::exact_structural_selector::EXACT_STRUCTURAL_SELECTOR_SCHEMA_VERSION;
use crate::exact_structural_selector::ExactCanonicalItemSelectorV1;
use crate::exact_structural_selector::ExactStructuralSelectorV1;
use crate::semantic_projection::SemanticProjection;
use crate::semantic_projection::SemanticProjectionInput;

fn root_selector() -> ExactStructuralSelectorV1 {
    ExactStructuralSelectorV1 {
        schema_id: EXACT_STRUCTURAL_SELECTOR_SCHEMA_ID.to_owned(),
        schema_version: EXACT_STRUCTURAL_SELECTOR_SCHEMA_VERSION.to_owned(),
        language_id: "rust".to_owned(),
        owner_path: "crates/example/src/lib.rs".to_owned(),
        selector: "rust://crates/example/src/lib.rs#node/function/run/identity=root".to_owned(),
        generation_identity_digest: "a".repeat(64),
        parser_identity_digest: "b".repeat(64),
        query_pack_digest: "c".repeat(64),
        root_item_selector: ExactCanonicalItemSelectorV1 {
            schema_id: "asp.canonical-item-selector.v1".to_owned(),
            schema_version: "1".to_owned(),
            language_id: "rust".to_owned(),
            kind: "function".to_owned(),
            symbol: "run".to_owned(),
            scopes: Vec::new(),
            structural_selector: "rust://crates/example/src/lib.rs#item/function/run".to_owned(),
        },
        segments: Vec::new(),
    }
}

#[test]
fn public_projection_mode_matches_the_wire_projection_kind() {
    assert_eq!(CallableSkeletonPayload::projection_mode(), "skeleton");
    assert_eq!(
        CallableSkeletonPayload::projection_kind(),
        "callable-skeleton"
    );
}

#[test]
fn shared_json_schema_keeps_wire_identity_at_version_one() {
    let schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../schemas/callable-skeleton.schema.json"
    ))
    .expect("callable skeleton shared schema must be valid JSON");
    assert!(schema["properties"].get("projectionKind").is_none());
    assert_eq!(
        schema["required"],
        serde_json::json!([
            "rootSelector",
            "rootNodeId",
            "callable",
            "nodes",
            "relations",
            "cost"
        ])
    );
}

fn projection() -> CallableSkeletonPayload {
    let root = root_selector();
    let arm_selector = format!("{}/segment/arm/pattern-command-rust", root.selector);
    CallableSkeletonPayload {
        root_selector: root.selector.clone(),
        root_node_id: "callable:run".to_owned(),
        callable: CallableDescriptorV1 {
            kind: CallableDescriptorKindV1::new("function").expect("callable kind"),
            display_name: "run".to_owned(),
            signature: "fn run(command: Command) -> Result<Receipt>".to_owned(),
        },
        nodes: vec![
            CallableSkeletonNodeV1 {
                node_id: "callable:run".to_owned(),
                kind: CallableSkeletonNodeKindV1::Callable,
                label: "run".to_owned(),
                order: 0,
                queryable: false,
                selector: None,
                selector_ref: None,
                source_locator_hint: None,
                language_facts: BTreeMap::new(),
            },
            CallableSkeletonNodeV1 {
                node_id: "arm:rust".to_owned(),
                kind: CallableSkeletonNodeKindV1::Arm,
                label: "Command::Rust".to_owned(),
                order: 1,
                queryable: true,
                selector: Some(arm_selector),
                selector_ref: None,
                source_locator_hint: None,
                language_facts: BTreeMap::new(),
            },
        ],
        relations: vec![CallableSkeletonRelationV1 {
            from_node_id: "callable:run".to_owned(),
            to_node_id: "arm:rust".to_owned(),
            kind: CallableSkeletonRelationKindV1::new("contains").expect("relation kind"),
        }],
        cost: CallableSkeletonCostV1 {
            source_bytes: 100,
            projected_bytes: 40,
            omitted_bytes: 60,
            estimated_source_tokens: None,
            estimated_projected_tokens: None,
            token_estimator: None,
        },
        omission_reasons: Vec::new(),
        language_facts: BTreeMap::new(),
    }
}

#[test]
fn validates_queryable_child_round_trip_contract() {
    projection().validate().expect("projection should be valid");
}

#[test]
fn rejects_queryable_node_without_selector() {
    let mut value = projection();
    value.nodes[1].selector = None;
    assert!(matches!(
        value.validate(),
        Err(CallableSkeletonValidationError::MissingChildSelector(node)) if node == "arm:rust"
    ));
}

#[test]
fn accepts_packet_scoped_queryable_selector() {
    let mut value = projection();
    value.nodes[1].selector = Some(format!(
        "{}/segment/arm/ordinal-1",
        root_selector().selector
    ));
    value
        .validate()
        .expect("packet-scoped selector should be valid");
}

#[test]
fn runtime_reference_interns_authority_and_uses_packet_local_selectors() {
    let mut value = projection();
    value.nodes[0].queryable = true;
    let root = root_selector().selector.to_owned();
    value.nodes[0].selector = Some(root.clone());
    let inline_bytes = serde_json::to_vec(&value)
        .expect("encode inline projection")
        .len();

    let envelope = SemanticProjection::new(SemanticProjectionInput {
        projection_kind: CallableSkeletonPayload::projection_kind().into(),
        language_id: "rust".into(),
        provider_id: "asp-rust".into(),
        root_selector: root.into(),
        evidence_context_ref:
            "blake3-256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".into(),
        payload_schema_id: CALLABLE_SKELETON_PAYLOAD_SCHEMA_ID.into(),
        payload: value,
    })
    .expect("semantic projection envelope");
    envelope.validate().expect("validate semantic projection");
    assert_eq!(
        envelope.payload.nodes[0].selector.as_deref(),
        Some(envelope.root_selector.as_str())
    );
    assert!(
        envelope.payload.nodes[1].selector.as_deref().is_some_and(
            |selector| selector.starts_with(&format!("{}/segment/", envelope.root_selector))
        )
    );
    assert!(
        serde_json::to_vec(&envelope)
            .expect("encode projection")
            .len()
            > inline_bytes
    );
}

#[test]
fn rejects_packet_scoped_selector_outside_root() {
    let mut value = projection();
    value.nodes[1].selector = Some("rust://other.rs#item/function/run".to_owned());
    assert!(matches!(
        value.validate_scope(&root_selector().selector),
        Err(CallableSkeletonValidationError::ChildSelectorContext(node)) if node == "arm:rust"
    ));
}

#[test]
fn projected_packet_may_be_larger_than_source() {
    let mut value = projection();
    value.cost.projected_bytes = 140;
    value.cost.omitted_bytes = 0;
    value
        .validate()
        .expect("complete packet accounting should saturate omitted bytes");
}

#[test]
fn encodes_validated_payload_for_exact_selector_packet() {
    let value = projection();
    assert_eq!(CallableSkeletonPayload::projection_mode(), "skeleton");
    let encoded = value.encode_payload_base64().expect("encoded payload");
    let decoded = STANDARD.decode(encoded).expect("base64 payload");
    let round_trip: CallableSkeletonPayload =
        serde_json::from_slice(&decoded).expect("projection JSON");
    assert_eq!(round_trip, value);
}
