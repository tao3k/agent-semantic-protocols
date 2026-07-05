use agent_semantic_search::{
    SourceIndexRankCandidate, SourceIndexRankRequest, rank_source_index_report,
};

#[test]
fn unit_test_target_keeps_public_rank_report_contract_runnable() {
    let report = rank_source_index_report(SourceIndexRankRequest {
        query: "crates/agent-semantic-search/src/source_index_rank.rs".to_owned(),
        candidates: vec![SourceIndexRankCandidate {
            ordinal: 0,
            path: "crates/agent-semantic-search/src/source_index_rank.rs".to_owned(),
            query_keys: vec!["source_index_rank".to_owned()],
        }],
    });

    let top = report
        .ranked_candidates
        .first()
        .expect("rank report should keep the unit_test smoke target runnable");
    assert_eq!(
        top.candidate.path,
        "crates/agent-semantic-search/src/source_index_rank.rs"
    );
    assert_eq!(top.score.exact_path, 1);
}
