use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    SearchPipeCandidateRequest, SearchPipeSourceIndexAcquisitionRequest,
    SearchPipeSourceIndexCandidate, SearchPipeSourceIndexDecision, SearchPipeSourceIndexLookup,
    collect_search_pipe_candidates, collect_search_pipe_source_index_acquisition,
    failure_candidate_query,
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
    let candidates = collect_search_pipe_candidates(SearchPipeCandidateRequest {
        language_id: "rust",
        project_root: &root,
        locator_root: &root,
        query: "pipe_candidate",
        owners: &[],
        ignore_dirs: &ignore_dirs,
        include_hidden_dirs: &include_hidden_dirs,
        limit: 16,
    })
    .expect("collect pipe candidates");

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
    let error = collect_search_pipe_candidates(SearchPipeCandidateRequest {
        language_id: "rust",
        project_root: &root,
        locator_root: &root,
        query: "  \t\n",
        owners: &[],
        ignore_dirs: &ignore_dirs,
        include_hidden_dirs: &include_hidden_dirs,
        limit: 16,
    })
    .expect_err("empty query should fail before collection");

    assert_eq!(error, "search pipe requires a non-empty query");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn source_index_acquisition_gates_broad_generic_queries() {
    let acquisition =
        collect_search_pipe_source_index_acquisition(SearchPipeSourceIndexAcquisitionRequest {
            intent: "search query route trace graph quality latency selector",
            project_root: std::path::Path::new("."),
            scopes: &[],
            lookup: None,
        })
        .expect("generic query should be gated");

    assert_eq!(
        acquisition.decision,
        SearchPipeSourceIndexDecision::QueryGate
    );
    let gate = acquisition.gate.expect("gate metadata");
    assert_eq!(gate.term_count, 8);
    assert_eq!(gate.generic_term_count, 8);
    assert!(acquisition.candidates.is_empty());
}

#[test]
fn source_index_acquisition_defers_backend_for_path_like_miss() {
    let lookup = SearchPipeSourceIndexLookup {
        state: "miss".to_string(),
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
    let lookup = SearchPipeSourceIndexLookup {
        state: "hit".to_string(),
        candidates: vec![SearchPipeSourceIndexCandidate {
            path: "crates/agent-semantic-client/src/search_pipe_source.rs".to_string(),
            language_id: Some("rust".to_string()),
            provider_id: Some("rs-harness".to_string()),
            source_kind: "file".to_string(),
            line_count: Some(42),
            query_keys: vec!["source_index_owner".to_string()],
            selector_proof: None,
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
    let lookup = SearchPipeSourceIndexLookup {
        state: "hit".to_string(),
        candidates: vec![SearchPipeSourceIndexCandidate {
            path: "src/lib.rs".to_string(),
            language_id: Some("rust".to_string()),
            provider_id: Some("rs-harness".to_string()),
            source_kind: "file".to_string(),
            line_count: Some(1),
            query_keys: vec!["current_owner".to_string()],
            selector_proof: None,
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
fn source_index_acquisition_uses_bounded_payload_proof_as_selector_ready() {
    let root = std::env::temp_dir().join(format!("asp-source-index-ready-{}", std::process::id()));
    let source_dir = root.join("src");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&source_dir).expect("create ready fixture source dir");
    std::fs::write(source_dir.join("lib.rs"), "pub fn current_owner() {}\n")
        .expect("write ready fixture source");
    let lookup = SearchPipeSourceIndexLookup {
        state: "hit".to_string(),
        candidates: vec![SearchPipeSourceIndexCandidate {
            path: "src/lib.rs".to_string(),
            language_id: Some("rust".to_string()),
            provider_id: Some("rs-harness".to_string()),
            source_kind: "file".to_string(),
            line_count: Some(1),
            query_keys: vec!["current_owner".to_string()],
            selector_proof: Some(SearchPipeSelectorPayloadProof {
                structural_selector: "rust://src/lib.rs#item/function/current_owner".to_string(),
                payload_kind: "code".to_string(),
                bounded: true,
            }),
        }],
    };

    let acquisition =
        collect_search_pipe_source_index_acquisition(SearchPipeSourceIndexAcquisitionRequest {
            intent: "current_owner",
            project_root: &root,
            scopes: &[],
            lookup: Some(&lookup),
        })
        .expect("hit with payload proof should produce source-index acquisition");

    assert_eq!(
        acquisition.decision,
        SearchPipeSourceIndexDecision::UseAndSkipSearchOverlay
    );
    assert_eq!(acquisition.candidates.len(), 1);
    let candidate = &acquisition.candidates[0];
    assert_eq!(candidate.path, "src/lib.rs");
    assert_eq!(candidate.source, "source-index");
    assert_eq!(candidate.confidence, "selector-ready");
    assert!(
        candidate.text.contains("payloadProof=code"),
        "{candidate:?}"
    );
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
use crate::pipe_source::SearchPipeSelectorPayloadProof;
