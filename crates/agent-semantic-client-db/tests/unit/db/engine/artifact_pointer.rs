mod artifact_pointer_tests {
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    use agent_semantic_client_db::artifact_pointer_store::{
        ClientDbArtifactPointerCasOutcome, ClientDbArtifactPointerCasRequest,
        ClientDbArtifactPointerKey, ClientDbFailedArtifact, TursoArtifactPointerStore,
    };
    use agent_semantic_content_identity::hash_blob;

    fn temp_db(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "asp-client-db-artifact-pointer-{name}-{}-{nonce}.turso",
            std::process::id()
        ))
    }

    fn key(pointer_name: impl Into<String>) -> ClientDbArtifactPointerKey {
        ClientDbArtifactPointerKey::new(
            "repo:test",
            "workspace:test",
            "scope:test",
            "semantic-index-root",
            pointer_name,
        )
    }

    fn root(label: &str) -> String {
        hash_blob(label.as_bytes()).to_string()
    }

    #[tokio::test]
    async fn artifact_pointer_cas_is_durable_and_rejects_stale_revision() {
        let path = temp_db("durable");
        let pointer_key = key("main");
        let first_root = root("first-root");
        let second_root = root("second-root");

        let store = TursoArtifactPointerStore::open(&path)
            .await
            .expect("open artifact pointer store");
        let first = store
            .compare_and_set(&ClientDbArtifactPointerCasRequest {
                key: pointer_key.clone(),
                expected_root_hash: None,
                expected_revision: 0,
                new_root_hash: first_root.clone(),
                updated_at_ms: 1,
            })
            .await
            .expect("initial CAS");
        assert_eq!(first.outcome, ClientDbArtifactPointerCasOutcome::Applied);
        assert_eq!(first.current.as_ref().map(|row| row.revision), Some(1));

        let second = store
            .compare_and_set(&ClientDbArtifactPointerCasRequest {
                key: pointer_key.clone(),
                expected_root_hash: Some(first_root.clone()),
                expected_revision: 1,
                new_root_hash: second_root.clone(),
                updated_at_ms: 2,
            })
            .await
            .expect("second CAS");
        assert_eq!(second.outcome, ClientDbArtifactPointerCasOutcome::Applied);
        assert_eq!(second.current.as_ref().map(|row| row.revision), Some(2));

        let stale = store
            .compare_and_set(&ClientDbArtifactPointerCasRequest {
                key: pointer_key.clone(),
                expected_root_hash: Some(first_root),
                expected_revision: 1,
                new_root_hash: root("must-not-win"),
                updated_at_ms: 3,
            })
            .await
            .expect("stale CAS returns typed conflict receipt");
        assert_eq!(stale.outcome, ClientDbArtifactPointerCasOutcome::Conflict);
        assert_eq!(stale.observed_revision, 2);
        assert_eq!(stale.observed_root_hash.as_deref(), Some(second_root.as_str()));
        drop(store);

        let reopened = TursoArtifactPointerStore::open(&path)
            .await
            .expect("reopen artifact pointer store");
        let recovered = reopened
            .compare_and_set(&ClientDbArtifactPointerCasRequest {
                key: pointer_key,
                expected_root_hash: Some(second_root.clone()),
                expected_revision: 2,
                new_root_hash: root("third-root"),
                updated_at_ms: 4,
            })
            .await
            .expect("CAS after reopen");
        assert_eq!(recovered.outcome, ClientDbArtifactPointerCasOutcome::Applied);
        assert_eq!(recovered.current.as_ref().map(|row| row.revision), Some(3));
    }

    #[tokio::test]
    async fn artifact_pointer_sixteen_way_cas_storm_has_exactly_one_winner() {
        let path = temp_db("storm");
        let pointer_key = key("storm");
        let mut stores = Vec::with_capacity(16);
        for _ in 0..16 {
            stores.push(Arc::new(
                TursoArtifactPointerStore::open(&path)
                    .await
                    .expect("open CAS contender"),
            ));
        }

        let mut tasks = tokio::task::JoinSet::new();
        for (candidate, store) in stores.into_iter().enumerate() {
            let contender_key = pointer_key.clone();
            tasks.spawn(async move {
                store
                    .compare_and_set(&ClientDbArtifactPointerCasRequest {
                        key: contender_key,
                        expected_root_hash: None,
                        expected_revision: 0,
                        new_root_hash: root(&format!("candidate-{candidate}")),
                        updated_at_ms: candidate as i64,
                    })
                    .await
            });
        }

        let mut applied = 0;
        let mut conflicts = 0;
        while let Some(result) = tasks.join_next().await {
            let receipt = result
                .expect("contender task")
                .expect("contender returns a CAS receipt");
            match receipt.outcome {
                ClientDbArtifactPointerCasOutcome::Applied => applied += 1,
                ClientDbArtifactPointerCasOutcome::Conflict => conflicts += 1,
            }
        }
        assert_eq!(applied, 1, "exactly one contender may advance revision zero");
        assert_eq!(conflicts, 15, "every losing contender is a typed conflict");
    }

    #[tokio::test]
    async fn failed_artifacts_are_append_only_and_never_advance_success_pointer() {
        let path = temp_db("failed-artifact");
        let pointer_key = key("failure-preservation");
        let successful_root = root("successful-root");
        let failed_root = root("failed-root");
        let store = TursoArtifactPointerStore::open(&path)
            .await
            .expect("open artifact pointer store");

        store
            .compare_and_set(&ClientDbArtifactPointerCasRequest {
                key: pointer_key.clone(),
                expected_root_hash: None,
                expected_revision: 0,
                new_root_hash: successful_root.clone(),
                updated_at_ms: 1,
            })
            .await
            .expect("initialize success pointer");

        let failed = ClientDbFailedArtifact {
            attempt_id: "attempt:failure-1".to_owned(),
            key: pointer_key.clone(),
            candidate_root_hash: Some(failed_root.clone()),
            error_digest: root("compiler-error"),
            evidence: b"compile error: intentional fixture".to_vec(),
            created_at_ms: 2,
        };
        store
            .preserve_failed_artifact(&failed)
            .await
            .expect("preserve failed artifact");

        let duplicate = store.preserve_failed_artifact(&failed).await;
        assert!(duplicate.is_err(), "attempt IDs are immutable and unique");

        let failures = store
            .list_failed_artifacts(&pointer_key, 10)
            .await
            .expect("list failed artifacts");
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].candidate_root_hash.as_deref(), Some(failed_root.as_str()));

        let still_current = store
            .compare_and_set(&ClientDbArtifactPointerCasRequest {
                key: pointer_key,
                expected_root_hash: Some(successful_root),
                expected_revision: 1,
                new_root_hash: root("next-success"),
                updated_at_ms: 3,
            })
            .await
            .expect("failure preservation did not advance pointer");
        assert_eq!(still_current.outcome, ClientDbArtifactPointerCasOutcome::Applied);
        assert_eq!(still_current.current.as_ref().map(|row| row.revision), Some(2));
    }
}
