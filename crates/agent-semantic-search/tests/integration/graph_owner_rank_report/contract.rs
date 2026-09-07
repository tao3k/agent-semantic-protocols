// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_search::GraphOwnerRankCandidate;
use agent_semantic_search::GraphOwnerRankRequest;
use agent_semantic_search::rank_graph_owner_report;

#[test]
fn graph_owner_rank_report_is_public_and_constructible() {
    let fixture = super::source_snapshot_fixture::canonical_test_snapshot();
    let generation =
        agent_semantic_search::graph_generation_authority::AdmittedGraphGenerationV1::admit(
            &fixture.evidence,
            &fixture.generation,
            &fixture.generation,
        )
        .expect("canonical graph generation");
    let report = rank_graph_owner_report(GraphOwnerRankRequest::from_admitted_generation(
        vec![
            GraphOwnerRankCandidate::new(
                "src/lib.rs",
                "SearchRouter",
                "dynamic overlay graph ranking",
                "source-index",
                "high",
            ),
            GraphOwnerRankCandidate::new(
                "languages/rust/src/lib.rs",
                "SearchRouter",
                "dynamic overlay graph ranking",
                "source-index",
                "high",
            ),
        ],
        vec!["dynamicOverlay".to_string()],
        vec!["languages/rust".to_string()],
        &generation,
    ));

    let top = report
        .ranked_owners
        .first()
        .expect("public graph owner rank report should include owners");
    assert_eq!(top.path, "languages/rust/src/lib.rs");
    assert_eq!(
        top.topology_submodule_path.as_deref(),
        Some("languages/rust")
    );
    assert_eq!(top.score.query_axis_count, 2);
    assert!(top.score.total > 0);
}
