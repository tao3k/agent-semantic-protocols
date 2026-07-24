mod turso_encrypted_storage_tests {
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use agent_semantic_client_db::turso_encrypted_storage::{
        TURSO_ENCRYPTION_FILE_RECEIPT_SCHEMA_ID, TursoEncryptedProfileConfig,
        TursoEncryptedStorage, TursoEncryptionCipher, TursoEncryptionKey,
    };

    const KEY: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    const WRONG_KEY: &str =
        "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";

    fn temp_db(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "asp-client-db-encrypted-{name}-{}-{nonce}.turso",
            std::process::id()
        ))
    }

    fn config(path: PathBuf, key: &str) -> TursoEncryptedProfileConfig {
        TursoEncryptedProfileConfig {
            path,
            cipher: TursoEncryptionCipher::Aegis256,
            key: TursoEncryptionKey::from_hex(TursoEncryptionCipher::Aegis256, key)
                .expect("valid fixed encryption key"),
        }
    }

    #[test]
    fn encryption_key_validation_is_typed_and_debug_redacted() {
        let error = TursoEncryptionKey::from_hex(TursoEncryptionCipher::Aegis256, "secret")
            .expect_err("short non-hex key must fail closed");
        assert!(error.contains("64 hexadecimal"));
        let key = TursoEncryptionKey::from_hex(TursoEncryptionCipher::Aegis256, KEY)
            .expect("valid key");
        let debug = format!("{key:?}");
        assert!(debug.contains("REDACTED"));
        assert!(!debug.contains(KEY));
    }

    #[tokio::test]
    async fn encrypted_database_hides_plaintext_reopens_and_rejects_wrong_key() {
        let path = temp_db("durable");
        let marker = "ASP_ENCRYPTED_PLAINTEXT_PROBE_9f6c5f42";
        let storage = TursoEncryptedStorage::open(config(path.clone(), KEY))
            .await
            .expect("open encrypted database");
        let connection = storage.connection();
        connection
            .execute(
                "CREATE TABLE encrypted_fixture(id INTEGER PRIMARY KEY, secret TEXT NOT NULL)",
                (),
            )
            .await
            .expect("create encrypted fixture");
        connection
            .execute(
                "INSERT INTO encrypted_fixture(id, secret) VALUES (?1, ?2)",
                (1_i64, marker),
            )
            .await
            .expect("write encrypted fixture");
        let receipt = storage
            .flush_and_measure(marker.as_bytes())
            .await
            .expect("measure encrypted artifacts");
        assert_eq!(receipt.schema_id, TURSO_ENCRYPTION_FILE_RECEIPT_SCHEMA_ID);
        assert!(receipt.database_bytes + receipt.wal_bytes > 0);
        assert!(!receipt.plaintext_probe_present);
        drop(connection);
        drop(storage);

        let reopened = TursoEncryptedStorage::open(config(path.clone(), KEY))
            .await
            .expect("reopen encrypted database with correct key");
        let recovered: String = reopened
            .connection()
            .query("SELECT secret FROM encrypted_fixture WHERE id = 1", ())
            .await
            .expect("query encrypted fixture")
            .next()
            .await
            .expect("advance encrypted fixture")
            .expect("encrypted fixture row")
            .get(0)
            .expect("decode encrypted fixture");
        assert_eq!(recovered, marker);
        drop(reopened);

        match TursoEncryptedStorage::open(config(path, WRONG_KEY)).await {
            Err(_) => {}
            Ok(wrong) => {
                let unreadable = wrong
                    .connection()
                    .query("SELECT secret FROM encrypted_fixture WHERE id = 1", ())
                    .await;
                assert!(unreadable.is_err(), "wrong key must never expose plaintext");
            }
        }
    }
}
