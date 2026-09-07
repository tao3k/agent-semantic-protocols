// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::{
    compile_graph_relation_pattern, selector_matches_tantivy_terms,
    smallest_selector_overlapping_line, source_line_ranges, syntax_owner_scope,
};

#[test]
fn syntax_stage_uses_the_union_of_prior_acquisition_owners() {
    let receipt =
        |axis, candidate_owners: &[&str]| agent_semantic_search::WorkspaceSearchClauseReceipt {
            axis,
            block_index: 0,
            priority_rank: 0,
            candidate_owners: candidate_owners
                .iter()
                .map(|owner| (*owner).to_owned())
                .collect(),
            complete: true,
            coverage_complete: true,
            truncated: false,
        };
    assert!(syntax_owner_scope(&[]).is_none());
    assert_eq!(
        syntax_owner_scope(&[
            receipt(
                agent_semantic_search::WorkspaceSearchAxisKind::Rg,
                &["src/a.rs", "src/shared.rs"],
            ),
            receipt(
                agent_semantic_search::WorkspaceSearchAxisKind::Tantivy,
                &["src/b.rs", "src/shared.rs"],
            ),
        ])
        .expect("prior acquisition establishes a bounded syntax scope"),
        ["src/a.rs", "src/b.rs", "src/shared.rs"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    );
}

#[test]
fn gql_adapter_uses_compiler_ir_for_the_admitted_relation_slice() {
    let Ok(()) = compile_graph_relation_pattern(
        "gql",
        "MATCH (h:Host)-[:PUBLISHES]->(e:RuntimeEndpoint) RETURN h, e",
    ) else {
        panic!("expected compiled GQL relation pattern");
    };
}

#[test]
fn gql_adapter_rejects_semantics_the_executor_does_not_implement() {
    let Err(super::AspClientOperationError::Message(error)) = compile_graph_relation_pattern(
        "gql",
        "MATCH (h:Host)-[:PUBLISHES]->(e:RuntimeEndpoint) WHERE h = 1 RETURN e",
    ) else {
        panic!("filtered GQL must fail closed");
    };
    assert!(error.contains("unfiltered MATCH"), "{error}");
}

#[test]
fn pgql_uses_the_same_v1_relation_ir_boundary() {
    let Ok(()) = compile_graph_relation_pattern(
        "pgql",
        "MATCH (h:Host)-[:PUBLISHES]->(e:RuntimeEndpoint) RETURN h, e",
    ) else {
        panic!("expected PGQL relation pattern to share the V1 IR boundary");
    };
}

#[test]
fn rg_line_maps_to_the_smallest_parser_owned_enclosing_selector() {
    let source = b"mod outer {\n    fn exact() {}\n}\n";
    let ranges = source_line_ranges(source);
    let projection = agent_semantic_search::NativeSyntaxProjection {
        owner_path: "src/lib.rs".to_owned(),
        content_digest: format!("blake3-256:{}", blake3::hash(source).to_hex()),
        selectors: vec![
            agent_semantic_search::NativeSyntaxSelector {
                selector: "rust://src/lib.rs#item/module/outer".to_owned(),
                byte_start: 0,
                byte_end: source.len(),
                query_keys: vec!["outer".to_owned()],
                derived_projection_digest: format!("blake3-256:{}", "a".repeat(64)),
            },
            agent_semantic_search::NativeSyntaxSelector {
                selector: "rust://src/lib.rs#item/function/exact".to_owned(),
                byte_start: ranges[1].0 + 4,
                byte_end: ranges[1].1,
                query_keys: vec!["exact".to_owned()],
                derived_projection_digest: format!("blake3-256:{}", "b".repeat(64)),
            },
        ],
    };
    assert_eq!(
        smallest_selector_overlapping_line(&projection, ranges[1].0, ranges[1].1)
            .expect("function selector encloses the rg match")
            .selector,
        "rust://src/lib.rs#item/function/exact"
    );
}

#[test]
fn tantivy_terms_map_only_to_parser_selectors_with_matching_query_keys() {
    let selector = agent_semantic_search::NativeSyntaxSelector {
        selector: "rust://src/lib.rs#item/function/RuntimeServingEndpoint".to_owned(),
        byte_start: 0,
        byte_end: 10,
        query_keys: vec![
            "runtime".to_owned(),
            "serving".to_owned(),
            "endpoint".to_owned(),
        ],
        derived_projection_digest: format!("blake3-256:{}", "a".repeat(64)),
    };
    assert!(selector_matches_tantivy_terms(
        &selector,
        &["runtime".to_owned()].into_iter().collect()
    ));
    assert!(!selector_matches_tantivy_terms(
        &selector,
        &["transport".to_owned()].into_iter().collect()
    ));
}
