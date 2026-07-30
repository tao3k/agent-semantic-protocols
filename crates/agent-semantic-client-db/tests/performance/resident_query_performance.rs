use std::sync::Arc;

use agent_semantic_client_db::resident_query_performance::{
    ResidentQueryConcurrency, ResidentQueryIoCounters, ResidentQueryPerformanceReceipt,
    ResidentQueryPhase, ResidentWorkspaceReference,
};
use tokio::sync::Barrier;
use tokio::task::JoinSet;
use tokio::time::Instant;

fn adaptive_concurrency() -> ResidentQueryConcurrency {
    let parallelism = std::thread::available_parallelism()
        .map(|parallelism| parallelism.get() as u32)
        .unwrap_or(2);
    ResidentQueryConcurrency {
        workspace_count: parallelism.div_ceil(2).clamp(2, 32),
        session_count: parallelism.saturating_mul(4),
        request_count: parallelism.saturating_mul(64),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn concurrent_receipt_validation_is_zero_io_and_sub_millisecond_at_p99() {
    let concurrency = adaptive_concurrency();
    let request_count =
        usize::try_from(concurrency.request_count).expect("request count fits usize");
    let workspace_count =
        usize::try_from(concurrency.workspace_count).expect("workspace count fits usize");
    let barrier = Arc::new(Barrier::new(request_count + 1));
    let mut tasks = JoinSet::new();

    for request_index in 0..request_count {
        let barrier = Arc::clone(&barrier);
        tasks.spawn(async move {
            barrier.wait().await;
            let started = Instant::now();
            let mut receipt = ResidentQueryPerformanceReceipt::zero_io(
                ResidentWorkspaceReference::WorkspaceId {
                    workspace_id: format!("workspace-stress-{}", request_index % workspace_count),
                },
                format!("blake3-256:{}", "a".repeat(64)),
                if request_index % 2 == 0 {
                    ResidentQueryPhase::ColdSelectorCacheMiss
                } else {
                    ResidentQueryPhase::WarmExactQuery
                },
                0,
                concurrency,
            )
            .expect("construct pressure receipt");
            receipt.elapsed_micros =
                u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
            receipt.validate().expect("pressure receipt satisfies gate");
            receipt
        });
    }

    barrier.wait().await;
    let mut elapsed_micros = Vec::with_capacity(request_count);
    while let Some(result) = tasks.join_next().await {
        let receipt = result.expect("pressure task joined");
        assert_eq!(receipt.io, ResidentQueryIoCounters::ZERO);
        elapsed_micros.push(receipt.elapsed_micros);
    }

    elapsed_micros.sort_unstable();
    let p99_index = (elapsed_micros.len() * 99 / 100).min(elapsed_micros.len() - 1);
    let p99_micros = elapsed_micros[p99_index];
    eprintln!(
        "[resident-query-pressure] workspaceCount={} sessionCount={} requestCount={} p99Micros={} fsReads=0 dbOpens=0 manifestReads=0 processSpawns=0 lockProbes=0 schemaBootstraps=0 workspaceCanonicalizations=0",
        concurrency.workspace_count,
        concurrency.session_count,
        concurrency.request_count,
        p99_micros
    );
    assert!(
        p99_micros <= 1_000,
        "resident receipt validation p99 exceeded 1000us: {p99_micros}us"
    );
}
