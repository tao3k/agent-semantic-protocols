use super::{
    GRAPH_TURBO_RESIDENT_SERVER_SCHEMA_ID, GRAPH_TURBO_RESIDENT_SERVER_SCHEMA_VERSION,
    GraphTurboMessageKind, GraphTurboRankedNode, GraphTurboResidentReceipt,
    GraphTurboResidentRequest, GraphTurboServerState,
};

fn digest(byte: char) -> String {
    format!("blake3-256:{}", byte.to_string().repeat(64))
}

fn request() -> GraphTurboResidentRequest {
    GraphTurboResidentRequest {
        schema_id: GRAPH_TURBO_RESIDENT_SERVER_SCHEMA_ID.to_owned(),
        schema_version: GRAPH_TURBO_RESIDENT_SERVER_SCHEMA_VERSION.to_owned(),
        message_kind: GraphTurboMessageKind::Rank,
        request_id: 7,
        workspace_identity: "workspace-a".to_owned(),
        generation_digest: digest('a'),
        protocol_digest: Some(digest('b')),
        page_roots: [("graph-relations".to_owned(), digest('c'))]
            .into_iter()
            .collect(),
        terms: vec!["runtime".to_owned(), "lifecycle".to_owned()],
        profile: Some("typed-ppr-diverse".to_owned()),
        budget: Some(10),
        deadline_unix_micros: Some(10_000),
        rank_payload: Some(serde_json::json!({"graph": {"nodes": [], "edges": []}})),
        generation_payload: None,
    }
}

#[test]
fn request_requires_exact_workspace_generation_and_v1_contract() {
    let request = request();
    request.validate().expect("current request validates");

    let mut invalid = request.clone();
    invalid.workspace_identity.clear();
    assert!(invalid.validate().is_err());

    let mut invalid = request;
    invalid.generation_digest = "generation-7".to_owned();
    assert!(invalid.validate().is_err());
}

#[test]
fn receipt_rejects_cross_workspace_and_cross_generation_results() {
    let request = request();
    let mut receipt = GraphTurboResidentReceipt {
        schema_id: GRAPH_TURBO_RESIDENT_SERVER_SCHEMA_ID.to_owned(),
        schema_version: GRAPH_TURBO_RESIDENT_SERVER_SCHEMA_VERSION.to_owned(),
        message_kind: GraphTurboMessageKind::Receipt,
        request_id: request.request_id,
        workspace_identity: request.workspace_identity.clone(),
        generation_digest: request.generation_digest.clone(),
        state: GraphTurboServerState::Completed,
        reason_kind: None,
        ranked_nodes: vec![GraphTurboRankedNode {
            node_id: "owner:runtime".to_owned(),
            score: 0.91,
            evidence_digest: digest('d'),
        }],
        process_spawns: 0,
        generation_loads: 0,
        graph_page_reads: 3,
    };
    receipt
        .validate_for(&request)
        .expect("matching receipt validates");

    receipt.workspace_identity = "workspace-b".to_owned();
    assert!(receipt.validate_for(&request).is_err());
    receipt.workspace_identity = request.workspace_identity.clone();
    receipt.generation_digest = digest('e');
    assert!(receipt.validate_for(&request).is_err());
}

#[test]
fn v1_rank_fixtures_bind_generation_and_require_warm_reuse() {
    let request: GraphTurboResidentRequest = serde_json::from_str(include_str!(
        "../../../../schemas/fixtures/graph-turbo-resident-ipc/v1/rank-request.json"
    ))
    .expect("decode v1 rank request fixture");
    request.validate().expect("v1 rank request validates");

    let receipt: GraphTurboResidentReceipt = serde_json::from_str(include_str!(
        "../../../../schemas/fixtures/graph-turbo-resident-ipc/v1/rank-receipt.json"
    ))
    .expect("decode v1 rank receipt fixture");
    receipt
        .validate_for(&request)
        .expect("v1 rank receipt preserves request identity");
    assert_eq!(
        receipt.process_spawns, 0,
        "warm IPC reuse must not spawn Python"
    );
    assert_eq!(
        receipt.generation_loads, 0,
        "warm IPC reuse must not reload the generation"
    );

    let mut root_substitution = receipt;
    root_substitution.generation_digest = request
        .page_roots
        .get("owners")
        .expect("fixture page root")
        .clone();
    assert!(
        root_substitution.validate_for(&request).is_err(),
        "a graph page/root digest must never substitute for generationDigest"
    );
}

#[test]
fn memory_search_performance_receipt_is_json_value_safe_for_v1_ipc() {
    let receipt = crate::MemorySearchPerformanceReceipt {
        generation_load_micros: u64::MAX,
        index_lookup_micros: u64::MAX,
        candidate_count: 1,
        source_bytes_materialized: 0,
        db_opens: 0,
        db_queries: 0,
        provider_subprocesses: 0,
        cache_writes: 0,
    };
    let value = serde_json::to_value(receipt)
        .expect("v1 IPC telemetry must not contain serde_json-unsupported u128 values");
    assert_eq!(value["generationLoadMicros"], serde_json::json!(u64::MAX));
    assert_eq!(value["indexLookupMicros"], serde_json::json!(u64::MAX));
}
