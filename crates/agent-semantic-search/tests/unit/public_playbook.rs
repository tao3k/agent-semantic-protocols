use crate::parse_search_playbook_args;

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn playbook_parser_materializes_the_complete_typed_request() {
    let request = parse_search_playbook_args(&args(&[
        "search",
        "playbook",
        "runtime graph authority",
        "--intent",
        "relationship",
        "--scope",
        "owner:src/lib.rs",
        "--coverage",
        "candidates",
        "--max-owners",
        "24",
        "--deadline-ms",
        "750",
        "--explain",
        "full",
        "--language",
        "rust",
        "--workspace",
        "/tmp/project",
    ]))
    .unwrap();
    assert_eq!(request.query, "runtime graph authority");
    assert_eq!(request.intent, "relationship");
    assert_eq!(request.scope, "owner:src/lib.rs");
    assert_eq!(request.max_owners, 24);
    assert_eq!(request.deadline_ms, 750);
    assert_eq!(request.explain, "full");
    assert_eq!(request.language.as_deref(), Some("rust"));
    assert_eq!(request.workspace, "/tmp/project");
}

#[test]
fn non_playbook_search_operations_fail_closed() {
    for command in [
        args(&["search", "owner", "src/lib.rs", "items"]),
        args(&["search", "guide"]),
        args(&["search", "history", "query"]),
    ] {
        assert!(parse_search_playbook_args(&command).is_err(), "{command:?}");
    }
}

#[test]
fn complete_coverage_requires_absence_proof_intent() {
    let error = parse_search_playbook_args(&args(&[
        "search",
        "playbook",
        "missing authority",
        "--coverage",
        "complete",
    ]))
    .unwrap_err();
    assert_eq!(error, "complete coverage requires absence-proof intent");

    parse_search_playbook_args(&args(&[
        "search",
        "playbook",
        "missing authority",
        "--intent",
        "absence-proof",
        "--coverage",
        "complete",
    ]))
    .unwrap();
}

#[test]
fn unknown_options_are_never_silently_ignored() {
    let error = parse_search_playbook_args(&args(&[
        "search",
        "playbook",
        "query",
        "--backend",
        "tantivy",
    ]))
    .unwrap_err();
    assert_eq!(error, "search playbook does not support option `--backend`");
}
