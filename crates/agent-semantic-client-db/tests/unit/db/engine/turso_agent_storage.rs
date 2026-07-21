use agent_semantic_client_db::turso_agent_storage::TursoMvccAgentStorage;

fn turso_agent_storage_partition() -> StoragePartitionKey {
    StoragePartitionKey {
        repo_id: "repo".to_string(),
        workspace_id: "workspace".to_string(),
        scope_id: "scope".to_string(),
        session_id: "session".to_string(),
        agent_id: "agent".to_string(),
    }
}

fn turso_agent_storage_batch(profile: StorageOptimizationProfile) -> SessionEventBatch {
    SessionEventBatch {
        schema_id: SESSION_EVENT_BATCH_SCHEMA_ID.to_string(),
        batch_id: "batch".to_string(),
        partition: turso_agent_storage_partition(),
        optimization_profile: profile,
        transaction_mode: StorageTransactionMode::Concurrent,
        retry_policy: StorageRetryPolicy::default(),
        events: (0..4)
            .map(|index| SessionEvent {
                event_id: format!("event-{index}"),
                turn_id: "turn".to_string(),
                event_kind: "tool".to_string(),
                payload: vec![index],
                created_at_ms: index as i64,
            })
            .collect(),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn turso_agent_storage_implements_atomic_batch_and_keyset_contract() {
    let temp = temp_root("turso-agent-storage-contract");
    let storage = TursoMvccAgentStorage::open(TursoMvccStoreConfig::new(
        temp.join("session-events.turso"),
    ))
    .await
    .expect("open Turso AgentStorage adapter");
    let batch = turso_agent_storage_batch(StorageOptimizationProfile::MvccConcurrent);
    let receipt = storage
        .append_session_events_atomically(&batch)
        .await
        .expect("append session event batch");
    assert_eq!(receipt.transaction_state, StorageTransactionState::Committed);
    assert_eq!(receipt.backend_version, "0.7.0");
    assert_eq!(receipt.committed_rows, 4);

    let first = storage
        .list_session_events(&SessionEventPageRequest {
            partition: turso_agent_storage_partition(),
            after: None,
            limit: 2,
        })
        .await
        .expect("read first Turso keyset page");
    assert_eq!(first.items.len(), 2);
    let second = storage
        .list_session_events(&SessionEventPageRequest {
            partition: turso_agent_storage_partition(),
            after: first.next,
            limit: 2,
        })
        .await
        .expect("read second Turso keyset page");
    assert_eq!(second.items[0].event_id, "event-2");
    drop(storage);
    let _ = std::fs::remove_dir_all(temp);
}

#[tokio::test(flavor = "current_thread")]
async fn turso_agent_storage_requires_the_opened_passive_checkpoint_profile() {
    let temp = temp_root("turso-agent-storage-passive-profile");
    let mut config = TursoMvccStoreConfig::new(temp.join("session-events.turso"));
    config.passive_checkpoint = true;
    let storage = TursoMvccAgentStorage::open(config)
        .await
        .expect("open passive-checkpoint adapter");
    let batch = turso_agent_storage_batch(
        StorageOptimizationProfile::MvccConcurrentPassiveCheckpoint,
    );
    let receipt = storage
        .append_session_events_atomically(&batch)
        .await
        .expect("append passive-checkpoint batch");
    assert_eq!(
        receipt.optimization_profile,
        StorageOptimizationProfile::MvccConcurrentPassiveCheckpoint
    );
    assert!(storage.store().optimization_receipt().passive_checkpoint);
    drop(storage);
    let _ = std::fs::remove_dir_all(temp);
}
