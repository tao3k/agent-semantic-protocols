use std::path::PathBuf;
use std::time::Instant;

use agent_semantic_search::file_locator::{
    FileLocatorIndex, FileLocatorMatchKind, FileLocatorQuery,
};

fn fixture_paths() -> Vec<PathBuf> {
    [
        "docs/10.05/search.org",
        "docs/10.05/planner.org",
        "src/search/planner.rs",
        "research/notes/evidence.md",
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect()
}

#[test]
fn locates_exact_path_without_scanning_content() {
    let index = FileLocatorIndex::build(fixture_paths());
    let matches = index.locate(&FileLocatorQuery::new("src/search/planner.rs"));

    assert_eq!(matches[0].workspace_relative_path, "src/search/planner.rs");
    assert_eq!(matches[0].match_kind, FileLocatorMatchKind::ExactPath);
}

#[test]
fn locates_basename_and_stem_from_indexes() {
    let index = FileLocatorIndex::build(fixture_paths());
    let basename = index.locate(&FileLocatorQuery::new("planner.rs"));
    let stem = index.locate(&FileLocatorQuery::new("planner"));

    assert_eq!(basename[0].match_kind, FileLocatorMatchKind::Basename);
    assert!(
        stem.iter()
            .any(|candidate| candidate.workspace_relative_path.ends_with("planner.rs"))
    );
    assert!(
        stem.iter()
            .any(|candidate| candidate.workspace_relative_path.ends_with("planner.org"))
    );
}

#[test]
fn locates_suffix_path_and_segments() {
    let index = FileLocatorIndex::build(fixture_paths());
    let suffix = index.locate(&FileLocatorQuery::new("10.05/search.org"));
    let segment = index.locate(&FileLocatorQuery::new("research"));

    assert_eq!(suffix[0].match_kind, FileLocatorMatchKind::SuffixPath);
    assert_eq!(suffix[0].workspace_relative_path, "docs/10.05/search.org");
    assert_eq!(segment[0].match_kind, FileLocatorMatchKind::Segment);
    assert_eq!(
        segment[0].workspace_relative_path,
        "research/notes/evidence.md"
    );
}

#[test]
fn locates_globs_with_globset() {
    let index = FileLocatorIndex::build(fixture_paths());
    let matches = index.locate(&FileLocatorQuery::new("docs/10.05/*.org"));

    assert_eq!(matches.len(), 2);
    assert!(
        matches
            .iter()
            .all(|candidate| candidate.match_kind == FileLocatorMatchKind::Glob)
    );
}

#[test]
fn locates_fuzzy_filename_tokens() {
    let index = FileLocatorIndex::build(fixture_paths());
    let matches = index.locate(&FileLocatorQuery::new("evidenc"));

    assert!(
        matches
            .iter()
            .any(|candidate| candidate.workspace_relative_path == "research/notes/evidence.md")
    );
}

#[test]
fn hot_path_lookup_stays_under_two_milliseconds() {
    let mut paths = (0..20_000)
        .map(|index| PathBuf::from(format!("crates/pkg-{index}/src/module_{index}.rs")))
        .collect::<Vec<_>>();
    paths.push(PathBuf::from(
        "crates/agent-semantic-search/src/file_locator.rs",
    ));

    let index = FileLocatorIndex::build(paths);
    let queries = [
        "crates/agent-semantic-search/src/file_locator.rs",
        "file_locator.rs",
        "agent-semantic-search/src/file_locator.rs",
        "file_locatr",
    ];

    for query in queries {
        let started = Instant::now();
        let matches = index.locate(&FileLocatorQuery::new(query).with_limit(8));
        let elapsed = started.elapsed();

        assert!(
            !matches.is_empty(),
            "query {query} should return at least one candidate"
        );
        assert!(
            elapsed.as_micros() < 2_000,
            "query {query} took {elapsed:?}, expected < 2ms"
        );
    }
}
