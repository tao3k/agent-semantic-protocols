use crate::{
    SearchPipeCohesionTerm, SearchPipeQueryTerm, SearchPipeTermRole,
    search_pipe_candidate_packages, search_pipe_fd_query_terms, search_pipe_missing_path_terms,
    search_pipe_owner_seed_terms, search_pipe_package_cohesion, search_pipe_package_key,
    search_pipe_quality_risks, search_pipe_query_pack_quality,
};

#[test]
fn search_pipe_package_key_matches_workspace_package_axes() {
    assert_eq!(
        search_pipe_package_key("packages/runtime/search/src/router.rs"),
        "packages/runtime/search"
    );
    assert_eq!(
        search_pipe_package_key("crates/asp/src/lib.rs"),
        "crates/asp"
    );
}

#[test]
fn search_pipe_candidate_packages_are_stable_and_bounded() {
    let packages = search_pipe_candidate_packages(
        [
            "packages/runtime/search/src/router.rs",
            "packages/runtime/search/src/lib.rs",
            "crates/agent-semantic-search/src/lib.rs",
        ]
        .into_iter()
        .map(str::to_string),
    );

    assert_eq!(
        packages,
        vec![
            "crates/agent-semantic-search".to_string(),
            "packages/runtime/search".to_string()
        ]
    );
}

#[test]
fn search_pipe_package_cohesion_requires_owner_hits_for_package_axes() {
    let packages = vec![
        "packages/runtime/search".to_string(),
        "crates/agent-semantic-search".to_string(),
    ];
    let terms = vec![
        SearchPipeCohesionTerm::new("runtime-search-route", "runtime-search-route"),
        SearchPipeCohesionTerm::new("agent-semantic-search", "agent-semantic-search"),
    ];
    let weak_owner = vec!["runtime-search-route".to_string()];
    let strong_owner = vec![
        "runtime-search-route".to_string(),
        "agent-semantic-search".to_string(),
    ];

    assert_eq!(
        search_pipe_package_cohesion(&packages, Some(&weak_owner), &terms),
        "low"
    );
    assert_eq!(
        search_pipe_package_cohesion(&packages, Some(&strong_owner), &terms),
        "medium"
    );
}

#[test]
fn search_pipe_quality_decision_flags_broad_weak_package_drift() {
    let terms = vec![
        term("src/runtime.rs", SearchPipeTermRole::Symbol),
        term("packages/runtime-search", SearchPipeTermRole::Symbol),
        term("SearchRouter", SearchPipeTermRole::Symbol),
        term("CacheStatus", SearchPipeTermRole::Symbol),
        term("concurrency", SearchPipeTermRole::Concept),
    ];
    let global_missing = Vec::new();
    let strong_matched = Vec::new();
    let weak_terms = vec!["SearchRouter".to_string()];

    let risks = search_pipe_quality_risks(
        &terms,
        ["pub struct SearchRouter {\n    very_long_field: String\n}".to_string()].into_iter(),
        &global_missing,
        &strong_matched,
        &weak_terms,
        "low",
        1,
    );

    assert!(risks.iter().any(|risk| risk == "single-broad-clause"));
    assert!(risks.iter().any(|risk| risk == "coverage-inflation"));
    assert!(risks.iter().any(|risk| risk == "package-drift"));
    assert!(risks.iter().any(|risk| risk == "weak-camelcase-match"));
    assert!(risks.iter().any(|risk| risk == "long-field-signatures"));
    assert!(risks.iter().any(|risk| risk == "no-strong-symbol-coverage"));
    assert_eq!(
        search_pipe_query_pack_quality(&terms, &global_missing, &weak_terms, &risks),
        "low"
    );
    assert_eq!(
        search_pipe_fd_query_terms(&terms, &weak_terms, &strong_matched, &risks),
        Some("SearchRouter".to_string())
    );
}

#[test]
fn search_pipe_quality_decision_projects_missing_paths_and_owner_seeds() {
    let terms = vec![
        term("src/runtime.rs", SearchPipeTermRole::Symbol),
        term("SearchRouter", SearchPipeTermRole::Symbol),
        term("concurrency", SearchPipeTermRole::Concept),
    ];
    let global_matched = vec!["searchrouter".to_string()];
    let missing_path_terms = search_pipe_missing_path_terms(&terms, &global_matched);

    assert_eq!(missing_path_terms, vec!["src/runtime.rs".to_string()]);
    assert_eq!(
        search_pipe_owner_seed_terms(&terms, &missing_path_terms),
        vec!["SearchRouter".to_string()]
    );
}

fn term(raw: &str, role: SearchPipeTermRole) -> SearchPipeQueryTerm {
    SearchPipeQueryTerm {
        raw: raw.to_string(),
        lower: raw.to_ascii_lowercase(),
        role,
    }
}
