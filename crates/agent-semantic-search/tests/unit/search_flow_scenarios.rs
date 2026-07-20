use agent_semantic_search::{
    GraphCandidateHotNodesRequest, GraphCandidateItemNodesRequest, GraphProjectionCandidate,
    SearchPipeAutoAcquisitionRequest, SearchPipeSelectorPayloadProof,
    SearchPipeSourceIndexCandidate, SearchPipeSourceIndexLookup,
    collect_search_pipe_auto_acquisition, graph_candidate_hot_node_id, graph_candidate_hot_nodes,
    graph_candidate_item_node_id, graph_candidate_item_nodes, owner_path_graph_nodes,
    stable_graph_node_id,
};
use std::path::{Path, PathBuf};

#[test]
fn search_flow_source_index_owner_item_graph_chain_is_executable() {
    let fixture = crate::source_snapshot_fixture::canonical_test_snapshot();
    let project_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lookup = SearchPipeSourceIndexLookup {
        state: "hit".to_string(),
        candidates: vec![SearchPipeSourceIndexCandidate {
            path: "src/dynamic_search/owner_items/core.rs".to_string(),
            language_id: Some("rust".to_string()),
            provider_id: Some("rs-harness".to_string()),
            source_kind: "source".to_string(),
            line_count: Some(240),
            query_keys: vec![
                "agent-semantic-search".to_string(),
                "DynamicOwnerItem".to_string(),
                "render_dynamic_owner_items_frontier".to_string(),
            ],
            selector_proof: Some(SearchPipeSelectorPayloadProof {
                structural_selector:
                    "rust://src/dynamic_search/owner_items/core.rs#item/struct/DynamicOwnerItem"
                        .to_string(),
                payload_kind: "code".to_string(),
                bounded: true,
            }),
        }],
    };
    let owners: Vec<PathBuf> = Vec::new();
    let ignore_dirs: Vec<String> = Vec::new();
    let include_hidden_dirs: Vec<String> = Vec::new();
    let query = "render_dynamic_owner_items_frontier DynamicOwnerItem no-owner-item-match";
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
    .expect("source-index evidence graph chain acquisition");

    assert_eq!(acquisition.candidate_sources, vec!["source-index"]);
    assert_eq!(acquisition.candidates.len(), 1);
    let candidate = &acquisition.candidates[0];
    assert_eq!(candidate.path, "src/dynamic_search/owner_items/core.rs");
    assert_eq!(candidate.symbol, "DynamicOwnerItem");
    assert_eq!(candidate.confidence, "selector-ready");

    let owner_nodes = owner_path_graph_nodes(std::slice::from_ref(&candidate.path));
    let graph_candidate = GraphProjectionCandidate::new(
        candidate.path.clone(),
        candidate.line,
        candidate.end_line,
        candidate.symbol.clone(),
        candidate.text.clone(),
        candidate.source.clone(),
        candidate.confidence.clone(),
    );
    let item_nodes = graph_candidate_item_nodes(GraphCandidateItemNodesRequest::new(
        "rust",
        std::slice::from_ref(&graph_candidate),
        5,
    ));
    let hot_nodes = graph_candidate_hot_nodes(GraphCandidateHotNodesRequest::new(
        "rust",
        std::slice::from_ref(&graph_candidate),
        5,
    ));

    assert_eq!(owner_nodes.len(), 1);
    assert_eq!(item_nodes.len(), 1);
    assert_eq!(hot_nodes.len(), 1);

    let owner_id = stable_graph_node_id("owner", &candidate.path);
    let item_id = graph_candidate_item_node_id(&graph_candidate);
    let hot_id = graph_candidate_hot_node_id(&graph_candidate);

    assert_eq!(owner_nodes[0]["id"], owner_id);
    assert_eq!(item_nodes[0]["id"], item_id);
    assert_eq!(hot_nodes[0]["id"], hot_id);
    assert_eq!(item_nodes[0]["ownerPath"], candidate.path);
    assert_eq!(hot_nodes[0]["ownerPath"], candidate.path);
    assert_eq!(item_nodes[0]["candidateState"], "selector-ready");
    assert_eq!(item_nodes[0]["rankEligible"], true);
    assert_eq!(
        item_nodes[0]["structuralSelector"],
        "rust://src/dynamic_search/owner_items/core.rs#item/symbol/DynamicOwnerItem"
    );
    assert_eq!(
        hot_nodes[0]["structuralSelector"],
        "rust://src/dynamic_search/owner_items/core.rs#range/hot/DynamicOwnerItem"
    );
    assert!(
        !item_nodes[0]["structuralSelector"]
            .as_str()
            .expect("item structural selector")
            .contains(":1:")
    );
}

