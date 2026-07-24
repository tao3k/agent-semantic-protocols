mod turso_cdc_storage_tests {
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use agent_semantic_client_db::turso_cdc_storage::{
        TursoCdcCaptureMode, TursoCdcChangeKind, TursoCdcProfileConfig, TursoCdcStorage,
    };
    use turso::transaction::TransactionBehavior;

    fn temp_db(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "asp-client-db-cdc-{name}-{}-{nonce}.turso",
            std::process::id()
        ))
    }

    fn config(name: &str) -> TursoCdcProfileConfig {
        TursoCdcProfileConfig {
            path: temp_db(name),
            mode: TursoCdcCaptureMode::Full,
            table_name: "asp_test_cdc".to_owned(),
        }
    }

    #[tokio::test]
    async fn cdc_profile_rejects_untrusted_table_name() {
        let error = match TursoCdcStorage::open(TursoCdcProfileConfig {
            path: temp_db("invalid-name"),
            mode: TursoCdcCaptureMode::Full,
            table_name: "cdc; DROP TABLE evidence".to_owned(),
        })
        .await
        {
            Ok(_) => panic!("untrusted CDC table name must fail closed"),
            Err(error) => error,
        };
        assert!(error.contains("ASCII letters"));
    }

    #[tokio::test]
    async fn cdc_profile_captures_commits_omits_rollbacks_and_pages_by_change_id() {
        let storage = TursoCdcStorage::open(config("transactions"))
            .await
            .expect("open non-MVCC CDC profile");
        let mut connection = storage.connection();
        connection
            .execute(
                "CREATE TABLE cdc_fixture(id INTEGER PRIMARY KEY, value TEXT NOT NULL)",
                (),
            )
            .await
            .expect("create CDC fixture table");
        let before = storage
    .read_page(None, 1_000.into())
            .await
            .expect("read CDC setup cursor")
            .next_change_id;

        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await
            .expect("begin captured transaction");
        transaction
            .execute(
                "INSERT INTO cdc_fixture(id, value) VALUES (?1, ?2)",
                (1_i64, "one"),
            )
            .await
            .expect("insert first captured row");
        transaction
            .execute(
                "INSERT INTO cdc_fixture(id, value) VALUES (?1, ?2)",
                (2_i64, "two"),
            )
            .await
            .expect("insert second captured row");
        transaction
            .execute(
                "UPDATE cdc_fixture SET value = ?1 WHERE id = ?2",
                ("one-updated", 1_i64),
            )
            .await
            .expect("update captured row");
        transaction
            .execute("DELETE FROM cdc_fixture WHERE id = ?1", [2_i64])
            .await
            .expect("delete captured row");
        transaction.commit().await.expect("commit captured transaction");

        let committed = storage
    .read_page(before.map(Into::into), 1_000.into())
            .await
            .expect("read committed CDC changes");
        let fixture_changes: Vec<_> = committed
            .changes
            .iter()
            .filter(|change| change.table_name.as_deref() == Some("cdc_fixture"))
            .collect();
        assert_eq!(
            fixture_changes
                .iter()
                .filter(|change| change.kind == TursoCdcChangeKind::Insert)
                .count(),
            2
        );
        assert_eq!(
            fixture_changes
                .iter()
                .filter(|change| change.kind == TursoCdcChangeKind::Update)
                .count(),
            1
        );
        assert_eq!(
            fixture_changes
                .iter()
                .filter(|change| change.kind == TursoCdcChangeKind::Delete)
                .count(),
            1
        );
        let committed_cursor = committed.next_change_id;

        let rollback = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await
            .expect("begin rolled-back transaction");
        rollback
            .execute(
                "INSERT INTO cdc_fixture(id, value) VALUES (?1, ?2)",
                (3_i64, "must-not-appear"),
            )
            .await
            .expect("write rolled-back row");
        rollback.rollback().await.expect("roll back CDC fixture");
        let after_rollback = storage
    .read_page(committed_cursor.map(Into::into), 1_000.into())
            .await
            .expect("read CDC after rollback");
        assert!(
            after_rollback
                .changes
                .iter()
                .all(|change| change.table_name.as_deref() != Some("cdc_fixture")),
            "rolled-back rows must not become CDC truth"
        );

        let mut journal_rows = connection
            .query("PRAGMA journal_mode", ())
            .await
            .expect("query CDC journal mode");
        let journal_mode: String = journal_rows
            .next()
            .await
            .expect("advance journal mode")
            .expect("journal mode row")
            .get(0)
            .expect("decode journal mode");
        assert_ne!(journal_mode.to_ascii_lowercase(), "mvcc");
    }

    #[tokio::test]
    async fn cdc_keyset_page_uses_limit_plus_one_and_stable_cursor() {
        let storage = TursoCdcStorage::open(config("keyset"))
            .await
            .expect("open CDC keyset profile");
        let connection = storage.connection();
        connection
            .execute(
                "CREATE TABLE cdc_keyset_fixture(id INTEGER PRIMARY KEY, value TEXT NOT NULL)",
                (),
            )
            .await
            .expect("create CDC keyset fixture");
        let after_setup = storage
    .read_page(None, 1_000.into())
            .await
            .expect("read CDC setup cursor")
            .next_change_id;
        for id in 1_i64..=4 {
            connection
                .execute(
                    "INSERT INTO cdc_keyset_fixture(id, value) VALUES (?1, ?2)",
                    (id, format!("value-{id}")),
                )
                .await
                .expect("insert CDC keyset fixture");
        }

        let first = storage
    .read_page(after_setup.map(Into::into), 2.into())
            .await
            .expect("read first CDC keyset page");
        assert_eq!(first.changes.len(), 2);
        assert!(first.has_more);
        let second = storage
    .read_page(first.next_change_id.map(Into::into), 2.into())
            .await
            .expect("read second CDC keyset page");
        assert_eq!(second.changes.len(), 2);
        assert!(
            second.changes.first().unwrap().change_id
                > first.changes.last().unwrap().change_id
        );
    }
}
