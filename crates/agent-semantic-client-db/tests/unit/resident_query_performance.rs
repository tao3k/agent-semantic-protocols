use agent_semantic_client_db::resident_query_performance::{
    ResidentQueryConcurrency, ResidentQueryIoCounters, ResidentQueryPerformanceReceipt,
    ResidentQueryPhase, ResidentWorkspaceReference,
};

fn concurrency() -> ResidentQueryConcurrency {
    let parallelism = std::thread::available_parallelism()
        .map(|parallelism| parallelism.get() as u32)
        .unwrap_or(2);
    ResidentQueryConcurrency {
        workspace_count: parallelism.div_ceil(2).clamp(2, 32),
        session_count: parallelism.saturating_mul(4),
        request_count: parallelism.saturating_mul(64),
    }
}

#[test]
fn zero_io_receipt_accepts_sub_millisecond_query() {
    let receipt = ResidentQueryPerformanceReceipt::zero_io(
        ResidentWorkspaceReference::WorkspaceId {
            workspace_id: "workspace-test".to_owned(),
        },
        format!("blake3-256:{}", "a".repeat(64)),
        ResidentQueryPhase::ColdSelectorCacheMiss,
        999,
        concurrency(),
    )
    .expect("sub-millisecond zero-I/O receipt");

    assert_eq!(receipt.io, ResidentQueryIoCounters::ZERO);
    assert_eq!(receipt.root_depth, [1, 0]);
}

#[test]
fn receipt_rejects_query_over_budget() {
    let error = ResidentQueryPerformanceReceipt::zero_io(
        ResidentWorkspaceReference::WorkspaceId {
            workspace_id: "workspace-test".to_owned(),
        },
        format!("blake3-256:{}", "a".repeat(64)),
        ResidentQueryPhase::WarmExactQuery,
        1_001,
        concurrency(),
    )
    .expect_err("query over budget must fail");

    assert!(error.contains("exceeded 1000us budget"));
}

#[test]
fn receipt_rejects_nonzero_query_io() {
    let mut receipt = ResidentQueryPerformanceReceipt::zero_io(
        ResidentWorkspaceReference::WorkspaceId {
            workspace_id: "workspace-test".to_owned(),
        },
        format!("blake3-256:{}", "a".repeat(64)),
        ResidentQueryPhase::WarmExactQuery,
        1,
        concurrency(),
    )
    .expect("baseline receipt");
    receipt.io.db_opens = 1;

    assert_eq!(
        receipt
            .validate()
            .expect_err("query-time DB open must fail"),
        "resident query data plane performed forbidden I/O"
    );
}

#[test]
fn receipt_serializes_shared_schema_field_names() {
    let receipt = ResidentQueryPerformanceReceipt::zero_io(
        ResidentWorkspaceReference::CheckoutRoot {
            workspace_root: "/workspace/repository".to_owned(),
        },
        format!("blake3-256:{}", "a".repeat(64)),
        ResidentQueryPhase::WarmExactQuery,
        1,
        concurrency(),
    )
    .expect("serializable receipt");
    let value = serde_json::to_value(receipt).expect("serialize receipt");

    assert_eq!(value["schemaId"], "asp.resident-query-performance-receipt");
    assert_eq!(value["workspace"]["kind"], "checkout-root");
    assert_eq!(value["workspace"]["workspaceRoot"], "/workspace/repository");
    assert_eq!(value["rootDepth"], serde_json::json!([1, 0]));
    assert_eq!(value["io"]["dbOpens"], 0);
}
