use crate::{SourceIndexRankCandidate, rank_source_index_candidates, source_index_lookup_terms};

#[test]
fn source_index_lookup_terms_keep_full_query_and_split_terms() {
    let terms = source_index_lookup_terms("src/lib.rs source_index_owner");

    assert!(terms.contains(&"src/lib.rs source_index_owner".to_string()));
    assert!(terms.contains(&"src/lib".to_string()));
    assert!(terms.contains(&"rs".to_string()));
    assert!(terms.contains(&"source_index_owner".to_string()));
}

#[test]
fn source_index_ranking_prefers_query_axis_coverage_then_db_order() {
    let ranked = rank_source_index_candidates(
        vec![
            SourceIndexRankCandidate {
                ordinal: 0,
                path: "src/low.rs".to_string(),
                query_keys: vec!["source_index".to_string()],
            },
            SourceIndexRankCandidate {
                ordinal: 1,
                path: "src/source_index_owner.rs".to_string(),
                query_keys: vec!["source_index_owner".to_string()],
            },
            SourceIndexRankCandidate {
                ordinal: 2,
                path: "src/also.rs".to_string(),
                query_keys: vec!["source_index_owner".to_string()],
            },
        ],
        "source_index_owner",
    );

    assert_eq!(ranked[0].ordinal, 1);
    assert_eq!(ranked[1].ordinal, 2);
    assert_eq!(ranked[2].ordinal, 0);
}
