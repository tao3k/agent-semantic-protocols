use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use agent_semantic_client_db::turso_mvcc_partition::TursoMvccExpectedHead;
use agent_semantic_client_db::turso_mvcc_partition::TursoMvccPartitionCommit;
use agent_semantic_client_db::turso_mvcc_partition::TursoMvccPartitionCommitOutcome;
use agent_semantic_client_db::turso_mvcc_partition::TursoMvccPartitionRecord;
use agent_semantic_client_db::turso_mvcc_store::TursoMvccStore;
use agent_semantic_client_db::turso_mvcc_store::TursoMvccStoreConfig;

#[tokio::test]
async fn independent_partitions_commit_without_false_conflicts() {
    let store = Arc::new(open_store("different-partitions").await);
    initialize(&store, "run-a").await;
    initialize(&store, "run-b").await;

    let left_commit = advance("run-a", 1);
    let right_commit = advance("run-b", 1);
    let (left, right) = tokio::join!(
        store.compare_and_append_partition(&left_commit),
        store.compare_and_append_partition(&right_commit)
    );
    assert!(matches!(
        left.expect("run-a commit"),
        TursoMvccPartitionCommitOutcome::Committed(_)
    ));
    assert!(matches!(
        right.expect("run-b commit"),
        TursoMvccPartitionCommitOutcome::Committed(_)
    ));
    assert_eq!(
        store
            .read_partition_records("run-a")
            .await
            .expect("run-a records")
            .len(),
        1
    );
    assert_eq!(
        store
            .read_partition_records("run-b")
            .await
            .expect("run-b records")
            .len(),
        1
    );
}

#[tokio::test]
#[ignore = "performance receipt; run explicitly on the release host"]
async fn partitioned_mvcc_write_and_read_latency_receipt() {
    let partitions = scenario_usize("ASP_TURSO_MVCC_BENCH_PARTITIONS", 64);
    let max_write_p95_us = scenario_u128("ASP_TURSO_MVCC_BENCH_MAX_WRITE_P95_US", 100_000);
    let max_read_p95_us = scenario_u128("ASP_TURSO_MVCC_BENCH_MAX_READ_P95_US", 50_000);
    let store = open_store("partition-benchmark").await;

    for index in 0..partitions {
        initialize(&store, &format!("run-{index}")).await;
    }

    let mut write_latency_us = Vec::with_capacity(partitions);
    for index in 0..partitions {
        let started = Instant::now();
        let outcome = store
            .compare_and_append_partition(&advance(&format!("run-{index}"), index as i64 + 1))
            .await
            .expect("partition write");
        assert!(matches!(
            outcome,
            TursoMvccPartitionCommitOutcome::Committed(_)
        ));
        write_latency_us.push(started.elapsed().as_micros());
    }

    let mut read_latency_us = Vec::with_capacity(partitions);
    for index in 0..partitions {
        let started = Instant::now();
        let records = store
            .read_partition_records(&format!("run-{index}"))
            .await
            .expect("partition read");
        assert_eq!(records.len(), 1);
        read_latency_us.push(started.elapsed().as_micros());
    }

    let write_p95_us = percentile(&mut write_latency_us, 95);
    let read_p95_us = percentile(&mut read_latency_us, 95);
    eprintln!(
        "[turso-mvcc-benchmark] partitions={partitions} writeP95Us={write_p95_us} \
         readP95Us={read_p95_us} maxWriteP95Us={max_write_p95_us} \
         maxReadP95Us={max_read_p95_us}"
    );
    assert!(
        write_p95_us <= max_write_p95_us,
        "MVCC partition write p95 {write_p95_us}us exceeds {max_write_p95_us}us"
    );
    assert!(
        read_p95_us <= max_read_p95_us,
        "MVCC partition read p95 {read_p95_us}us exceeds {max_read_p95_us}us"
    );
}

async fn open_store(label: &str) -> TursoMvccStore {
    TursoMvccStore::open(TursoMvccStoreConfig {
        path: temp_database(label),
        connection_lanes: 4,
        passive_checkpoint: true,
        busy_timeout_ms: 5_000,
        retry_attempts: 16,
        max_batch_rows: 1_000,
    })
    .await
    .expect("open Turso MVCC store")
}

async fn initialize(store: &TursoMvccStore, partition_key: &str) {
    let outcome = store
        .compare_and_append_partition(&TursoMvccPartitionCommit {
            partition_key: partition_key.to_owned(),
            expected: None,
            next_revision: 0,
            next_head_digest: format!("{partition_key}-head-0"),
            next_projection: vec![0],
            records: Vec::new(),
            committed_at_ms: 0,
        })
        .await
        .expect("initialize partition");
    assert!(matches!(
        outcome,
        TursoMvccPartitionCommitOutcome::Committed(_)
    ));
}

fn advance(partition_key: &str, committed_at_ms: i64) -> TursoMvccPartitionCommit {
    TursoMvccPartitionCommit {
        partition_key: partition_key.to_owned(),
        expected: Some(TursoMvccExpectedHead {
            revision: 0,
            head_digest: format!("{partition_key}-head-0"),
            last_sequence: 0,
        }),
        next_revision: 1,
        next_head_digest: format!("{partition_key}-head-1"),
        next_projection: vec![1],
        records: vec![
            TursoMvccPartitionRecord::new(
                format!("{partition_key}-record-1"),
                "benchmark",
                vec![1],
            )
            .expect("valid benchmark record"),
        ],
        committed_at_ms,
    }
}

fn percentile(values: &mut [u128], percentile: usize) -> u128 {
    values.sort_unstable();
    let index = ((values.len() - 1) * percentile) / 100;
    values[index]
}

fn scenario_usize(name: &str, fallback: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|value| *value > 0)
        .unwrap_or(fallback)
}

fn scenario_u128(name: &str, fallback: u128) -> u128 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|value| *value > 0)
        .unwrap_or(fallback)
}

fn temp_database(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "agent-semantic-client-db-{label}-{}-{nonce}.turso",
        std::process::id()
    ))
}
