use std::path::PathBuf;
use std::process::Command;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use agent_semantic_client_db::artifact_pointer_store::{
    ClientDbArtifactPointerCasOutcome, ClientDbArtifactPointerCasRequest,
    ClientDbArtifactPointerKey, TursoArtifactPointerStore,
};
use agent_semantic_client_db::storage_contract::StorageRetryPolicy;
use agent_semantic_client_db::storage_performance_receipt::{
    STORAGE_SLO_MATRIX_RECEIPT_SCHEMA_ID, StorageLatencyDistributionMicros, StorageSloMatrixReceipt,
};
use agent_semantic_client_db::turso_cdc_storage::{
    TursoCdcCaptureMode, TursoCdcProfileConfig, TursoCdcStorage,
};
use agent_semantic_client_db::turso_mvcc_store::{
    TursoMvccEvent, TursoMvccStore, TursoMvccStoreConfig,
};
use agent_semantic_content_identity::hash_blob;
use turso::transaction::TransactionBehavior;

const LONG_INGESTION_ROWS: usize = 65_536;
const LONG_INGESTION_BATCH_ROWS: usize = 256;
const MIXED_PRESSURE_ITERATIONS: usize = 200;

fn temp_db(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "asp-client-db-slo-{name}-{}-{nonce}.turso",
        std::process::id()
    ))
}

fn artifact_key(iteration: usize) -> ClientDbArtifactPointerKey {
    ClientDbArtifactPointerKey::new(
        "repo:slo-matrix",
        "workspace:slo-matrix",
        "scope:slo-matrix",
        "topology-root",
        format!("iteration-{iteration}"),
    )
}

fn resident_set_kib() -> u64 {
    let output = Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .expect("query benchmark resident set");
    String::from_utf8(output.stdout)
        .expect("ps RSS is UTF-8")
        .trim()
        .parse()
        .expect("parse ps RSS in KiB")
}

fn file_len(path: &std::path::Path) -> u64 {
    std::fs::metadata(path)
        .map(|metadata| metadata.len())
        .unwrap_or(0)
}

