// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::{
    auxiliary_owner_applies_to_source, encode_semantic_projection, normalized_item_parser_facts,
};
use agent_semantic_client_db::runtime_server_workspace::ExactProjectionKind;
use agent_semantic_provider_transport::projection_batch::{
    ProviderProjectedItem, ProviderProjectedItemIdentity,
};

fn fixture_item() -> ProviderProjectedItem {
    ProviderProjectedItem {
        item_id: "item:target".to_owned(),
        owner_id: "owner:src/lib.rs".to_owned(),
        kind: "function".to_owned(),
        name: "target".to_owned(),
        selector: "rust://src/lib.rs#item/function/target".to_owned(),
        source_byte_start: 0,
        source_byte_end: 18,
        identity: ProviderProjectedItemIdentity {
            schema_id: "agent.semantic-protocols.canonical-item-identity".to_owned(),
            schema_version: "1".to_owned(),
            language_id: "rust".to_owned(),
            kind: "function".to_owned(),
            symbol: "target".to_owned(),
            scopes: Vec::new(),
        },
        projections: Vec::new(),
    }
}

#[test]
fn normalized_selector_fact_is_item_local_and_scales_linearly() {
    let item = fixture_item();
    let baseline = normalized_item_parser_facts(&item).expect("encode item-local parser facts");
    let selector_count = 4_096_usize;
    let encoded_bytes = (0..selector_count)
        .map(|_| normalized_item_parser_facts(&item).unwrap().len())
        .sum::<usize>();

    assert_eq!(
        baseline,
        serde_json::to_vec(&item).expect("encode fixture item")
    );
    assert!(baseline.len() < 1_024);
    assert_eq!(
        encoded_bytes,
        selector_count * baseline.len(),
        "selector proof bytes must scale with item facts only"
    );
}

#[test]
fn auxiliary_context_is_limited_to_source_ancestors() {
    assert!(auxiliary_owner_applies_to_source(
        "Cargo.toml",
        "crates/core/src/lib.rs"
    ));
    assert!(auxiliary_owner_applies_to_source(
        "crates/core/Cargo.toml",
        "crates/core/src/lib.rs"
    ));
    assert!(!auxiliary_owner_applies_to_source(
        "crates/other/Cargo.toml",
        "crates/core/src/lib.rs"
    ));
}

#[test]
fn callable_projection_digest_is_derived_from_the_typed_v1_payload() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../schemas/fixtures/semantic-projection.callable-skeleton.v1.json"
    ))
    .expect("decode semantic projection fixture");
    let field = |name: &str| {
        fixture[name]
            .as_str()
            .unwrap_or_else(|| panic!("semantic projection fixture is missing {name}"))
    };

    let bytes = encode_semantic_projection(
        ExactProjectionKind::CallableSkeleton,
        field("languageId"),
        field("providerId"),
        field("rootSelector"),
        field("evidenceContextRef"),
        &fixture["payload"],
    )
    .expect("encode typed callable-skeleton projection");
    let envelope = serde_json::from_slice::<
        agent_semantic_content_identity::semantic_projection::SemanticProjection<
            agent_semantic_content_identity::callable_skeleton_projection::CallableSkeletonPayload,
        >,
    >(&bytes)
    .expect("decode typed callable-skeleton projection envelope");

    envelope
        .validate()
        .expect("typed callable-skeleton payload digest must validate");
    assert_eq!(
        envelope.payload_schema_id,
        "agent.semantic-protocols.callable-skeleton"
    );
}
