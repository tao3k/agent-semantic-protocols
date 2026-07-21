mod turso_sync_storage_tests {
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use agent_semantic_client_db::turso_sync_storage::{
        TURSO_SYNC_OPERATION_RECEIPT_SCHEMA_ID, TursoSyncOperation, TursoSyncOperationOutcome,
        TursoSyncProfileConfig, TursoSyncStorage, TursoSyncStorageErrorCode,
    };

    fn temp_db(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "asp-client-db-sync-{name}-{}-{nonce}.turso",
            std::process::id()
        ))
    }

    fn offline_config(path: PathBuf) -> TursoSyncProfileConfig {
        TursoSyncProfileConfig {
            path,
            remote_url: "http://127.0.0.1:1".to_owned(),
            auth_token: "fixed-test-token".to_owned(),
            bootstrap_if_empty: false,
        }
    }

    #[tokio::test]
    async fn sync_profile_rejects_missing_remote_identity_before_open() {
        let result = TursoSyncStorage::open(TursoSyncProfileConfig {
            path: temp_db("invalid"),
            remote_url: String::new(),
            auth_token: String::new(),
            bootstrap_if_empty: false,
        })
        .await;
        let error = match result {
            Ok(_) => panic!("missing remote identity must fail closed"),
            Err(error) => error,
        };
        assert_eq!(error.code, TursoSyncStorageErrorCode::InvalidConfiguration);
    }

    #[tokio::test]
    async fn sync_profile_keeps_local_writes_across_checkpoint_network_failure_and_reopen() {
        let path = temp_db("offline-first");
        let storage = TursoSyncStorage::open(offline_config(path.clone()))
            .await
            .expect("open offline-first sync profile");
        let connection = storage.connect().await.expect("connect local sync database");
        connection
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS sync_fixture(\
                    id INTEGER PRIMARY KEY, payload TEXT NOT NULL\
                 );\
                 INSERT INTO sync_fixture(id, payload) VALUES (1, 'durable-local-write');",
            )
            .await
            .expect("local write does not require remote connectivity");

        let before = storage.stats().await;
        assert_eq!(before.schema_id, TURSO_SYNC_OPERATION_RECEIPT_SCHEMA_ID);
        assert_eq!(before.operation, TursoSyncOperation::Stats);
        assert_eq!(before.outcome, TursoSyncOperationOutcome::Observed);
        assert!(before.stats.is_some());

        let checkpoint = storage.checkpoint().await;
        assert_eq!(checkpoint.operation, TursoSyncOperation::Checkpoint);
        assert_eq!(checkpoint.outcome, TursoSyncOperationOutcome::Applied);
        assert!(checkpoint.stats.is_some());

        let push = storage.push().await;
        assert_eq!(push.operation, TursoSyncOperation::Push);
        assert_eq!(push.outcome, TursoSyncOperationOutcome::Failed);
        assert!(push.error_digest.is_some());

        let pull = storage.pull().await;
        assert_eq!(pull.operation, TursoSyncOperation::Pull);
        assert_eq!(pull.outcome, TursoSyncOperationOutcome::Failed);
        assert!(pull.error_digest.is_some());

        let count: i64 = connection
            .query("SELECT COUNT(*) FROM sync_fixture", ())
            .await
            .expect("query local fixture")
            .next()
            .await
            .expect("read local fixture row")
            .expect("count row")
            .get(0)
            .expect("decode count");
        assert_eq!(count, 1, "network failure must not roll back local state");
        drop(connection);
        drop(storage);

        let reopened = TursoSyncStorage::open(offline_config(path))
            .await
            .expect("reopen offline-first sync profile");
        let reopened_connection = reopened.connect().await.expect("reconnect local database");
        let payload: String = reopened_connection
            .query("SELECT payload FROM sync_fixture WHERE id = 1", ())
            .await
            .expect("query recovered local fixture")
            .next()
            .await
            .expect("read recovered fixture row")
            .expect("payload row")
            .get(0)
            .expect("decode payload");
        assert_eq!(payload, "durable-local-write");
    }
}