#[test]
fn search_flow_graph_router_prefers_exact_action_for_selector_ready_item() {
    let exact = agent_semantic_search::SearchActionSelection::for_first_action(
        agent_semantic_search::SearchEvidenceState::KnownSelector,
        "item-skeleton",
    );

    assert_eq!(
        exact.evidence_state,
        agent_semantic_search::SearchEvidenceState::KnownSelector
    );
    assert_eq!(exact.first_action_stage, "item-skeleton");
    assert!(exact.first_action_matches_evidence_state);
    assert!(exact.chosen_route_preconditions_met);
    assert_eq!(exact.unnecessary_seed_count, 0);
    assert_eq!(exact.seed_when_known_selector_count, 0);
    assert!(exact.allowed_first_stages.contains(&"item-skeleton"));
    assert!(exact.allowed_first_stages.contains(&"query-code"));
    assert!(exact.disallowed_first_stages.contains(&"seed"));
    assert!(exact.disallowed_first_stages.contains(&"prime"));

    let escaped = agent_semantic_search::SearchActionSelection::for_first_action(
        agent_semantic_search::SearchEvidenceState::KnownSelector,
        "seed",
    );

    assert!(!escaped.first_action_matches_evidence_state);
    assert!(!escaped.chosen_route_preconditions_met);
    assert_eq!(escaped.unnecessary_seed_count, 1);
    assert_eq!(escaped.seed_when_known_selector_count, 1);
}

#[test]
fn search_flow_subagent_receipt_is_compact_graph_route() {
    let item_selector =
        "rust://src/dynamic_search/owner_items/core.rs#item/symbol/DynamicOwnerItem";
    let receipt = agent_semantic_search::search_subagent_graph_route_receipt(
        "render_dynamic_owner_items_frontier DynamicOwnerItem no-owner-item-match",
        "source-index-owner-item-graph-chain",
        "known-selector",
        vec![serde_json::json!({
            "kind": "item",
            "owner": "src/dynamic_search/owner_items/core.rs",
            "selector": item_selector,
            "edge": "owner-contains-item"
        })],
        serde_json::json!({
            "action": "item-skeleton",
            "selector": item_selector
        }),
    );

    assert_eq!(
        receipt["schema"],
        agent_semantic_search::SEARCH_SUBAGENT_GRAPH_ROUTE_RECEIPT_SCHEMA
    );
    assert!(agent_semantic_search::search_subagent_graph_route_receipt_is_compact(&receipt));
    assert_eq!(receipt["next"]["action"], "item-skeleton");
    assert_eq!(receipt["next"]["selector"], item_selector);

    let mut with_confidence = receipt.clone();
    with_confidence["evidence"][0]["confidence"] = serde_json::json!("high");
    assert!(
        !agent_semantic_search::search_subagent_graph_route_receipt_is_compact(&with_confidence)
    );

    let mut with_line_range = receipt.clone();
    with_line_range["evidence"][0]["displayLineRange"] = serde_json::json!("12:24");
    assert!(
        !agent_semantic_search::search_subagent_graph_route_receipt_is_compact(&with_line_range)
    );

    let mut with_source_body = receipt;
    with_source_body["evidence"][0]["sourceBody"] = serde_json::json!("fn body() {}");
    assert!(
        !agent_semantic_search::search_subagent_graph_route_receipt_is_compact(&with_source_body)
    );
}

