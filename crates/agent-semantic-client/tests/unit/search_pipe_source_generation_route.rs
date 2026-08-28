use super::{fast_search_requires_source_index_snapshot, parse_lexical_args};

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn owner_discovery_does_not_require_a_persisted_source_envelope() {
    let owner_args = args(&["search", "owner", "src/lib.rs", "items", "--query", "run"]);
    assert!(!fast_search_requires_source_index_snapshot(&owner_args));
    assert_eq!(
        parse_lexical_args(&owner_args).expect_err("owner must not parse as lexical"),
        "expected `search lexical`"
    );
}

#[test]
fn generation_consumers_fail_closed_at_their_route_boundary() {
    assert!(fast_search_requires_source_index_snapshot(&args(&[
        "search", "pipe", "run",
    ])));
    assert!(fast_search_requires_source_index_snapshot(&args(&[
        "search", "lexical", "run",
    ])));
    assert!(fast_search_requires_source_index_snapshot(&args(&[
        "search", "ingest",
    ])));
}
