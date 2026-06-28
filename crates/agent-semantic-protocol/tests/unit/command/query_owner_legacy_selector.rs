#[path = "../../../src/command/query_owner_legacy_selector.rs"]
mod query_owner_legacy_selector;

use std::path::PathBuf;

use query_owner_legacy_selector::parse_legacy_owner_selector;

#[test]
fn parses_single_line_display_selector() {
    let selector = parse_legacy_owner_selector("rank_frontier@src/ranking.py:28")
        .expect("parse legacy selector");

    assert_eq!(selector.term, "rank_frontier");
    assert_eq!(selector.owner_path, PathBuf::from("src/ranking.py"));
}

#[test]
fn parses_line_range_display_selector() {
    let selector = parse_legacy_owner_selector("rank_frontier@src/ranking.py:28-134")
        .expect("parse legacy selector");

    assert_eq!(selector.term, "rank_frontier");
    assert_eq!(selector.owner_path, PathBuf::from("src/ranking.py"));
}

#[test]
fn preserves_file_like_colons_that_are_not_line_suffixes() {
    let selector = parse_legacy_owner_selector("rank_frontier@src/ranking.py:owner")
        .expect("parse legacy selector");

    assert_eq!(selector.owner_path, PathBuf::from("src/ranking.py:owner"));
}

#[test]
fn rejects_empty_or_missing_terms() {
    assert!(parse_legacy_owner_selector("@src/ranking.py:28").is_none());
    assert!(parse_legacy_owner_selector("rank_frontier").is_none());
}