#[test]
fn search_flow_degraded_source_index_miss_uses_bounded_receipt_reason() {
    let terms = vec![
        "render_dynamic_owner_items_frontier".to_string(),
        "DynamicOwnerItem".to_string(),
        "no-owner-item-match".to_string(),
    ];
    let candidates: Vec<agent_semantic_search::LexicalSearchFrameCandidate> = Vec::new();

    let route = agent_semantic_search::plan_lexical_search_frame(
        agent_semantic_search::LexicalSearchFrameRequest {
            terms: &terms,
            warm_candidates: &candidates,
            session_candidates: &candidates,
            owner_candidates: &candidates,
            provider_owner_item_available: false,
            cold_scan_allowed: true,
        },
    );

    assert_eq!(
        route.acquisition_route,
        agent_semantic_search::LexicalAcquisitionRoute::BoundedColdScan
    );
    assert_eq!(
        route.evidence_state,
        agent_semantic_search::LexicalEvidenceState::NeedsColdScan
    );
    assert_eq!(route.fallback_reason, "warm-miss");
    assert_eq!(route.provider_process_count, 0);
    assert_eq!(route.native_finder_process_count, 0);
    assert_eq!(route.selected_candidate_count, 0);

    let receipt = agent_semantic_search::search_subagent_graph_route_receipt(
        "render_dynamic_owner_items_frontier DynamicOwnerItem no-owner-item-match",
        "bounded-cold-scan",
        "needs-cold-scan",
        vec![serde_json::json!({
            "kind": "route-miss",
            "source": "search-frame",
            "reason": route.fallback_reason,
            "bounded": true,
            "providerProcessCount": route.provider_process_count,
            "nativeFinderProcessCount": route.native_finder_process_count
        })],
        serde_json::json!({
            "action": "bounded-cold-scan",
            "reason": route.fallback_reason,
            "maxProviderProcessCount": 0,
            "maxNativeFinderProcessCount": 0
        }),
    );

    assert!(agent_semantic_search::search_subagent_graph_route_receipt_is_compact(&receipt));
    assert_eq!(receipt["route"], "bounded-cold-scan");
    assert_eq!(receipt["state"], "needs-cold-scan");
    assert_eq!(receipt["evidence"][0]["reason"], "warm-miss");
    assert_eq!(receipt["next"]["action"], "bounded-cold-scan");

    let mut polluted = receipt;
    polluted["evidence"][0]["commandLog"] = serde_json::json!(["asp rg ..."]);
    assert!(!agent_semantic_search::search_subagent_graph_route_receipt_is_compact(&polluted));
}

#[test]
fn search_flow_busy_source_index_miss_returns_overlay_skipped() {
    let fixture = crate::source_snapshot_fixture::canonical_test_snapshot();
    let project_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lookup = SearchPipeSourceIndexLookup {
        state: "busy".to_string(),
        candidates: Vec::new(),
    };
    let owners: Vec<PathBuf> = Vec::new();
    let ignore_dirs: Vec<String> = Vec::new();
    let include_hidden_dirs: Vec<String> = Vec::new();
    let query = "render_dynamic_owner_items_frontier DynamicOwnerItem no-owner-item-match";
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
    .expect("busy source-index should short-circuit without overlay");

    assert_eq!(acquisition.candidate_sources, vec!["source-index"]);
    assert!(acquisition.candidates.is_empty());
    assert!(
        acquisition
            .source_trace
            .iter()
            .any(|trace| trace.source == "sourceIndex" && trace.status == "busy")
    );
    assert!(
        acquisition
            .source_trace
            .iter()
            .any(|trace| trace.source == "search-overlay" && trace.status == "skipped")
    );
}

#[test]
fn search_flow_cold_required_source_index_returns_overlay_skipped() {
    let fixture = crate::source_snapshot_fixture::canonical_test_snapshot();
    let project_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lookup = SearchPipeSourceIndexLookup {
        state: "cold-required".to_string(),
        candidates: Vec::new(),
    };
    let owners: Vec<PathBuf> = Vec::new();
    let ignore_dirs: Vec<String> = Vec::new();
    let include_hidden_dirs: Vec<String> = Vec::new();
    let query = "source-index schema requires explicit rebuild SourceIndexLookup";
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
    .expect("cold-required source-index should short-circuit without overlay");

    assert_eq!(acquisition.candidate_sources, vec!["source-index"]);
    assert!(acquisition.candidates.is_empty());
    assert!(
        acquisition
            .source_trace
            .iter()
            .any(|trace| trace.source == "sourceIndex" && trace.status == "cold-required")
    );
    assert!(
        acquisition
            .source_trace
            .iter()
            .any(|trace| trace.source == "search-overlay" && trace.status == "skipped")
    );
}
