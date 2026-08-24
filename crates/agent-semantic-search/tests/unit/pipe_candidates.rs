use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    SearchPipeAutoAcquisitionRequest, SearchPipeCandidateRequest,
    SearchPipeSourceIndexAcquisitionRequest, SearchPipeSourceIndexCandidate,
    SearchPipeSourceIndexDecision, SearchPipeSourceIndexLookup,
    collect_search_pipe_auto_acquisition, collect_search_pipe_candidates,
    collect_search_pipe_source_index_acquisition, failure_candidate_query,
};

#[test]
fn pipe_candidates_collect_dynamic_overlay_for_non_path_query() {
    let root = temp_root("asp-pipe-candidates-dynamic");
    let src = root.join("src");
    fs::create_dir_all(&src).expect("create source directory");
    fs::write(
        src.join("pipe_owner.rs"),
        "pub fn pipe_candidate_owner() { let dynamic_overlay = true; }\n",
    )
    .expect("write rust fixture");

    let ignore_dirs = vec!["target".to_string()];
    let include_hidden_dirs = Vec::new();
    let owners = vec![std::path::PathBuf::from("src/pipe_owner.rs")];
    let fixture = crate::source_snapshot_fixture::canonical_test_snapshot();
    let collection = collect_search_pipe_candidates(SearchPipeCandidateRequest {
        language_id: "rust",
        project_root: &root,
        locator_root: &root,
        query: "pipe_candidate",
        owners: &owners,
        ignore_dirs: &ignore_dirs,
        include_hidden_dirs: &include_hidden_dirs,
        base_snapshot: &fixture.workspace,
        provider_digest: fixture.provider_digest.as_str(),
        limit: 16,
        require_multi_clause: false,
    })
    .expect("collect pipe candidates");
    let candidates = collection.candidates;

    assert!(
        candidates
            .iter()
            .any(|candidate| candidate.path == "src/pipe_owner.rs"
                && candidate.source == "search-overlay")
    );
    assert!(candidates.iter().all(|candidate| {
        candidate.line == 1 && candidate.end_line == 1 && !candidate.path.contains(":1:1")
    }));

    fs::remove_dir_all(root).expect("remove fixture");
}