fn main() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("SLO benchmark runtime");
    let mvcc_path = temp_db("mvcc");
    let store = runtime
        .block_on(TursoMvccStore::open(TursoMvccStoreConfig {
            path: mvcc_path.clone(),
            connection_lanes: 4,
            passive_checkpoint: true,
            busy_timeout_ms: 5_000,
            retry_attempts: 8,
            max_batch_rows: LONG_INGESTION_BATCH_ROWS,
        }))
        .expect("open passive-checkpoint MVCC SLO store");
    let retry_policy = StorageRetryPolicy::default();
    let payload = vec![0x6d_u8; 1_024];
    let mut long_latencies = Vec::with_capacity(LONG_INGESTION_ROWS / LONG_INGESTION_BATCH_ROWS);

    runtime.block_on(async {
        for batch in 0..(LONG_INGESTION_ROWS / LONG_INGESTION_BATCH_ROWS) {
            let events: Vec<_> = (0..LONG_INGESTION_BATCH_ROWS)
                .map(|offset| {
                    let sequence = batch * LONG_INGESTION_BATCH_ROWS + offset;
                    TursoMvccEvent {
                        partition_key: format!("partition-{}", sequence % 4),
                        event_id: format!("long-{sequence:08}"),
                        payload: payload.clone(),
                        created_at_ms: sequence as i64,
                    }
                })
                .collect();
            let started = Instant::now();
            store
                .append_batch_typed(&events, &retry_policy)
                .await
                .expect("append long-ingestion batch");
            long_latencies.push(started.elapsed().as_micros() as u64);
        }
    });

    let artifact_store = runtime
        .block_on(TursoArtifactPointerStore::open(temp_db("artifact")))
        .expect("open mixed-pressure artifact authority");
    let cdc = runtime
        .block_on(TursoCdcStorage::open(TursoCdcProfileConfig {
            path: temp_db("cdc"),
            mode: TursoCdcCaptureMode::Id,
            table_name: "asp_slo_cdc".to_owned(),
        }))
        .expect("open mixed-pressure CDC authority");
    let mut cdc_connection = cdc.connection();
    runtime
        .block_on(cdc_connection.execute(
            "CREATE TABLE mixed_cdc(id INTEGER PRIMARY KEY, payload BLOB NOT NULL)",
            (),
        ))
        .expect("create mixed-pressure CDC table");
    let mut mixed_latencies = Vec::with_capacity(MIXED_PRESSURE_ITERATIONS);
    let mut cdc_id = 0_i64;

    runtime.block_on(async {
        for iteration in 0..MIXED_PRESSURE_ITERATIONS {
            let events: Vec<_> = (0..32)
                .map(|offset| TursoMvccEvent {
                    partition_key: format!("mixed-partition-{}", offset % 4),
                    event_id: format!("mixed-{iteration:04}-{offset:02}"),
                    payload: payload.clone(),
                    created_at_ms: (LONG_INGESTION_ROWS + iteration * 32 + offset) as i64,
                })
                .collect();
            let new_root = hash_blob(format!("mixed-root-{iteration}").as_bytes()).to_string();
            let request = ClientDbArtifactPointerCasRequest {
                key: artifact_key(iteration),
                expected_root_hash: None,
                expected_revision: 0,
                new_root_hash: new_root,
                updated_at_ms: iteration as i64,
            };
            let cdc_start = cdc_id + 1;
            let started = Instant::now();
            let (mvcc_result, cas_result, cdc_result) = tokio::join!(
                store.append_batch_typed(&events, &retry_policy),
                artifact_store.compare_and_set(&request),
                async {
                    let transaction = cdc_connection
                        .transaction_with_behavior(TransactionBehavior::Immediate)
                        .await?;
                    let mut statement = transaction
                        .prepare_cached("INSERT INTO mixed_cdc(id, payload) VALUES (?1, ?2)")
                        .await?;
                    for offset in 0_i64..32 {
                        statement
                            .execute((cdc_start + offset, payload.as_slice()))
                            .await?;
                    }
                    drop(statement);
                    transaction.commit().await
                }
            );
            mvcc_result.expect("mixed-pressure MVCC batch");
            let cas = cas_result.expect("mixed-pressure artifact CAS");
            assert_eq!(cas.outcome, ClientDbArtifactPointerCasOutcome::Applied);
            cdc_result.expect("mixed-pressure CDC batch");
            cdc_id += 32;
            mixed_latencies.push(started.elapsed().as_micros() as u64);
        }
    });

    let maintenance = runtime
        .block_on(store.flush_and_measure())
        .expect("flush and measure passive-checkpoint store");
    drop(store);
    let reopened = runtime
        .block_on(TursoMvccStore::open(TursoMvccStoreConfig {
            path: mvcc_path.clone(),
            connection_lanes: 4,
            passive_checkpoint: true,
            busy_timeout_ms: 5_000,
            retry_attempts: 8,
            max_batch_rows: LONG_INGESTION_BATCH_ROWS,
        }))
        .expect("reopen long-ingestion store");
    let recovered_rows = runtime.block_on(async {
        let mut total = 0;
        for partition in 0..4 {
            total += reopened
                .read_partition(&format!("partition-{partition}"))
                .await
                .expect("read recovered long-ingestion partition")
                .len();
        }
        total
    });
    assert_eq!(recovered_rows, LONG_INGESTION_ROWS);

    let receipt = StorageSloMatrixReceipt {
        schema_id: STORAGE_SLO_MATRIX_RECEIPT_SCHEMA_ID.to_owned(),
        long_ingestion_rows: LONG_INGESTION_ROWS,
        long_ingestion_batch_rows: LONG_INGESTION_BATCH_ROWS,
        long_ingestion_latency_micros: StorageLatencyDistributionMicros::from_samples(
            &long_latencies,
        )
        .expect("long-ingestion latency samples"),
        recovered_rows,
        mixed_pressure_iterations: MIXED_PRESSURE_ITERATIONS,
        mixed_pressure_latency_micros: StorageLatencyDistributionMicros::from_samples(
            &mixed_latencies,
        )
        .expect("mixed-pressure latency samples"),
        resident_set_kib: resident_set_kib(),
        database_bytes: maintenance.database_bytes,
        wal_bytes: maintenance.wal_bytes,
        shm_bytes: file_len(&PathBuf::from(format!(
            "{}-shm",
            mvcc_path.to_string_lossy()
        ))),
        passive_checkpoint: maintenance.passive_checkpoint,
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&receipt).expect("serialize SLO matrix receipt")
    );
}
