// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::{
    bounded_graph_seed_scope, bounded_semantic_projection_scope, branch_marginal_reductions,
    compile_graph_relation_pattern, fused_file_context_scope, intersect_clause_owner_scopes,
    resident_grep_candidate_scope, retrieval_composition_kind, smallest_selector_overlapping_line,
    source_line_ranges,
};

#[test]
fn implicit_parser_projection_is_bounded_after_complete_retrieval() {
    let complete = ["a.rs", "b.rs", "c.rs", "d.rs"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    let matches = [
        super::RuntimeGrepMatch {
            owner_path: "c.rs".to_owned(),
            owner_line: 7,
            byte_start: 6,
            byte_end: 7,
        },
        super::RuntimeGrepMatch {
            owner_path: "c.rs".to_owned(),
            owner_line: 9,
            byte_start: 8,
            byte_end: 9,
        },
        super::RuntimeGrepMatch {
            owner_path: "a.rs".to_owned(),
            owner_line: 3,
            byte_start: 2,
            byte_end: 3,
        },
    ];
    let ranked_owner_paths = ["a.rs".to_owned(), "c.rs".to_owned()];
    let ranked = vec![ranked_owner_paths.as_slice()];
    let (projection, truncated) =
        bounded_semantic_projection_scope(&complete, &matches, &ranked, 1);

    assert_eq!(
        projection,
        ["c.rs"].into_iter().map(str::to_owned).collect()
    );
    assert!(truncated);
    assert_eq!(complete.len(), 4, "the complete retrieval set is unchanged");
}

#[test]
fn implicit_projection_fills_from_complete_scope_when_no_rg_line_exists() {
    let complete = ["a.rs", "b.rs", "c.rs"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    let ranked_owner_paths = ["c.rs".to_owned(), "b.rs".to_owned(), "a.rs".to_owned()];
    let ranked = vec![ranked_owner_paths.as_slice()];
    let (projection, truncated) =
        bounded_semantic_projection_scope(&complete, std::iter::empty(), &ranked, 2);

    assert_eq!(
        projection,
        ["b.rs", "c.rs"].into_iter().map(str::to_owned).collect()
    );
    assert!(truncated);
}

#[test]
fn top_k_projection_scenario_preserves_complete_retrieval_and_bounds_parser_work() {
    let scenario = toml::from_str::<toml::Value>(include_str!(
        "scenarios/runtime_search_top_k_projection/scenario.toml"
    ))
    .expect("Top-K projection Scenario metadata");
    let benchmark = toml::from_str::<toml::Value>(include_str!(
        "scenarios/runtime_search_top_k_projection/benchmark.toml"
    ))
    .expect("Top-K projection benchmark metadata");
    let retrieval_owner_count = scenario["model"]["retrieval_owner_count"]
        .as_integer()
        .and_then(|count| usize::try_from(count).ok())
        .expect("retrieval owner count");
    let top_k = scenario["model"]["evidence_top_k"]
        .as_integer()
        .and_then(|count| usize::try_from(count).ok())
        .expect("evidence Top-K");
    let complete = (0..retrieval_owner_count)
        .map(|index| format!("src/owner-{index:03}.rs"))
        .collect::<std::collections::BTreeSet<_>>();
    let matches = complete
        .iter()
        .enumerate()
        .flat_map(|(index, owner)| {
            (0..=(index % 3)).map(move |line| super::RuntimeGrepMatch {
                owner_path: owner.clone(),
                owner_line: u64::try_from(line + 1).expect("bounded line"),
                byte_start: line,
                byte_end: line + 1,
            })
        })
        .collect::<Vec<_>>();

    let ranked_owner_paths = complete.iter().rev().cloned().collect::<Vec<_>>();
    let ranked = vec![ranked_owner_paths.as_slice()];
    let (projection, truncated) =
        bounded_semantic_projection_scope(&complete, &matches, &ranked, top_k);

    assert_eq!(complete.len(), retrieval_owner_count);
    assert_eq!(projection.len(), top_k);
    assert!(projection.is_subset(&complete));
    assert!(truncated);
    assert_eq!(
        benchmark["observed"]["after"]["projection_owner_count"].as_integer(),
        Some(i64::try_from(top_k).expect("Top-K fits i64"))
    );
    assert!(
        benchmark["observed"]["after"]["owner_materialization_micros"].as_integer()
            < benchmark["observed"]["before"]["owner_materialization_micros"].as_integer()
    );
}

#[test]
fn implicit_projection_fuses_complete_branch_ranks_before_top_k() {
    let complete = ["a.rs", "b.rs", "c.rs"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    let branch_1 = ["a.rs".to_owned(), "b.rs".to_owned(), "c.rs".to_owned()];
    let branch_2 = ["b.rs".to_owned(), "c.rs".to_owned(), "a.rs".to_owned()];
    let branches = vec![branch_1.as_slice(), branch_2.as_slice()];

    let (projection, truncated) =
        bounded_semantic_projection_scope(&complete, std::iter::empty(), &branches, 1);

    assert_eq!(
        projection,
        ["b.rs"].into_iter().map(str::to_owned).collect()
    );
    assert!(truncated);
}

#[test]
fn runtime_reads_retrieval_semantics_from_the_operator_tree() {
    use agent_semantic_client_protocol::{
        AspClientSearchPlaybookClauseAxis as Axis, AspClientSearchPlaybookClauseRef as Clause,
        AspClientSearchPlaybookComposition as Composition,
    };
    let leaf = |axis| Composition::Leaf {
        clause: Clause {
            axis,
            block_index: 0,
        },
    };
    assert_eq!(
        retrieval_composition_kind(&leaf(Axis::Rg)),
        super::RetrievalCompositionKind::Single
    );
    assert_eq!(
        retrieval_composition_kind(&leaf(Axis::Syntax)),
        super::RetrievalCompositionKind::None
    );
    assert_eq!(
        retrieval_composition_kind(&leaf(Axis::Topology)),
        super::RetrievalCompositionKind::Single
    );
    assert_eq!(
        retrieval_composition_kind(&Composition::Chain {
            children: vec![Composition::Intersect {
                children: vec![leaf(Axis::Rg), leaf(Axis::Tantivy)],
            }],
        }),
        super::RetrievalCompositionKind::Intersect
    );
}

#[test]
fn topology_owner_language_is_derived_from_admitted_provider_routes() {
    let route = agent_semantic_search::WorkspaceSearchPlaybookRoute {
        language_id: "rust".to_owned(),
        provider_id: "asp-rust".to_owned(),
        producer_axis: agent_semantic_search::WorkspaceSearchProducerAxis::Language,
        generation_digest: format!("blake3-256:{}", "1".repeat(64)),
        extensions: vec!["rs".to_owned()],
    };
    assert!(matches!(
        super::topology_owner_language("crates/runtime/src/lib.rs", std::slice::from_ref(&route)),
        Ok(Some(language)) if language == "rust"
    ));
    assert!(matches!(
        super::topology_owner_language("docs/runtime.org", &[route]),
        Ok(None)
    ));
}

#[test]
fn branch_receipt_detects_zero_marginal_intersection_work() {
    let branches = vec![
        (
            agent_semantic_search::WorkspaceSearchAxisKind::Rg,
            0,
            ["a.rs", "shared.rs"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        ),
        (
            agent_semantic_search::WorkspaceSearchAxisKind::Tantivy,
            0,
            ["shared.rs"].into_iter().map(str::to_owned).collect(),
        ),
    ];
    let reductions = branch_marginal_reductions(&branches);
    assert_eq!(
        reductions[&(agent_semantic_search::WorkspaceSearchAxisKind::Rg, 0)],
        0
    );
    assert_eq!(
        reductions[&(agent_semantic_search::WorkspaceSearchAxisKind::Tantivy, 0)],
        1
    );
}

#[test]
fn independent_branches_both_contribute_to_a_real_intersection() {
    let branches = vec![
        (
            agent_semantic_search::WorkspaceSearchAxisKind::Rg,
            0,
            ["rg-only.rs", "shared.rs"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        ),
        (
            agent_semantic_search::WorkspaceSearchAxisKind::Tantivy,
            0,
            ["tantivy-only.rs", "shared.rs"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        ),
    ];
    assert!(
        branch_marginal_reductions(&branches)
            .values()
            .all(|reduction| *reduction == 1)
    );
}

#[test]
fn truncated_tantivy_scope_cannot_authorize_an_empty_intersection() {
    let ranked = ["src/unrelated.rs", "src/hit.rs"];
    let grep = ["src/hit.rs".to_owned()].into_iter().collect();
    let scope = super::TantivyClauseResult {
        owners: ranked
            .iter()
            .take(1)
            .map(|path| (*path).to_owned())
            .collect(),
        syntax_candidates: Vec::new(),
        truncated: true,
    };
    assert!(fused_file_context_scope(&grep, &scope.owners.iter().cloned().collect()).is_empty());
    assert!(scope.require_complete_fused_scope().is_err());
    let complete = super::TantivyClauseResult {
        owners: ranked.iter().map(|path| (*path).to_owned()).collect(),
        syntax_candidates: Vec::new(),
        truncated: false,
    };
    assert!(complete.require_complete_fused_scope().is_ok());
    assert_eq!(
        fused_file_context_scope(&grep, &complete.owners.into_iter().collect()),
        grep
    );
}

#[test]
fn every_intersection_branch_requires_complete_untruncated_coverage() {
    for axis in [
        agent_semantic_search::WorkspaceSearchAxisKind::Rg,
        agent_semantic_search::WorkspaceSearchAxisKind::Tantivy,
    ] {
        assert!(super::require_complete_intersection_branch(axis, false).is_ok());
        let error = super::require_complete_intersection_branch(axis, true)
            .expect_err("a truncated branch cannot authorize set intersection");
        let super::AspClientOperationError::Message(message) = error else {
            panic!("truncated intersection branch must return a query-not-ready message");
        };
        assert!(message.contains("candidate scope truncated before explicit intersection"));
    }
}

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
fn every_same_axis_leaf_remains_a_required_intersection_predicate() {
    let scopes = [
        ["a.rs", "shared.rs"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        ["shared.rs", "b.rs"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
    ];
    assert_eq!(
        intersect_clause_owner_scopes(&scopes),
        ["shared.rs"].into_iter().map(str::to_owned).collect()
    );
}

#[test]
fn an_empty_leaf_proves_the_whole_intersection_empty() {
    let scopes = [
        ["a.rs"].into_iter().map(str::to_owned).collect(),
        Default::default(),
        ["a.rs"].into_iter().map(str::to_owned).collect(),
    ];
    assert!(intersect_clause_owner_scopes(&scopes).is_empty());
}

#[test]
fn non_selective_grep_plan_uses_only_the_bounded_tantivy_scope() {
    let tantivy = ["src/a.rs", "src/b.rs"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    let (owners, receipt) = resident_grep_candidate_scope(
        &agent_semantic_search::ResidentGrepCandidatePlan::MatchAll,
        &tantivy,
        2,
        || panic!("MatchAll must not enter the trigram index"),
    )
    .unwrap();
    assert_eq!(owners, ["src/a.rs", "src/b.rs"]);
    assert_eq!(receipt.requested_gram_count, 0);
    assert_eq!(receipt.candidate_count, 2);
    assert_eq!(receipt.decoded_posting_count, 0);
}

#[test]
fn selective_grep_candidates_are_fused_before_exact_owner_reads() {
    let scope = ["b.rs".to_owned()].into_iter().collect();
    let (owners, receipt) = resident_grep_candidate_scope(
        &agent_semantic_search::ResidentGrepCandidatePlan::Grams(vec![1]),
        &scope,
        3,
        || {
            Ok((
                vec!["a.rs".into(), "b.rs".into(), "c.rs".into()],
                agent_semantic_search::ResidentByteCoverageQueryReceipt {
                    requested_gram_count: 1,
                    decoded_posting_count: 3,
                    smallest_posting_count: 3,
                    candidate_count: 3,
                    lookup_nanos: 1,
                },
            ))
        },
    )
    .unwrap();
    assert_eq!(owners, ["b.rs"]);
    assert_eq!(receipt.candidate_count, 1);
    assert_eq!(receipt.decoded_posting_count, 3);
}

#[test]
fn graph_scope_comes_from_the_final_typed_frontier() {
    let scope = ["src/matched.rs".to_owned()].into_iter().collect();
    let (owners, truncated) = bounded_graph_seed_scope(&scope, 30);
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
