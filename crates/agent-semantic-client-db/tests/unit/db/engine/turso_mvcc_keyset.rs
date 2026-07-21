use agent_semantic_client_db::storage_contract::StorageRetryPolicy;
use agent_semantic_client_db::turso_mvcc_store::{
    TursoMvccEvent, TursoMvccStore, TursoMvccStoreConfig, TursoMvccWriteErrorCode,
};

struct TestDbDir(std::path::PathBuf);

impl TestDbDir {
    fn new(label: &str) -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "asp-turso-mvcc-keyset-{label}-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir(&path).expect("create temporary database directory");
        Self(path)
    }
}

impl Drop for TestDbDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn event(event_id: &str, created_at_ms: i64) -> TursoMvccEvent {
    TursoMvccEvent {
        partition_key: "keyset-partition".to_owned(),
        event_id: event_id.to_owned(),
        payload: event_id.as_bytes().to_vec(),
        created_at_ms,
    }
}

async fn open_store(label: &str, passive_checkpoint: bool) -> (TestDbDir, TursoMvccStore) {
    let temp = TestDbDir::new(label);
    let store = TursoMvccStore::open(TursoMvccStoreConfig {
        path: temp.0.join("events.db"),
        connection_lanes: 4,
        passive_checkpoint,
        busy_timeout_ms: 250,
        retry_attempts: 8,
        max_batch_rows: 1_024,
    })
    .await
    .expect("open MVCC event store");
    (temp, store)
}

#[tokio::test]
async fn turso_mvcc_database_keyset_uses_limit_plus_one_and_stable_tie_break() {
    let (_temp, store) = open_store("page", true).await;
    let receipt = store
        .append_batch_typed(
            &[
                event("event-b", 10),
                event("event-a", 10),
                event("event-c", 10),
            ],
            &StorageRetryPolicy::default(),
        )
        .await
        .expect("commit typed MVCC batch");
    assert_eq!(receipt.committed_rows, 3);
    assert_eq!(receipt.retry_count, 0);
    assert_eq!(receipt.busy_count, 0);
    assert_eq!(receipt.snapshot_conflict_count, 0);
    assert_eq!(receipt.retry_delay_ms, 0);

    let first = store
        .read_partition_page("keyset-partition", None, 2)
        .await
        .expect("read first database keyset page");
    assert_eq!(first.len(), 3, "backend must fetch limit + 1");
    assert_eq!(first[0].event_id, "event-a");
    assert_eq!(first[1].event_id, "event-b");
    assert_eq!(first[2].event_id, "event-c");

    let second = store
        .read_partition_page("keyset-partition", Some((10, "event-b")), 2)
        .await
        .expect("read second database keyset page");
    assert_eq!(second.len(), 1);
    assert_eq!(second[0].event_id, "event-c");
}

#[tokio::test]
async fn turso_mvcc_typed_duplicate_aborts_without_retrying() {
    let (_temp, store) = open_store("typed-rollback", false).await;
    store
        .append_batch_typed(&[event("duplicate", 10)], &StorageRetryPolicy::default())
        .await
        .expect("seed identity");

    let error = store
        .append_batch_typed(
            &[event("new-before-error", 20), event("duplicate", 30)],
            &StorageRetryPolicy::default(),
        )
        .await
        .expect_err("constraint failure must abort without retry");
    assert_eq!(error.code, TursoMvccWriteErrorCode::DuplicateIdentity);
    assert!(!error.retryable);

    let committed = store
        .read_partition("keyset-partition")
        .await
        .expect("read committed events");
    assert_eq!(committed.len(), 1);
    assert_eq!(committed[0].event_id, "duplicate");
}

#[tokio::test]
async fn turso_mvcc_passive_checkpoint_flushes_and_recovers_one_mebibyte_payload() {
    let (temp, store) = open_store("recovery", true).await;
    let database_path = temp.0.join("events.db");
    let large_event = TursoMvccEvent {
        partition_key: "recovery-partition".to_owned(),
        event_id: "one-mebibyte".to_owned(),
        payload: vec![0x5a; 1024 * 1024],
        created_at_ms: 1,
    };
    store
        .append_batch_typed(&[large_event], &StorageRetryPolicy::default())
        .await
        .expect("commit large recovery payload");
    let maintenance = store
        .flush_and_measure()
        .await
        .expect("flush dirty pages and measure files");
    assert!(maintenance.passive_checkpoint);
    assert_eq!(maintenance.cache_flush_count, 4);
    assert!(maintenance.total_file_bytes > 0);
    assert!(!maintenance.checkpoint_counter_observable);
    drop(store);

    let reopened = TursoMvccStore::open(TursoMvccStoreConfig {
        path: database_path,
        connection_lanes: 4,
        passive_checkpoint: true,
        busy_timeout_ms: 250,
        retry_attempts: 8,
        max_batch_rows: 1_024,
    })
    .await
    .expect("reopen MVCC database after flush");
    let recovered = reopened
        .read_partition("recovery-partition")
        .await
        .expect("read recovered large payload");
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].payload.len(), 1024 * 1024);
    assert!(recovered[0].payload.iter().all(|byte| *byte == 0x5a));
}
