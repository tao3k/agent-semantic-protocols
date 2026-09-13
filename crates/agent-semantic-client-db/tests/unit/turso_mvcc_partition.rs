// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use agent_semantic_client_db::turso_mvcc_partition::TursoMvccExpectedHead;
use agent_semantic_client_db::turso_mvcc_partition::TursoMvccPartitionCommit;
use agent_semantic_client_db::turso_mvcc_partition::TursoMvccPartitionCommitOutcome;
use agent_semantic_client_db::turso_mvcc_partition::TursoMvccPartitionRecord;
use agent_semantic_client_db::turso_mvcc_store::TursoMvccStore;
use agent_semantic_client_db::turso_mvcc_store::TursoMvccStoreConfig;

fn temp_database(name: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    std::env::temp_dir().join(format!("asp-mvcc-partition-{name}-{nanos}.turso"))
}

fn initialize(partition_key: &str) -> TursoMvccPartitionCommit {
    TursoMvccPartitionCommit {
        partition_key: partition_key.to_string(),
        expected: None,
        next_revision: 0,
        next_head_digest: "head-0".to_string(),
        next_projection: b"projection-0".to_vec(),
        records: Vec::new(),
        committed_at_ms: 1,
    }
}

fn advance(
    partition_key: &str,
    expected: TursoMvccExpectedHead,
    record_id: &str,
    next_digest: &str,
) -> TursoMvccPartitionCommit {
    TursoMvccPartitionCommit {
        partition_key: partition_key.to_string(),
        next_revision: expected.revision + 1,
        expected: Some(expected),
        next_head_digest: next_digest.to_string(),
        next_projection: next_digest.as_bytes().to_vec(),
        records: vec![
            TursoMvccPartitionRecord::new(
                record_id,
                "test-event.v1",
                record_id.as_bytes().to_vec(),
            )
            .expect("valid test event record"),
        ],
        committed_at_ms: 2,
    }
}

#[tokio::test]
async fn partition_compare_and_append_is_atomic_and_rejects_stale_head() {
    let store = TursoMvccStore::open(TursoMvccStoreConfig::new(temp_database("atomic")))
        .await
        .expect("open MVCC store");
    let initialized = store
        .compare_and_append_partition(&initialize("run-1"))
        .await
        .expect("initialize partition");
    let TursoMvccPartitionCommitOutcome::Committed(initialized) = initialized else {
        panic!("initialization must commit");
    };

    let expected = TursoMvccExpectedHead::from(&initialized.head);
    let commit = advance("run-1", expected, "event-1", "head-1");
    let applied = store
        .compare_and_append_partition(&commit)
        .await
        .expect("advance partition");
    assert!(matches!(
        applied,
        TursoMvccPartitionCommitOutcome::Committed(_)
    ));

    let stale = store
        .compare_and_append_partition(&commit)
        .await
        .expect("stale commit returns a typed conflict");
    let TursoMvccPartitionCommitOutcome::Conflict(Some(observed)) = stale else {
        panic!("stale commit must conflict");
    };
    assert_eq!(observed.revision, 1);
    assert_eq!(observed.last_sequence, 1);
    assert_eq!(
        store.read_partition_records("run-1").await.unwrap().len(),
        1
    );
}

#[tokio::test]
async fn same_head_concurrency_commits_exactly_once() {
    let store = TursoMvccStore::open(TursoMvccStoreConfig::new(temp_database("concurrent")))
        .await
        .expect("open MVCC store");
    let initialized = store
        .compare_and_append_partition(&initialize("run-1"))
        .await
        .expect("initialize partition");
    let TursoMvccPartitionCommitOutcome::Committed(initialized) = initialized else {
        panic!("initialization must commit");
    };
    let expected = TursoMvccExpectedHead::from(&initialized.head);
    let first = advance("run-1", expected.clone(), "event-a", "head-a");
    let second = advance("run-1", expected, "event-b", "head-b");

    let (first, second) = tokio::join!(
        store.compare_and_append_partition(&first),
        store.compare_and_append_partition(&second)
    );
    let outcomes = [first.unwrap(), second.unwrap()];
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| matches!(outcome, TursoMvccPartitionCommitOutcome::Committed(_)))
            .count(),
        1
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| matches!(outcome, TursoMvccPartitionCommitOutcome::Conflict(_)))
            .count(),
        1
    );
    assert_eq!(
        store.read_partition_records("run-1").await.unwrap().len(),
        1
    );
}
