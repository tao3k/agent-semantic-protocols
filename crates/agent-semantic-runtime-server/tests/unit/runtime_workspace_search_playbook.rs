// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::{
    compile_graph_relation_pattern, fused_file_context_scope, smallest_selector_overlapping_line,
    source_line_ranges, structural_candidate_owner_scope,
};

#[test]
fn rg_and_tantivy_intersection_owns_the_file_context_scope() {
    let rg = ["src/rg-only.rs", "src/shared.rs"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    let tantivy = ["src/tantivy-only.rs", "src/shared.rs"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    assert_eq!(
        fused_file_context_scope(&rg, &tantivy),
        ["src/shared.rs"].into_iter().map(str::to_owned).collect()
    );
}

#[test]
fn graph_scope_comes_from_structural_matches_not_the_broader_file_scope() {
    let candidate =
        |owner: &str, selector: &str| agent_semantic_search::WorkspaceSearchSyntaxCandidate {
            owner: owner.to_owned(),
            selector: selector.to_owned(),
            relation: "syntax-capture:symbol".to_owned(),
            hit: Default::default(),
        };
    let (owners, truncated) = structural_candidate_owner_scope(
        &[
            candidate("src/matched.rs", "rust://src/matched.rs#item/function/a"),
            candidate("src/matched.rs", "rust://src/matched.rs#item/function/b"),
        ],
        30,
    );
    assert_eq!(owners, ["src/matched.rs"]);
    assert!(!truncated);
}

fn graph_block(language: &str, source: &str) -> agent_semantic_search::GraphNativeBlock {
    agent_semantic_search::GraphNativeBlock {
        language: language.to_owned(),
        argv: vec![source.to_owned()],
    }
}

#[test]
fn gql_adapter_uses_compiler_ir_for_the_admitted_relation_slice() {
    let Ok(pattern) = compile_graph_relation_pattern(&graph_block(
        "gql",
        "MATCH (h:Owner)-[:PUBLISHES]->(e:RuntimeEndpoint) RETURN h, e",
    )) else {
        panic!("expected compiled GQL relation pattern");
    };
    assert!(pattern.left_binding.eq_ignore_ascii_case("h"));
    assert!(pattern.left_kind.eq_ignore_ascii_case("Owner"));
    assert!(pattern.relation.eq_ignore_ascii_case("PUBLISHES"));
    assert!(matches!(
        pattern.direction,
        agent_semantic_search::ResidentGraphRelationDirection::Out
    ));
    assert!(pattern.right_kind.eq_ignore_ascii_case("RuntimeEndpoint"));
    assert_eq!(
        pattern
            .projected_bindings
            .iter()
            .map(|binding| binding.to_ascii_lowercase())
            .collect::<Vec<_>>(),
        ["h", "e"]
    );
}

#[test]
fn gql_adapter_rejects_semantics_the_executor_does_not_implement() {
    let Err(super::AspClientOperationError::Message(error)) =
        compile_graph_relation_pattern(&graph_block(
            "gql",
            "MATCH (h:Owner)-[:PUBLISHES]->(e:RuntimeEndpoint) WHERE h = 1 RETURN h",
        ))
    else {
        panic!("filtered GQL must fail closed");
    };
    assert!(error.contains("unfiltered MATCH"), "{error}");
}

#[test]
fn pgql_uses_the_same_v1_relation_ir_boundary() {
    let Ok(pattern) = compile_graph_relation_pattern(&graph_block(
        "pgql",
        "MATCH (h:Owner)-[:PUBLISHES]->(e:RuntimeEndpoint) RETURN h, e",
    )) else {
        panic!("expected PGQL relation pattern to share the V1 IR boundary");
    };
    assert_eq!(
        pattern
            .projected_bindings
            .iter()
            .map(|binding| binding.to_ascii_lowercase())
            .collect::<Vec<_>>(),
        ["h", "e"]
    );
}

#[test]
fn gql_adapter_rejects_a_projection_without_an_owner_endpoint() {
    let Err(super::AspClientOperationError::Message(error)) =
        compile_graph_relation_pattern(&graph_block(
            "gql",
            "MATCH (h:Owner)-[:PUBLISHES]->(e:RuntimeEndpoint) RETURN e",
        ))
    else {
        panic!("non-owner projection must fail closed");
    };
    assert!(error.contains("project an Owner endpoint"), "{error}");
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
