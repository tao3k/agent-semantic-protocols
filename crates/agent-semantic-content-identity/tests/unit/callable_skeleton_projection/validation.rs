use std::collections::BTreeMap;

use base64::{Engine as _, engine::general_purpose::STANDARD};

use super::{
    CALLABLE_SKELETON_PROJECTION_SCHEMA_ID, CALLABLE_SKELETON_PROJECTION_SCHEMA_VERSION,
    CallableDescriptorV1, CallableSkeletonCostV1, CallableSkeletonNodeKindV1,
    CallableSkeletonNodeV1, CallableSkeletonProjectionV1, CallableSkeletonRelationV1,
    CallableSkeletonValidationError,
};
use crate::exact_structural_selector::{
    CanonicalItemSelector, EXACT_STRUCTURAL_SELECTOR_SCHEMA_ID,
    EXACT_STRUCTURAL_SELECTOR_SCHEMA_VERSION, ExactStructuralSelectorSegmentV1,
    ExactStructuralSelectorV1,
};

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
        root_item_selector: CanonicalItemSelector {
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
    assert_eq!(CallableSkeletonProjectionV1::projection_mode(), "skeleton");
    assert_eq!(
        CallableSkeletonProjectionV1::projection_kind(),
        "callable-skeleton"
    );
}

#[test]
fn shared_json_schema_keeps_wire_identity_at_version_one() {
    let schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../schemas/callable-skeleton-projection.v1.schema.json"
    ))
    .expect("callable skeleton shared schema must be valid JSON");
    assert_eq!(
        schema["properties"]["schemaId"]["const"],
        CALLABLE_SKELETON_PROJECTION_SCHEMA_ID
    );
    assert_eq!(
        schema["properties"]["schemaVersion"]["const"],
        CALLABLE_SKELETON_PROJECTION_SCHEMA_VERSION
    );
    assert_eq!(
        schema["properties"]["projectionKind"]["const"],
        CallableSkeletonProjectionV1::projection_kind()
    );
}

fn projection() -> CallableSkeletonProjectionV1 {
    let root = root_selector();
    let mut arm_selector = root.clone();
    arm_selector.selector =
        "rust://crates/example/src/lib.rs#node/function/run/arm/identity=rust".to_owned();
    arm_selector
        .segments
        .push(ExactStructuralSelectorSegmentV1 {
            relation: "contains".to_owned(),
            kind: "arm".to_owned(),
            identity: "pattern-command-rust".to_owned(),
            label: Some("Command::Rust".to_owned()),
        });
    CallableSkeletonProjectionV1 {
        schema_id: CALLABLE_SKELETON_PROJECTION_SCHEMA_ID.to_owned(),
        schema_version: CALLABLE_SKELETON_PROJECTION_SCHEMA_VERSION.to_owned(),
        projection_kind: "callable-skeleton".to_owned(),
        language_id: "rust".to_owned(),
        provider_id: "rs-harness".to_owned(),
        root_selector: root,
        root_node_id: "callable:run".to_owned(),
        callable: CallableDescriptorV1 {
            kind: "function".to_owned(),
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
                exact_selector: None,
                source_locator_hint: None,
                language_facts: BTreeMap::new(),
            },
            CallableSkeletonNodeV1 {
                node_id: "arm:rust".to_owned(),
                kind: CallableSkeletonNodeKindV1::Arm,
                label: "Command::Rust".to_owned(),
                order: 1,
                queryable: true,
                exact_selector: Some(arm_selector),
                source_locator_hint: None,
                language_facts: BTreeMap::new(),
            },
        ],
        relations: vec![CallableSkeletonRelationV1 {
            from_node_id: "callable:run".to_owned(),
            to_node_id: "arm:rust".to_owned(),
            kind: "contains".to_owned(),
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
fn rejects_queryable_node_without_exact_selector() {
    let mut value = projection();
    value.nodes[1].exact_selector = None;
    assert!(matches!(
        value.validate(),
        Err(CallableSkeletonValidationError::MissingChildSelector(node)) if node == "arm:rust"
    ));
}

#[test]
fn rejects_child_selector_from_another_generation() {
    let mut value = projection();
    value.nodes[1]
        .exact_selector
        .as_mut()
        .expect("child selector")
        .generation_identity_digest = "d".repeat(64);
    assert!(matches!(
        value.validate(),
        Err(CallableSkeletonValidationError::ChildSelectorContext(node)) if node == "arm:rust"
    ));
}

#[test]
fn encodes_validated_payload_for_exact_selector_packet() {
    let value = projection();
    assert_eq!(CallableSkeletonProjectionV1::projection_mode(), "skeleton");
    let encoded = value.encode_payload_base64().expect("encoded payload");
    let decoded = STANDARD.decode(encoded).expect("base64 payload");
    let round_trip: CallableSkeletonProjectionV1 =
        serde_json::from_slice(&decoded).expect("projection JSON");
    assert_eq!(round_trip, value);
}
