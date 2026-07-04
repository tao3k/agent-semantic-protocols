use crate::{
    SearchPipeEvidenceCandidate, SearchPipeQueryTerm, SearchPipeTermRole,
    search_pipe_declaration_header_match, search_pipe_parser_handles,
    search_pipe_search_overlay_handles, search_pipe_strong_match, search_pipe_weak_reason,
};

#[test]
fn search_pipe_evidence_matches_declarations_and_rust_compound_paths() {
    let candidate = candidate(
        "src/router.rs",
        7,
        "SearchRouter",
        "pub struct SearchRouter {}",
        "source-index",
    );
    let declaration = term("SearchRouter", SearchPipeTermRole::Symbol);
    let compound = term("router::SearchRouter", SearchPipeTermRole::Symbol);

    assert!(search_pipe_declaration_header_match(
        "rust",
        &candidate,
        &declaration
    ));
    assert!(search_pipe_strong_match("rust", &candidate, &compound));
}

#[test]
fn search_pipe_evidence_projects_parser_and_search_overlay_handles() {
    let candidates = vec![
        candidate(
            "src/router.rs",
            7,
            "SearchRouter",
            "pub struct SearchRouter {}",
            "source-index",
        ),
        candidate(
            "src/cache.rs",
            3,
            "CacheStatus",
            "CacheStatus hit",
            "search-overlay",
        ),
    ];
    let terms = vec![
        term("SearchRouter", SearchPipeTermRole::Symbol),
        term("CacheStatus", SearchPipeTermRole::Symbol),
    ];

    assert_eq!(
        search_pipe_parser_handles("rust", &candidates, &terms),
        vec!["SearchRouter@src/router.rs:7".to_string()]
    );
    assert_eq!(
        search_pipe_search_overlay_handles(&candidates, &terms),
        vec!["CacheStatus".to_string()]
    );
}

#[test]
fn search_pipe_evidence_explains_weak_package_drift() {
    let candidates = vec![
        candidate(
            "packages/runtime/search/src/router.rs",
            1,
            "searchrouter",
            "SearchRouterField",
            "source-index",
        ),
        candidate(
            "crates/agent-semantic-search/src/lib.rs",
            1,
            "Other",
            "SearchRouterField",
            "source-index",
        ),
    ];
    let reason = search_pipe_weak_reason(
        &term("SearchRouter", SearchPipeTermRole::Symbol),
        &candidates,
    );

    assert!(reason.contains("lowercase-symbol"));
    assert!(reason.contains("field-substring"));
    assert!(reason.contains("package-drift"));
}

fn candidate(
    path: &str,
    line: usize,
    symbol: &str,
    text: &str,
    source: &str,
) -> SearchPipeEvidenceCandidate {
    SearchPipeEvidenceCandidate {
        path: path.to_string(),
        line,
        symbol: symbol.to_string(),
        text: text.to_string(),
        source: source.to_string(),
    }
}

fn term(raw: &str, role: SearchPipeTermRole) -> SearchPipeQueryTerm {
    SearchPipeQueryTerm {
        raw: raw.to_string(),
        lower: raw.to_ascii_lowercase(),
        role,
    }
}
