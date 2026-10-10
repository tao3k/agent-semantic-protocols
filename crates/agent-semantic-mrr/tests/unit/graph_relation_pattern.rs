// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_mrr::{GraphRelationDirection, compile_graph_relation_pattern_v1};

fn compile(
    language: &str,
    source: &str,
) -> Result<agent_semantic_mrr::GraphRelationPattern, String> {
    compile_graph_relation_pattern_v1(language, &[source.to_owned()], Some("Owner"))
        .map_err(|error| error.to_string())
}

#[test]
fn compiler_ir_produces_the_admitted_relation_slice() {
    let pattern = compile(
        "gql",
        "MATCH (h:Owner)-[:PUBLISHES]->(e:RuntimeEndpoint) RETURN h, e",
    )
    .expect("compiled relation pattern");
    assert_eq!(pattern.relation, "PUBLISHES");
    assert_eq!(pattern.direction, GraphRelationDirection::Out);
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
fn unsupported_semantics_fail_closed() {
    let error = compile(
        "gql",
        "MATCH (h:Owner)-[:PUBLISHES]->(e:RuntimeEndpoint) WHERE h = 1 RETURN h",
    )
    .expect_err("filtered GQL must fail closed");
    assert!(error.contains("unfiltered MATCH"), "{error}");
}

#[test]
fn pgql_uses_the_same_ir_boundary() {
    compile(
        "pgql",
        "MATCH (h:Owner)-[:PUBLISHES]->(e:RuntimeEndpoint) RETURN h, e",
    )
    .expect("PGQL relation pattern");
}

#[test]
fn caller_owned_required_projection_is_enforced() {
    let error = compile(
        "gql",
        "MATCH (h:Owner)-[:PUBLISHES]->(e:RuntimeEndpoint) RETURN e",
    )
    .expect_err("Owner projection is required");
    assert!(error.contains("project an Owner endpoint"), "{error}");
}
