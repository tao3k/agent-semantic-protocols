use agent_semantic_client_db::turso_mvcc_store::{
    TursoMvccEvent, TursoMvccStore, TursoMvccStoreConfig,
};

fn turso_mvcc_event(partition: &str, id: usize) -> TursoMvccEvent {
    TursoMvccEvent {
        partition_key: partition.to_string(),
        event_id: format!("{id:08}"),
        payload: format!("payload-{partition}-{id}").into_bytes(),
        created_at_ms: id as i64,
    }
}

fn turso_mvcc_batch(partition: &str, count: usize) -> Vec<TursoMvccEvent> {
    (0..count)
        .map(|id| turso_mvcc_event(partition, id))
        .collect()
}

#[tokio::test(flavor = "current_thread")]
async fn turso_mvcc_store_reports_and_executes_the_four_lane_contract() {
    let temp = temp_root("turso-mvcc-four-lane");
    let db_path = temp.join("append.turso");
    let store = TursoMvccStore::open(TursoMvccStoreConfig::new(db_path.clone()))
        .await
        .expect("open Turso MVCC store");

    let receipt = store.optimization_receipt();
    assert_eq!(
        receipt.schema_id,
        "asp.turso-mvcc-optimization-receipt.v1"
    );
    assert_eq!(receipt.profile, "async-io-mvcc");
    assert_eq!(receipt.connection_lanes, 4);
    assert_eq!(receipt.partition_shards, 4);
    assert_eq!(receipt.insert_rows_per_statement, 32);
    assert_eq!(receipt.statement_cache, "prepared-cached-per-connection");
    assert_eq!(receipt.transaction_mode, "begin-concurrent");
    assert!(receipt.mvcc);
    assert!(!receipt.passive_checkpoint);
    assert!(!receipt.multiprocess_wal);
    assert!(!receipt.fts);
    let optimization_schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../../schemas/turso-mvcc-optimization-receipt.v1.schema.json"
    ))
    .expect("parse Turso MVCC optimization receipt schema");
    assert_eq!(
        optimization_schema["properties"]["schemaId"]["const"],
        receipt.schema_id
    );
    let batch_schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../../schemas/turso-mvcc-batch-write-receipt.v1.schema.json"
    ))
    .expect("parse Turso MVCC batch receipt schema");
    assert_eq!(
        batch_schema["properties"]["schemaId"]["const"],
        "asp.turso-mvcc-batch-write-receipt.v1"
    );

    let a = turso_mvcc_batch("agent-a", 64);
    let b = turso_mvcc_batch("agent-b", 64);
    let c = turso_mvcc_batch("agent-c", 64);
    let d = turso_mvcc_batch("agent-d", 64);
    let (a_receipt, b_receipt, c_receipt, d_receipt) = tokio::join!(
        store.append_batch(&a),
        store.append_batch(&b),
        store.append_batch(&c),
        store.append_batch(&d),
    );
    for write in [a_receipt, b_receipt, c_receipt, d_receipt] {
        let write = write.expect("commit concurrent Turso MVCC batch");
        assert_eq!(
            write.schema_id,
            "asp.turso-mvcc-batch-write-receipt.v1"
        );
        assert_eq!(write.attempted_rows, 64);
        assert_eq!(write.committed_rows, 64);
        assert_eq!(write.optimization, receipt);
    }

    drop(store);
    let reopened = TursoMvccStore::open(TursoMvccStoreConfig::new(db_path))
        .await
        .expect("reopen Turso MVCC store");
    for partition in ["agent-a", "agent-b", "agent-c", "agent-d"] {
        let persisted = reopened
            .read_partition(partition)
            .await
            .expect("read Turso MVCC partition");
        assert_eq!(persisted, turso_mvcc_batch(partition, 64));
    }
    drop(reopened);
    let _ = std::fs::remove_dir_all(temp);
}

#[tokio::test(flavor = "current_thread")]
async fn turso_mvcc_store_exposes_the_passive_checkpoint_profile() {
    let temp = temp_root("turso-mvcc-passive-checkpoint");
    let mut config = TursoMvccStoreConfig::new(temp.join("append.turso"));
    config.passive_checkpoint = true;
    let store = TursoMvccStore::open(config)
        .await
        .expect("open passive-checkpoint Turso MVCC store");
    let receipt = store.optimization_receipt();
    assert_eq!(receipt.profile, "async-io-mvcc-passive-checkpoint");
    assert!(receipt.mvcc);
    assert!(receipt.passive_checkpoint);
    assert!(!receipt.multiprocess_wal);
    assert!(!receipt.fts);
    drop(store);
    let _ = std::fs::remove_dir_all(temp);
}

#[tokio::test(flavor = "current_thread")]
async fn turso_mvcc_store_rolls_back_the_whole_batch_on_duplicate_identity() {
    let temp = temp_root("turso-mvcc-atomic-batch");
    let store = TursoMvccStore::open(TursoMvccStoreConfig::new(temp.join("atomic.turso")))
        .await
        .expect("open Turso MVCC store");
    let mixed_partition_error = store
        .append_batch(&[
            turso_mvcc_event("agent-a", 0),
            turso_mvcc_event("agent-b", 0),
        ])
        .await
        .expect_err("mixed-partition batch must be rejected before writing");
    assert!(mixed_partition_error.contains("exactly one partition"));
    store
        .append_batch(&[turso_mvcc_event("agent-a", 1)])
        .await
        .expect("seed Turso MVCC event");

    let error = store
        .append_batch(&[
            turso_mvcc_event("agent-a", 2),
            turso_mvcc_event("agent-a", 1),
        ])
        .await
        .expect_err("duplicate identity must reject the whole batch");
    assert!(error.contains("failed to append Turso MVCC event"));

    let persisted = store
        .read_partition("agent-a")
        .await
        .expect("read Turso MVCC partition after rollback");
    assert_eq!(persisted, vec![turso_mvcc_event("agent-a", 1)]);
    let _ = std::fs::remove_dir_all(temp);
}
