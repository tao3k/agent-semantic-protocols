// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::{
    auxiliary_owner_applies_to_source, encode_semantic_projection, normalized_item_parser_facts,
    parser_auxiliary_input_digest_for_owner, parser_auxiliary_input_identities,
};
use agent_semantic_client_db::runtime_server_workspace::ExactProjectionKind;
use agent_semantic_provider_transport::projection_batch::{
    ProviderProjectedItem, ProviderProjectedItemIdentity, ProviderProjectionOwner,
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
fn unrelated_auxiliary_change_preserves_other_owner_artifact_identity() {
    let inputs = |a_manifest: &[u8]| {
        parser_auxiliary_input_identities(&[
            ProviderProjectionOwner {
                owner_path: "Cargo.toml".to_owned(),
                source_leaf_digest: "root".to_owned(),
                source_bytes: b"[workspace]".to_vec(),
            },
            ProviderProjectionOwner {
                owner_path: "crates/a/Cargo.toml".to_owned(),
                source_leaf_digest: "a".to_owned(),
                source_bytes: a_manifest.to_vec(),
            },
        ])
        .expect("canonical auxiliary identities")
    };
    let before = inputs(b"[package]\nname='a'");
    let after = inputs(b"[package]\nname='a2'");

    assert_ne!(
        parser_auxiliary_input_digest_for_owner(&before, "crates/a/src/lib.rs").unwrap(),
        parser_auxiliary_input_digest_for_owner(&after, "crates/a/src/lib.rs").unwrap(),
        "an applicable auxiliary change must invalidate its dependent owner"
    );
    assert_eq!(
        parser_auxiliary_input_digest_for_owner(&before, "crates/b/src/lib.rs").unwrap(),
        parser_auxiliary_input_digest_for_owner(&after, "crates/b/src/lib.rs").unwrap(),
        "an unrelated nested auxiliary must not invalidate another owner"
    );
}

#[test]
fn parser_artifact_content_reuse_is_scenario_measured() {
    use asp_rust_project_harness_policy::{
        AspRustProjectHarnessScenarioObservation, PARSER_ARTIFACT_CONTENT_REUSE_SCENARIO_ID,
        asp_search_scenario_package, measure_asp_rust_scenario,
        render_asp_rust_scenario_benchmark_toml,
    };

    let auxiliary = |a_manifest: &[u8]| {
        vec![
            ProviderProjectionOwner {
                owner_path: "Cargo.toml".to_owned(),
                source_leaf_digest: "root".to_owned(),
                source_bytes: b"[workspace]".to_vec(),
            },
            ProviderProjectionOwner {
                owner_path: "crates/a/Cargo.toml".to_owned(),
                source_leaf_digest: "a".to_owned(),
                source_bytes: a_manifest.to_vec(),
            },
        ]
    };
    let after = parser_auxiliary_input_identities(&auxiliary(b"[package]\nname='a2'"))
        .expect("changed auxiliary identities");
    let scenario = asp_search_scenario_package()
        .scenarios
        .into_iter()
        .find(|scenario| scenario.name == PARSER_ARTIFACT_CONTENT_REUSE_SCENARIO_ID)
        .expect("parser artifact content reuse Scenario");
    let measurement = measure_asp_rust_scenario(&scenario, || {
        let hash_started = std::time::Instant::now();
        let before = parser_auxiliary_input_identities(&auxiliary(b"[package]\nname='a'"))
            .expect("baseline auxiliary identities");
        let hash_elapsed = hash_started.elapsed();

        let cut_started = std::time::Instant::now();
        let a_before =
            parser_auxiliary_input_digest_for_owner(&before, "crates/a/src/lib.rs").unwrap();
        let a_after =
            parser_auxiliary_input_digest_for_owner(&after, "crates/a/src/lib.rs").unwrap();
        let b_before =
            parser_auxiliary_input_digest_for_owner(&before, "crates/b/src/lib.rs").unwrap();
        let b_after =
            parser_auxiliary_input_digest_for_owner(&after, "crates/b/src/lib.rs").unwrap();
        let cut_elapsed = cut_started.elapsed();
        assert_ne!(a_before, a_after);
        assert_eq!(b_before, b_after);

        AspRustProjectHarnessScenarioObservation::default()
            .with_timing("hash_auxiliary_once", hash_elapsed)
            .with_timing("derive_owner_cuts", cut_elapsed)
            .with_metric("auxiliary_owner_hash_count", 2)
            .with_metric("affected_owner_count", 1)
            .with_metric("unrelated_owner_invalidation_count", 0)
            .with_metric("provider_process_count", 0)
    })
    .expect("measure parser artifact content reuse Scenario");
    let rendered = render_asp_rust_scenario_benchmark_toml(&scenario, &measurement)
        .expect("render parser artifact content reuse benchmark");
    assert!(rendered.contains("[metrics.unrelated_owner_invalidation_count]"));
    assert!(rendered.contains("observed = 0"));
    println!("{rendered}");
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
