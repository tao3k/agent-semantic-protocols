use std::time::{Duration, Instant};

use agent_semantic_search::{
    SourceIndexRankCandidate, SourceIndexRankRequest, rank_source_index_report,
};

const INTERACTIVE_RANK_CANDIDATE_COUNT: usize = 1_024;
#[cfg(not(debug_assertions))]
const LARGE_RANK_CANDIDATE_COUNT: usize = 20_000;
const RANK_HOT_PATH_LIMIT: Duration = Duration::from_millis(20);
const TARGET_PATH: &str = "crates/agent-semantic-search/src/source_index_rank.rs";

#[test]
fn rank_report_interactive_page_stays_under_twenty_milliseconds() {
    assert_rank_report_hot_path_stays_under_limit(INTERACTIVE_RANK_CANDIDATE_COUNT);
}

#[cfg(not(debug_assertions))]
#[test]
fn rank_report_large_release_batch_stays_under_twenty_milliseconds() {
    assert_rank_report_hot_path_stays_under_limit(LARGE_RANK_CANDIDATE_COUNT);
}

fn assert_rank_report_hot_path_stays_under_limit(candidate_count: usize) {
    let candidates = synthetic_source_index_rank_candidates(candidate_count);

    let started = Instant::now();
    let report = rank_source_index_report(SourceIndexRankRequest {
        query: TARGET_PATH.to_owned(),
        candidates,
    });
    let elapsed = started.elapsed();

    let top = report
        .ranked_candidates
        .first()
        .expect("rank report should contain candidates");
    assert_eq!(top.candidate.path, TARGET_PATH);
    assert_eq!(top.score.exact_path, 1);
    assert!(top.score.total > 0);
    assert!(
        elapsed < RANK_HOT_PATH_LIMIT,
        "rank report over {candidate_count} candidates took {elapsed:?}, expected < {RANK_HOT_PATH_LIMIT:?}"
    );
}

fn synthetic_source_index_rank_candidates(candidate_count: usize) -> Vec<SourceIndexRankCandidate> {
    (0..candidate_count)
        .map(|ordinal| {
            let path = if ordinal == candidate_count / 2 {
                TARGET_PATH.to_owned()
            } else {
                format!(
                    "crates/synthetic-package-{}/src/module_{ordinal}.rs",
                    ordinal % 128
                )
            };
            SourceIndexRankCandidate {
                ordinal,
                query_keys: vec![
                    format!("synthetic-package-{}", ordinal % 128),
                    format!("module_{ordinal}"),
                ],
                path,
            }
        })
        .collect()
}
