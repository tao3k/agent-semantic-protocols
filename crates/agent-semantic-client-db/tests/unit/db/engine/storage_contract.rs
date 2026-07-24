use agent_semantic_client_db::storage_contract::{
    AgentStorage, InMemoryAgentStorage, SESSION_EVENT_BATCH_SCHEMA_ID, SessionEvent,
    SessionEventBatch, SessionEventPageRequest, StorageErrorCode, StorageOptimizationProfile,
    StoragePartitionKey, StorageRetryPolicy, StorageTransactionMode, StorageTransactionState,
};

fn storage_contract_partition() -> StoragePartitionKey {
    StoragePartitionKey {
        repo_id: "repo".into(),
        workspace_id: "workspace".into(),
        scope_id: "scope".into(),
        session_id: "session".into(),
        agent_id: "agent".into(),
    }
}

fn storage_contract_batch(batch_id: &str, start: usize, count: usize) -> SessionEventBatch {
    SessionEventBatch {
        schema_id: SESSION_EVENT_BATCH_SCHEMA_ID.to_string(),
        batch_id: batch_id.to_string(),
        partition: storage_contract_partition(),
        optimization_profile: StorageOptimizationProfile::CompatibilityImmediate,
        transaction_mode: StorageTransactionMode::Immediate,
        retry_policy: StorageRetryPolicy::default(),
        events: (start..start + count)
            .map(|index| SessionEvent {
                event_id: format!("event-{index:04}"),
                turn_id: "turn".to_string(),
                event_kind: "test".to_string(),
                payload: vec![index as u8],
                created_at_ms: index as i64,
            })
            .collect(),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn storage_contract_in_memory_batch_is_atomic_and_receipted() {
    let storage = InMemoryAgentStorage::default();
    let request = storage_contract_batch("batch-1", 0, 4);
    let receipt = storage
        .append_session_events_atomically(&request)
        .await
        .expect("append in-memory session batch");
    assert_eq!(receipt.transaction_state, StorageTransactionState::Committed);
    assert_eq!(receipt.committed_rows, 4);
    assert!(receipt.execution_digest.starts_with("sha256:"));

    let duplicate = storage_contract_batch("batch-2", 3, 2);
    let error = storage
        .append_session_events_atomically(&duplicate)
        .await
        .expect_err("reject duplicate identity");
    assert_eq!(error.code, StorageErrorCode::DuplicateIdentity);

    let page = storage
        .list_session_events(&SessionEventPageRequest {
            partition: storage_contract_partition(),
            after: None,
            limit: 10,
        })
        .await
        .expect("list in-memory session events");
    assert_eq!(page.items.len(), 4);
    assert_eq!(page.items.last().expect("last event").event_id, "event-0003");
}

#[tokio::test(flavor = "current_thread")]
async fn storage_contract_in_memory_keyset_page_uses_stable_tie_break_cursor() {
    let storage = InMemoryAgentStorage::default();
    storage
        .append_session_events_atomically(&storage_contract_batch("batch", 0, 5))
        .await
        .expect("append page fixture");
    let first = storage
        .list_session_events(&SessionEventPageRequest {
            partition: storage_contract_partition(),
            after: None,
            limit: 2,
        })
        .await
        .expect("first page");
    assert_eq!(first.items.len(), 2);
    let second = storage
        .list_session_events(&SessionEventPageRequest {
            partition: storage_contract_partition(),
            after: first.next,
            limit: 2,
        })
        .await
        .expect("second page");
    assert_eq!(second.items[0].event_id, "event-0002");
    assert!(second.next.is_some());
}
