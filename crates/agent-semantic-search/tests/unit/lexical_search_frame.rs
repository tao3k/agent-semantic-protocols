use agent_semantic_search::{
    LexicalAcquisitionRoute, LexicalEvidenceState, LexicalSearchFrameCandidate,
    LexicalSearchFrameRequest, SearchPipeAutoAcquisitionRequest, SearchPipeSelectorPayloadProof,
    SearchPipeSourceIndexCandidate, SearchPipeSourceIndexLookup,
    collect_search_pipe_auto_acquisition, plan_lexical_search_frame,
};
use std::{
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

const LEXICAL_FRAME_SCENARIO_ID: &str = "lexical-search-frame-graph-router-warm-path";
const LEXICAL_FRAME_SCENARIO_ROOT: &str =
    "tests/unit/scenarios/lexical_search_frame_graph_router_warm_path";

#[test]
fn lexical_search_frame_requires_query_bundle_before_warm_acquisition() {
    let terms = vec!["render_dynamic_owner_items_frontier".to_string()];
    let warm = vec![LexicalSearchFrameCandidate {
        path: "crates/agent-semantic-search/src/dynamic_search/owner_items/core.rs".to_string(),
        symbol: "render_dynamic_owner_items_frontier".to_string(),
        source: "turso-overlay".to_string(),
    }];

    let route = plan_lexical_search_frame(LexicalSearchFrameRequest {
        terms: &terms,
        warm_candidates: &warm,
        session_candidates: &[],
        owner_candidates: &[],
        provider_owner_item_available: true,
        cold_scan_allowed: true,
    });

    assert_eq!(
        route.acquisition_route,
        LexicalAcquisitionRoute::QueryBundleRequired
    );
    assert_eq!(route.evidence_state, LexicalEvidenceState::Degraded);
    assert_eq!(route.fallback_reason, "query-bundle-required");
    assert_eq!(route.provider_process_count, 0);
    assert_eq!(route.native_finder_process_count, 0);
    assert_eq!(route.selected_candidate_count, 0);
    assert_eq!(
        route.query_relation,
        agent_semantic_search::LexicalQueryRelation::QueryBundleRequired
    );
    assert!(
        route
            .render_receipt()
            .contains("acquisitionRoute=query-bundle-required")
    );
}

#[test]
fn lexical_search_frame_routes_multi_seed_bundle_by_cohesive_owner() {
    let terms = vec![
        "manifest_path".to_string(),
        "try_map_resources".to_string(),
        "plugin_root".to_string(),
    ];
    let warm = vec![
        LexicalSearchFrameCandidate {
            path: "plugin/src/provider.rs".to_string(),
            symbol: "manifest_path".to_string(),
            source: "turso-overlay".to_string(),
        },
        LexicalSearchFrameCandidate {
            path: "plugin/src/provider.rs".to_string(),
            symbol: "try_map_resources".to_string(),
            source: "turso-overlay".to_string(),
        },
        LexicalSearchFrameCandidate {
            path: "plugin/src/provider.rs".to_string(),
            symbol: "plugin_root".to_string(),
            source: "turso-overlay".to_string(),
        },
        LexicalSearchFrameCandidate {
            path: "plugin/src/manifest.rs".to_string(),
            symbol: "manifest_path".to_string(),
            source: "turso-overlay".to_string(),
        },
    ];

    let route = plan_lexical_search_frame(LexicalSearchFrameRequest {
        terms: &terms,
        warm_candidates: &warm,
        session_candidates: &[],
        owner_candidates: &[],
        provider_owner_item_available: false,
        cold_scan_allowed: true,
    });

    assert_eq!(
        route.query_relation,
        agent_semantic_search::LexicalQueryRelation::Cohesive
    );
    assert_eq!(route.query_bundle_count, 3);
    assert_eq!(route.covered_seed_count, 3);
    assert_eq!(route.cohesive_owner_count, 1);
    assert!(route.render_receipt().contains("queryBundle=3"));
    assert!(route.render_receipt().contains("queryRelation=cohesive"));
    assert!(route.render_receipt().contains("coveredSeeds=3"));
    assert!(route.render_receipt().contains("cohesiveOwnerCount=1"));
}

#[test]
fn lexical_search_frame_marks_degraded_finder_as_last_resort() {
    let terms = vec!["missing".to_string(), "owner".to_string()];
    let route = plan_lexical_search_frame(LexicalSearchFrameRequest {
        terms: &terms,
        warm_candidates: &[],
        session_candidates: &[],
        owner_candidates: &[],
        provider_owner_item_available: false,
        cold_scan_allowed: false,
    });

    assert_eq!(
        route.acquisition_route,
        LexicalAcquisitionRoute::DegradedFinder
    );
    assert_eq!(route.evidence_state, LexicalEvidenceState::Degraded);
    assert_eq!(route.fallback_reason, "no-parser-facts");
    assert_eq!(route.native_finder_process_count, 1);
}

#[test]
fn lexical_search_frame_uses_owner_evidence_before_cold_scan() {
    let terms = vec!["DynamicOwnerItem".to_string(), "owner_items".to_string()];
    let owners = vec![LexicalSearchFrameCandidate {
        path: "crates/agent-semantic-search/src/dynamic_search/owner_items/core.rs".to_string(),
        symbol: String::new(),
        source: "source-index".to_string(),
    }];

    let route = plan_lexical_search_frame(LexicalSearchFrameRequest {
        terms: &terms,
        warm_candidates: &[],
        session_candidates: &[],
        owner_candidates: &owners,
        provider_owner_item_available: true,
        cold_scan_allowed: true,
    });

    assert_eq!(
        route.acquisition_route,
        LexicalAcquisitionRoute::SourceIndexOwnerEvidence
    );
    assert_eq!(route.evidence_state, LexicalEvidenceState::OwnerReady);
    assert_eq!(route.fallback_reason, "selector-proof-missing");
    assert_eq!(route.provider_process_count, 0);
    assert_eq!(route.native_finder_process_count, 0);
}

#[test]
fn lexical_search_frame_warm_path_stays_inside_scenario_gate() {
    let scenario_root = Path::new(env!("CARGO_MANIFEST_DIR")).join(LEXICAL_FRAME_SCENARIO_ROOT);
    let scenario = fs::read_to_string(scenario_root.join("scenario.toml"))
        .expect("read lexical frame scenario manifest");
    let benchmark = fs::read_to_string(scenario_root.join("benchmark.toml"))
        .expect("read lexical frame benchmark manifest");
    assert!(scenario.contains(&format!("id = \"{LEXICAL_FRAME_SCENARIO_ID}\"")));
    for expected in [
        "SEARCH-AGENT-ASP-PERF-LEXICAL-FRAME-WARM-001",
        "GraphRouter",
        "plan_lexical_search_frame",
        "phase = \"warm\"",
        "route_source = \"lexical-search-frame\"",
        "graph_router = \"lexical-v1\"",
        "max_provider_process_count = 0",
        "max_native_finder_process_count = 0",
        "fallback_reason = \"none\"",
    ] {
        assert!(
            scenario.contains(expected) || benchmark.contains(expected),
            "lexical frame scenario benchmark missing {expected:?}"
        );
    }

    let terms = vec![
        "render_dynamic_owner_items_frontier".to_string(),
        "DynamicOwnerItem".to_string(),
    ];
    let warm = vec![
        LexicalSearchFrameCandidate {
            path: "crates/agent-semantic-search/src/dynamic_search/owner_items/core.rs".to_string(),
            symbol: "render_dynamic_owner_items_frontier".to_string(),
            source: "turso-overlay".to_string(),
        },
        LexicalSearchFrameCandidate {
            path: "crates/agent-semantic-search/src/dynamic_search/owner_items/core.rs".to_string(),
            symbol: "DynamicOwnerItem".to_string(),
            source: "turso-overlay".to_string(),
        },
    ];

    let started = Instant::now();
    for _ in 0..3 {
        let route = plan_lexical_search_frame(LexicalSearchFrameRequest {
            terms: &terms,
            warm_candidates: &warm,
            session_candidates: &[],
            owner_candidates: &[],
            provider_owner_item_available: true,
            cold_scan_allowed: true,
        });
        assert_eq!(
            route.acquisition_route,
            LexicalAcquisitionRoute::WarmOverlay
        );
        assert_eq!(route.fallback_reason, "none");
        assert_eq!(route.provider_process_count, 0);
        assert_eq!(route.native_finder_process_count, 0);
    }
    assert!(
        started.elapsed() <= std::time::Duration::from_millis(50),
        "warm lexical SearchFrame routing exceeded scenario max_total"
    );
}

#[test]
fn lexical_search_frame_trace_skips_overlay_when_source_index_is_selector_ready() {
    let fixture = crate::source_snapshot_fixture::canonical_test_snapshot();
    let project_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lookup = SearchPipeSourceIndexLookup {
        state: "hit".to_string(),
        candidates: vec![SearchPipeSourceIndexCandidate {
            path: "src/lexical_search_frame.rs".to_string(),
            language_id: Some("rust".to_string()),
            provider_id: Some("rs-harness".to_string()),
            source_kind: "source".to_string(),
            line_count: Some(120),
            query_keys: vec![
                "agent-semantic-search".to_string(),
                "LexicalSearchFrameRequest".to_string(),
                "plan_lexical_search_frame".to_string(),
            ],
            selector_proof: Some(SearchPipeSelectorPayloadProof {
                structural_selector:
                    "rust://src/lexical_search_frame.rs#item/struct/LexicalSearchFrameRequest"
                        .to_string(),
                payload_kind: "code".to_string(),
                bounded: true,
            }),
        }],
    };
    let owners: Vec<PathBuf> = Vec::new();
    let ignore_dirs: Vec<String> = Vec::new();
    let include_hidden_dirs: Vec<String> = Vec::new();
    let query = "LexicalSearchFrameRequest plan_lexical_search_frame";
    let query_terms = crate::query_pack_fixture::with_typescript_query_pack("rust", |descriptor| {
        agent_semantic_search::search_pipe_typed_query_terms("rust", query, descriptor)
    });

    let acquisition = collect_search_pipe_auto_acquisition(SearchPipeAutoAcquisitionRequest {
        language_id: "rust",
        project_root,
        locator_root: project_root,
        query,
        query_terms: &query_terms,
        owners: &owners,
        ignore_dirs: &ignore_dirs,
        include_hidden_dirs: &include_hidden_dirs,
        require_multi_clause: false,
        limit: 5,
        source_index_lookup: Some(&lookup),
        base_snapshot: &fixture.workspace,
        provider_digest: fixture.provider_digest.as_str(),
    })
    .expect("source-index warm lexical frame acquisition");

    assert_eq!(acquisition.candidate_sources, vec!["source-index"]);
    assert!(acquisition.source_trace.iter().any(|trace| {
        trace.source == "lexical-search-frame"
            && trace.status.contains("searchFrame=lexical")
            && trace.status.contains("acquisitionRoute=warm-overlay")
            && trace.status.contains("graphRouter=lexical-v1")
    }));
    assert!(
        acquisition
            .source_trace
            .iter()
            .any(|trace| { trace.source == "search-overlay" && trace.status == "skipped" })
    );
}

#[test]
fn lexical_search_frame_uses_source_index_owner_evidence_before_overlay() {
    let fixture = crate::source_snapshot_fixture::canonical_test_snapshot();
    let project_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lookup = SearchPipeSourceIndexLookup {
        state: "hit".to_string(),
        candidates: vec![SearchPipeSourceIndexCandidate {
            path: "src/lexical_search_frame.rs".to_string(),
            language_id: Some("rust".to_string()),
            provider_id: Some("rs-harness".to_string()),
            source_kind: "source".to_string(),
            line_count: Some(120),
            query_keys: vec![
                "LexicalSearchFrameRequest".to_string(),
                "plan_lexical_search_frame".to_string(),
            ],
            selector_proof: None,
        }],
    };
    let owners: Vec<PathBuf> = Vec::new();
    let ignore_dirs: Vec<String> = Vec::new();
    let include_hidden_dirs: Vec<String> = Vec::new();
    let query = "LexicalSearchFrameRequest plan_lexical_search_frame";
    let query_terms = crate::query_pack_fixture::with_typescript_query_pack("rust", |descriptor| {
        agent_semantic_search::search_pipe_typed_query_terms("rust", query, descriptor)
    });

    let acquisition = collect_search_pipe_auto_acquisition(SearchPipeAutoAcquisitionRequest {
        language_id: "rust",
        project_root,
        locator_root: project_root,
        query,
        query_terms: &query_terms,
        owners: &owners,
        ignore_dirs: &ignore_dirs,
        include_hidden_dirs: &include_hidden_dirs,
        require_multi_clause: false,
        limit: 5,
        source_index_lookup: Some(&lookup),
        base_snapshot: &fixture.workspace,
        provider_digest: fixture.provider_digest.as_str(),
    })
    .expect("source-index owner evidence lexical frame acquisition");

    assert_eq!(acquisition.candidate_sources, vec!["source-index"]);
    assert_eq!(acquisition.candidates.len(), 1);
    assert_eq!(
        acquisition.candidates[0].symbol,
        "LexicalSearchFrameRequest"
    );
    assert_eq!(acquisition.candidates[0].confidence, "inventory-only");
    assert!(acquisition.source_trace.iter().any(|trace| {
        trace.source == "lexical-search-frame"
            && trace
                .status
                .contains("acquisitionRoute=source-index-owner-evidence")
            && trace.status.contains("evidenceState=owner-ready")
    }));
    assert!(
        acquisition
            .source_trace
            .iter()
            .any(|trace| { trace.source == "search-overlay" && trace.status == "skipped" })
    );
}
