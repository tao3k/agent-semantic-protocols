use std::collections::BTreeSet;

use crate::recall_route_planner::{
    GraphClosureState, IndexedLexicalObservation, LexicalBackendKind, PythonGraphObservation,
    RecallCalibrationState, RecallCompositionRequest, RecallQueryIntent,
    RipgrepVerificationObservation, plan_recall_composition,
};

fn owners(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn request(intent: RecallQueryIntent) -> RecallCompositionRequest {
    RecallCompositionRequest {
        workspace_identity: "workspace-a".to_owned(),
        generation_digest: "blake3-256:generation".to_owned(),
        source_root_digest: "blake3-256:root".to_owned(),
        intent,
        exact_selector_current: false,
        calibration_state: RecallCalibrationState::Calibrated,
        lexical: Some(IndexedLexicalObservation {
            backend: LexicalBackendKind::TantivyHotOverlay,
            admitted_owner_count: 4,
            indexed_owner_count: 4,
            candidate_owner_ids: owners(&["owner-a", "owner-b"]),
        }),
        graph: Some(PythonGraphObservation {
            closure_state: GraphClosureState::Complete,
            seed_owner_ids: owners(&["owner-a"]),
            candidate_owner_ids: owners(&["owner-a", "owner-c"]),
            unresolved_frontier_count: 0,
        }),
        ripgrep: Some(RipgrepVerificationObservation {
            admitted_owner_count: 4,
            covered_owner_count: 3,
            verified_owner_ids: owners(&["owner-a", "owner-b", "owner-c"]),
            matched_owner_ids: owners(&["owner-a", "owner-c"]),
        }),
    }
}

#[test]
fn conceptual_route_preserves_lexical_recall_then_graph_context_then_bytes() {
    let decision =
        plan_recall_composition(request(RecallQueryIntent::Conceptual)).expect("conceptual route");

    assert_eq!(
        decision
            .stages
            .iter()
            .map(|stage| stage.capability_id.as_str())
            .collect::<Vec<_>>(),
        [
            "search.indexed-lexical",
            "search.python-graph",
            "search.ripgrep-verify-candidates",
        ]
    );
    assert_eq!(decision.correlation.lexical_graph_overlap_count, 1);
    assert_eq!(decision.correlation.graph_marginal_candidate_count, 1);
    assert_eq!(decision.correlation.verified_union_candidate_count, 3);
    assert_eq!(decision.correlation.lexical_graph_jaccard_permille, 333);
    assert!(decision.residual_uncertainty.is_empty());
}

#[test]
fn relationship_route_uses_python_graph_before_lexical_gap_fill() {
    let decision = plan_recall_composition(request(RecallQueryIntent::Relationship))
        .expect("relationship route");

    assert_eq!(decision.stages[0].capability_id, "search.python-graph");
    assert_eq!(decision.stages[1].capability_id, "search.indexed-lexical");
    assert_eq!(
        decision.stages[2].capability_id,
        "search.ripgrep-verify-candidates"
    );
}

#[test]
fn absence_requires_complete_rg_coverage_and_never_uses_candidate_empty_as_proof() {
    let mut incomplete = request(RecallQueryIntent::AbsenceProof);
    let decision = plan_recall_composition(incomplete.clone()).expect("coverage route");
    assert_eq!(
        decision.stages[0].capability_id,
        "search.ripgrep-prove-coverage"
    );
    assert_eq!(decision.residual_uncertainty, ["coverage-proof-incomplete"]);

    incomplete
        .ripgrep
        .as_mut()
        .expect("rg lane")
        .covered_owner_count = 4;
    let complete = plan_recall_composition(incomplete).expect("complete coverage route");
    assert!(complete.residual_uncertainty.is_empty());
}

#[test]
fn exact_current_selector_bypasses_all_recall_lanes() {
    let mut exact = request(RecallQueryIntent::ExactSelector);
    exact.exact_selector_current = true;
    exact.lexical = None;
    exact.graph = None;
    exact.ripgrep = None;

    let decision = plan_recall_composition(exact).expect("direct selector route");
    assert_eq!(decision.stages[0].capability_id, "search.direct-selector");
    assert!(decision.residual_uncertainty.is_empty());
}

#[test]
fn online_route_reports_uncalibrated_and_open_graph_without_claiming_recall() {
    let mut open = request(RecallQueryIntent::Conceptual);
    open.calibration_state = RecallCalibrationState::Uncalibrated;
    let graph = open.graph.as_mut().expect("graph lane");
    graph.closure_state = GraphClosureState::FrontierOpen;
    graph.unresolved_frontier_count = 2;

    let decision = plan_recall_composition(open).expect("open graph route");
    assert_eq!(
        decision.residual_uncertainty,
        ["graph-frontier-open", "uncalibrated-route-quality"]
    );
}

#[test]
fn malformed_lane_evidence_fails_closed() {
    let mut malformed = request(RecallQueryIntent::Conceptual);
    malformed
        .ripgrep
        .as_mut()
        .expect("rg lane")
        .matched_owner_ids
        .insert("unverified-owner".to_owned());

    assert_eq!(
        plan_recall_composition(malformed),
        Err("ripgrep matches must be a subset of verified owners".to_owned())
    );
}
