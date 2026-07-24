mod artifact_pointer_crash_tests {
    use std::path::PathBuf;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    use agent_semantic_client_db::artifact_pointer_store::{
        ClientDbArtifactPointerCasOutcome, ClientDbArtifactPointerCasRequest,
        ClientDbArtifactPointerKey, TursoArtifactPointerStore,
    };
    use agent_semantic_content_identity::hash_blob;

    const CHILD_PATH_ENV: &str = "ASP_ARTIFACT_POINTER_CRASH_CHILD_PATH";
    const CHILD_TEST: &str =
        "db_engine::artifact_pointer_crash_tests::artifact_pointer_crash_writer_child";

    fn temp_db() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "asp-client-db-artifact-crash-{}-{nonce}.turso",
            std::process::id()
        ))
    }

    fn key() -> ClientDbArtifactPointerKey {
        ClientDbArtifactPointerKey::new(
            "repo:crash-test",
            "workspace:crash-test",
            "scope:crash-test",
            "topology-root",
            "current",
        )
    }

    fn durable_root() -> String {
        hash_blob(b"durable-before-abort").to_string()
    }

    #[tokio::test]
    #[ignore = "spawned by the crash-recovery parent test"]
    async fn artifact_pointer_crash_writer_child() {
        let Some(path) = std::env::var_os(CHILD_PATH_ENV) else {
            return;
        };
        let store = TursoArtifactPointerStore::open(PathBuf::from(path))
            .await
            .expect("crash child opens artifact pointer store");
        let receipt = store
            .compare_and_set(&ClientDbArtifactPointerCasRequest {
                key: key(),
                expected_root_hash: None,
                expected_revision: 0,
                new_root_hash: durable_root(),
                updated_at_ms: 1,
            })
            .await
            .expect("crash child commits pointer before abort");
        assert_eq!(receipt.outcome, ClientDbArtifactPointerCasOutcome::Applied);
        std::process::abort();
    }

    #[test]
    fn committed_artifact_pointer_recovers_after_process_abort() {
        let path = temp_db();
        let status = Command::new(std::env::current_exe().expect("current unit-test executable"))
            .arg("--ignored")
            .arg("--exact")
            .arg(CHILD_TEST)
            .arg("--nocapture")
            .env(CHILD_PATH_ENV, &path)
            .status()
            .expect("spawn crash writer child");
        assert!(!status.success(), "crash child must terminate abnormally");

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("crash recovery runtime");
        runtime.block_on(async {
            let recovered = TursoArtifactPointerStore::open(&path)
                .await
                .expect("reopen artifact pointer store after abort");
            let receipt = recovered
                .compare_and_set(&ClientDbArtifactPointerCasRequest {
                    key: key(),
                    expected_root_hash: None,
                    expected_revision: 0,
                    new_root_hash: hash_blob(b"must-not-overwrite-recovered-pointer").to_string(),
                    updated_at_ms: 2,
                })
                .await
                .expect("read recovered pointer through typed conflict");
            assert_eq!(receipt.outcome, ClientDbArtifactPointerCasOutcome::Conflict);
            assert_eq!(receipt.observed_revision, 1);
            assert_eq!(receipt.observed_root_hash.as_deref(), Some(durable_root().as_str()));
        });
    }
}
