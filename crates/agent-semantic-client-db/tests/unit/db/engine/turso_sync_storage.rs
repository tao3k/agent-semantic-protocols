mod turso_sync_storage_tests {
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

use agent_semantic_client_db::turso_sync_storage::{
    TursoSyncProfileConfig, TursoSyncProfileMode, TursoSyncStorage, TursoSyncStorageErrorCode,
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
        mode: TursoSyncProfileMode::Remote {
            remote_url: "http://127.0.0.1:1".to_owned(),
            auth_token: "fixed-test-token".to_owned(),
            bootstrap_if_empty: false,
        },
            operation_timeout: Duration::from_millis(100),
        }
    }

    #[tokio::test]
    async fn sync_profile_rejects_missing_remote_identity_before_open() {
        let result = TursoSyncStorage::open(TursoSyncProfileConfig {
            path: temp_db("invalid"),
        mode: TursoSyncProfileMode::Remote {
            remote_url: String::new(),
            auth_token: String::new(),
            bootstrap_if_empty: false,
        },
            operation_timeout: Duration::from_millis(100),
        })
        .await;
        let error = match result {
            Ok(_) => panic!("missing remote identity must fail closed"),
            Err(error) => error,
        };
        assert_eq!(error.code, TursoSyncStorageErrorCode::InvalidConfiguration);
    }

    #[tokio::test]
    async fn sync_profile_open_times_out_for_unresponsive_remote() {
        let error = match TursoSyncStorage::open(offline_config(temp_db("open-timeout"))).await {
            Ok(_) => panic!("unresponsive remote must fail at the configured open timeout"),
            Err(error) => error,
        };

        assert_eq!(format!("{:?}", error.code), "Timeout");
        assert!(error.message.contains("open timed out"));
    }
}
