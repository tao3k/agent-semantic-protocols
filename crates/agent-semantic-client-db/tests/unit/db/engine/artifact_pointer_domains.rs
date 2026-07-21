mod artifact_pointer_domain_tests {
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use agent_semantic_client_db::artifact_pointer_store::{
        ClientDbArtifactPointerCasOutcome, ClientDbArtifactPointerCasRequest,
        ClientDbArtifactPointerKey, TursoArtifactPointerStore,
    };
    use agent_semantic_content_identity::hash_blob;

    fn temp_db() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "asp-client-db-domain-authority-{}-{nonce}.turso",
            std::process::id()
        ))
    }

    fn key(pointer_kind: &str) -> ClientDbArtifactPointerKey {
        ClientDbArtifactPointerKey {
            repo_id: "repo:domain-test".to_owned(),
            workspace_id: "workspace:domain-test".to_owned(),
            scope_id: "scope:domain-test".to_owned(),
            pointer_kind: pointer_kind.to_owned(),
            pointer_name: "current".to_owned(),
        }
    }

    fn root(label: &str) -> String {
        hash_blob(label.as_bytes()).to_string()
    }

    #[tokio::test]
    async fn memory_topology_and_coordination_authorities_are_key_isolated() {
        let store = TursoArtifactPointerStore::open(temp_db())
            .await
            .expect("open domain authority store");
        for (index, pointer_kind) in [
            "memory-root",
            "topology-root",
            "coordination-root",
        ]
        .into_iter()
        .enumerate()
        {
            let receipt = store
                .compare_and_set(&ClientDbArtifactPointerCasRequest {
                    key: key(pointer_kind),
                    expected_root_hash: None,
                    expected_revision: 0,
                    new_root_hash: root(pointer_kind),
                    updated_at_ms: index as i64,
                })
                .await
                .expect("initialize isolated domain authority");
            assert_eq!(receipt.outcome, ClientDbArtifactPointerCasOutcome::Applied);
            assert_eq!(receipt.current.as_ref().map(|row| row.revision), Some(1));
        }

        let memory = store
            .compare_and_set(&ClientDbArtifactPointerCasRequest {
                key: key("memory-root"),
                expected_root_hash: Some(root("memory-root")),
                expected_revision: 1,
                new_root_hash: root("memory-root-next"),
                updated_at_ms: 4,
            })
            .await
            .expect("advance memory authority only");
        assert_eq!(memory.outcome, ClientDbArtifactPointerCasOutcome::Applied);
        assert_eq!(memory.current.as_ref().map(|row| row.revision), Some(2));

        for pointer_kind in ["topology-root", "coordination-root"] {
            let independent = store
                .compare_and_set(&ClientDbArtifactPointerCasRequest {
                    key: key(pointer_kind),
                    expected_root_hash: Some(root(pointer_kind)),
                    expected_revision: 1,
                    new_root_hash: root(&format!("{pointer_kind}-next")),
                    updated_at_ms: 5,
                })
                .await
                .expect("other domain retained its own revision");
            assert_eq!(independent.outcome, ClientDbArtifactPointerCasOutcome::Applied);
            assert_eq!(
                independent.current.as_ref().map(|row| row.revision),
                Some(2)
            );
        }
    }
}
