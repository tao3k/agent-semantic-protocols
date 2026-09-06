use agent_semantic_search_projection::QueryPlaybookMaterializationRequest;
use agent_semantic_search_projection::SearchTopologySettlement;

fn settlement() -> SearchTopologySettlement {
    SearchTopologySettlement::admit(
        serde_json::from_str(include_str!(
            "../../../../schemas/fixtures/search-topology-settlement/valid-derived-and-proposed.v1.json"
        ))
        .expect("valid Search settlement fixture"),
    )
    .expect("admitted Search settlement")
}

fn request() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../../../../schemas/fixtures/query-playbook-materialization-request/valid-search-handoff.v1.json"
    ))
    .expect("valid Query Playbook handoff fixture")
}

#[test]
fn query_playbook_handoff_requires_the_exact_search_materialization_set() {
    QueryPlaybookMaterializationRequest::admit_for_settlement(request(), &settlement())
        .expect("exact Search MaterializationSet handoff must be admitted");

    let mut subset = request();
    subset["selectors"].as_array_mut().expect("selectors").pop();
    let error = QueryPlaybookMaterializationRequest::admit_for_settlement(subset, &settlement())
        .expect_err("a selector subset cannot claim Search recommendation authority");
    assert_eq!(
        error.reason_kind(),
        "query-playbook-materialization-mismatch"
    );
}

#[test]
fn query_playbook_handoff_rejects_cross_generation_replay_and_changed_proofs() {
    let mut replay = request();
    replay["settlementBinding"]["topologyGenerationDigest"] = serde_json::json!(
        "blake3-256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
    );
    let error = QueryPlaybookMaterializationRequest::admit_for_settlement(replay, &settlement())
        .expect_err("a Query handoff cannot replay another topology generation");
    assert_eq!(error.reason_kind(), "query-playbook-binding-mismatch");

    let mut changed_proofs = request();
    changed_proofs["proofDependencies"] = serde_json::json!([]);
    let error =
        QueryPlaybookMaterializationRequest::admit_for_settlement(changed_proofs, &settlement())
            .expect_err("proof dependency drift must fail before provider materialization");
    assert_eq!(
        error.reason_kind(),
        "query-playbook-materialization-mismatch"
    );
}
