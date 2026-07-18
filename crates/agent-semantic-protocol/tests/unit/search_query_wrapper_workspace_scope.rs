use std::collections::BTreeMap;
use std::path::Path;

use agent_semantic_search::{SemanticWorkspaceScope, SemanticWorkspaceScopeSet};
use serde_json::json;

use super::{Candidate, QueryCandidateCollection, admit_query_candidate_collection};

fn scope(
    workspace_id: &str,
    language_id: &str,
    provider_id: &str,
    extension: &str,
    root: &str,
    manifest: &str,
) -> SemanticWorkspaceScope {
    SemanticWorkspaceScope::from_packet(&json!({
        "schemaId": "agent.semantic-protocols.semantic-workspace-scope",
        "schemaVersion": "1",
        "workspaceId": workspace_id,
        "languageId": language_id,
        "providerId": provider_id,
        "packageManager": "test-pm",
        "sourceExtensions": [extension],
        "discoveryRoot": root,
        "anchors": [{
            "kind": "test-manifest",
            "path": manifest,
            "sha256": format!("sha256:{}", "a".repeat(64))
        }],
        "packages": [{
            "packageId": format!("{language_id}:package"),
            "name": "package",
            "root": root,
            "manifestPath": manifest,
            "languageId": language_id
        }],
        "admittedRoots": [root],
        "fingerprint": format!("sha256:{}", "b".repeat(64))
    }))
    .expect("scope")
}

fn candidate(path: &str) -> Candidate {
    Candidate {
        path: path.to_owned(),
        line: 1,
        end_line: 1,
        symbol: "candidate".to_owned(),
        selector: None,
        text: String::new(),
        source: "source-index".to_owned(),
        confidence: "exact".to_owned(),
    }
}

#[test]
fn scope_set_filters_before_query_wrapper_quality_and_frontier() {
    let scopes = SemanticWorkspaceScopeSet::new(vec![
        scope(
            "rust:root",
            "rust",
            "rs-harness",
            ".rs",
            "/repo/rust",
            "/repo/rust/Cargo.toml",
        ),
        scope(
            "python:root",
            "python",
            "py-harness",
            ".py",
            "/repo/python",
            "/repo/python/pyproject.toml",
        ),
    ])
    .expect("scope set");
    let mut collection = QueryCandidateCollection {
        candidates: vec![
            candidate("rust/src/lib.rs"),
            candidate("python/src/app.py"),
            candidate("docs/notes.md"),
        ],
        trace_fields: BTreeMap::new(),
        source_trace: Vec::new(),
        candidate_sources: vec!["source-index".to_owned()],
        failure_reason: None,
    };

    admit_query_candidate_collection(&mut collection, &scopes, Path::new("/repo"));

    assert_eq!(collection.candidates.len(), 2);
    assert!(
        collection
            .candidates
            .iter()
            .all(|candidate| candidate.path.ends_with(".rs") || candidate.path.ends_with(".py"))
    );
    assert_eq!(collection.failure_reason, None);
    let trace = collection.source_trace.last().expect("scope trace");
    assert_eq!(trace.status, "filtered");
    assert_eq!(trace.matched, 2);
    assert_eq!(trace.missing, 1);
}

#[test]
fn scope_set_closes_when_every_candidate_has_language_drift() {
    let scopes = SemanticWorkspaceScopeSet::new(vec![scope(
        "rust:root",
        "rust",
        "rs-harness",
        ".rs",
        "/repo/rust",
        "/repo/rust/Cargo.toml",
    )])
    .expect("scope set");
    let mut collection = QueryCandidateCollection {
        candidates: vec![candidate("rust/scripts/tool.py")],
        trace_fields: BTreeMap::new(),
        source_trace: Vec::new(),
        candidate_sources: vec!["source-index".to_owned()],
        failure_reason: None,
    };

    admit_query_candidate_collection(&mut collection, &scopes, Path::new("/repo"));

    assert!(collection.candidates.is_empty());
    assert_eq!(
        collection.failure_reason.as_deref(),
        Some("candidate-language-mismatch")
    );
}

#[test]
fn relative_candidate_resolves_against_explicit_subworkspace_root() {
    let scopes = SemanticWorkspaceScopeSet::new(vec![scope(
        "rust:search",
        "rust",
        "rs-harness",
        ".rs",
        "/repo/crates/search",
        "/repo/crates/search/Cargo.toml",
    )])
    .expect("scope set");
    let mut collection = QueryCandidateCollection {
        candidates: vec![candidate("src/workspace_scope.rs")],
        trace_fields: BTreeMap::new(),
        source_trace: Vec::new(),
        candidate_sources: vec!["fd-path".to_owned()],
        failure_reason: None,
    };

    admit_query_candidate_collection(&mut collection, &scopes, Path::new("/repo/crates/search"));

    assert_eq!(collection.candidates.len(), 1);
    assert_eq!(collection.failure_reason, None);
    assert_eq!(collection.source_trace[0].status, "used");
}