#[test]
fn pipe_candidates_reject_empty_query_before_any_route() {
    let root = temp_root("asp-pipe-candidates-empty");
    let ignore_dirs = Vec::new();
    let include_hidden_dirs = Vec::new();
    let fixture = crate::source_snapshot_fixture::canonical_test_snapshot();
    let error = collect_search_pipe_candidates(SearchPipeCandidateRequest {
        language_id: "rust",
        project_root: &root,
        locator_root: &root,
        query: "  \t\n",
        owners: &[],
        ignore_dirs: &ignore_dirs,
        include_hidden_dirs: &include_hidden_dirs,
        base_snapshot: &fixture.workspace,
        provider_digest: fixture.provider_digest.as_str(),
        limit: 16,
        require_multi_clause: false,
    })
    .expect_err("empty query should fail before collection");

    assert_eq!(error, "search pipe requires a non-empty query");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn source_index_acquisition_gates_broad_generic_queries() {
    let terms = crate::query_pack_fixture::with_typescript_query_pack("rust", |descriptor| {
        crate::search_pipe_typed_query_terms(
            crate::search_pipe_query_pack::SearchPipeLanguageId("rust"),
            crate::search_pipe_query_pack::SearchPipeQueryText(
                "search query budget block generic provider",
            ),
            descriptor,
        )
    });
    let gate =
        crate::search_pipe_source_index_query_gate(&terms).expect("generic query should be gated");
    assert_eq!(gate.term_count, 6);
    assert_eq!(gate.generic_term_count, 6);
}

#[test]
fn auto_acquisition_query_gate_does_not_claim_an_unpublished_generation() {
    let fixture = crate::source_snapshot_fixture::canonical_test_snapshot();
    let query = "search query budget block generic provider";
    let terms = crate::query_pack_fixture::with_typescript_query_pack("rust", |descriptor| {
        crate::search_pipe_typed_query_terms(
            crate::search_pipe_query_pack::SearchPipeLanguageId("rust"),
            crate::search_pipe_query_pack::SearchPipeQueryText(query),
            descriptor,
        )
    });

    let acquisition = collect_search_pipe_auto_acquisition(SearchPipeAutoAcquisitionRequest {
        language_id: "rust",
        project_root: std::path::Path::new("."),
        locator_root: std::path::Path::new("."),
        query,
        query_terms: &terms,
        owners: &[],
        ignore_dirs: &[],
        include_hidden_dirs: &[],
        base_snapshot: Some(&fixture.workspace),
        base_source_snapshot: &fixture.evidence,
        provider_digest: fixture.provider_digest.as_str(),
        require_multi_clause: false,
        limit: 5,
        source_index_lookup: None,
    })
    .expect("query gate should remain a zero-I/O admission result");

    assert_eq!(acquisition.source_snapshot, None);
    assert_eq!(acquisition.candidate_sources, vec!["query-gate"]);
}

#[test]
fn source_index_acquisition_defers_backend_for_path_like_miss() {
    let snapshot = crate::source_snapshot_fixture::canonical_test_snapshot();
    let index_artifact_digest =
        agent_semantic_search::source_index_artifact_digest(&snapshot.evidence);
    let lookup = SearchPipeSourceIndexLookup {
        source_snapshot: Some(snapshot.evidence.clone()),
        index_artifact_digest: Some((index_artifact_digest.clone()).into()),
        state: ("miss".to_string()).into(),
        candidates: Vec::new(),
    };

    let acquisition =
        collect_search_pipe_source_index_acquisition(SearchPipeSourceIndexAcquisitionRequest {
            intent: "crates/agent-semantic-search/src/pipe_source.rs",
            project_root: std::path::Path::new("."),
            scopes: &[],
            lookup: Some(&lookup),
        })
        .expect("path-like miss should produce source-index acquisition");

    assert_eq!(
        acquisition.decision,
        SearchPipeSourceIndexDecision::DeferBackend
    );
    assert!(acquisition.candidates.is_empty());
}

#[test]
fn source_index_acquisition_quarantines_stale_candidates_and_defers_overlay() {
    let root = std::env::temp_dir().join(format!("asp-source-index-drift-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create drift fixture root");
    let snapshot = crate::source_snapshot_fixture::canonical_test_snapshot();
    let index_artifact_digest =
        agent_semantic_search::source_index_artifact_digest(&snapshot.evidence);
    let lookup = SearchPipeSourceIndexLookup {
        source_snapshot: Some(snapshot.evidence.clone()),
        index_artifact_digest: Some((index_artifact_digest.clone()).into()),
        state: ("hit".to_string()).into(),
        candidates: vec![SearchPipeSourceIndexCandidate {
            path: ("crates/agent-semantic-client/src/search_pipe_source.rs".to_string()).into(),
            language_id: Some(("rust".to_string()).into()),
            provider_id: Some(("rs-harness".to_string()).into()),
            source_kind: ("file".to_string()).into(),
            line_count: Some(42),
            query_keys: vec![("source_index_owner".to_string()).into()],
            selector_projection: None,
        }],
    };

    let acquisition =
        collect_search_pipe_source_index_acquisition(SearchPipeSourceIndexAcquisitionRequest {
            intent: "source_index_owner",
            project_root: &root,
            scopes: &[],
            lookup: Some(&lookup),
        })
        .expect("hit should produce source-index acquisition");

    assert_eq!(
        acquisition.decision,
        SearchPipeSourceIndexDecision::DeferBackend
    );
    assert_eq!(acquisition.candidates.len(), 1);
    let candidate = &acquisition.candidates[0];
    assert_eq!(
        candidate.path,
        "crates/agent-semantic-client/src/search_pipe_source.rs"
    );
    assert_eq!(candidate.end_line, 42);
    assert_eq!(candidate.symbol, "source_index_owner");
    assert_eq!(candidate.source, "source-index");
    assert_eq!(candidate.confidence, "stale-index");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn source_index_acquisition_keeps_existing_rows_inventory_only() {
    let root =
        std::env::temp_dir().join(format!("asp-source-index-inventory-{}", std::process::id()));
    let source_dir = root.join("src");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&source_dir).expect("create inventory fixture source dir");
    std::fs::write(source_dir.join("lib.rs"), "pub fn current_owner() {}\n")
        .expect("write inventory fixture source");
    let snapshot = crate::source_snapshot_fixture::canonical_test_snapshot();
    let index_artifact_digest =
        agent_semantic_search::source_index_artifact_digest(&snapshot.evidence);
    let lookup = SearchPipeSourceIndexLookup {
        source_snapshot: Some(snapshot.evidence.clone()),
        index_artifact_digest: Some((index_artifact_digest.clone()).into()),
        state: ("hit".to_string()).into(),
        candidates: vec![SearchPipeSourceIndexCandidate {
            path: ("src/lib.rs".to_string()).into(),
            language_id: Some(("rust".to_string()).into()),
            provider_id: Some(("rs-harness".to_string()).into()),
            source_kind: ("file".to_string()).into(),
            line_count: Some(1),
            query_keys: vec![("current_owner".to_string()).into()],
            selector_projection: None,
        }],
    };

    let acquisition =
        collect_search_pipe_source_index_acquisition(SearchPipeSourceIndexAcquisitionRequest {
            intent: "current_owner",
            project_root: &root,
            scopes: &[],
            lookup: Some(&lookup),
        })
        .expect("hit should produce source-index acquisition");

    assert_eq!(
        acquisition.decision,
        SearchPipeSourceIndexDecision::DeferBackend
    );
    assert_eq!(acquisition.candidates.len(), 1);
    let candidate = &acquisition.candidates[0];
    assert_eq!(candidate.path, "src/lib.rs");
    assert_eq!(candidate.source, "source-index");
    assert_eq!(candidate.confidence, "inventory-only");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn failure_candidate_query_extracts_structural_terms_without_noise() {
    let query = failure_candidate_query(
        "expected left failure in foo_bar::inner-owner but observed file_hash mismatch",
    );

    assert_eq!(query, "inner-owner");
}

fn temp_root(prefix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "{prefix}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ))
}
