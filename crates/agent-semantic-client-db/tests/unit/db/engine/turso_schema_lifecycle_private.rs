use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use super::turso::{bootstrap_turso_schema_version, turso_table_exists};

fn temp_db(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "asp-client-db-schema-{name}-{}-{nonce}.turso",
        std::process::id()
    ))
}

async fn open(path: &PathBuf) -> (turso::Database, turso::Connection) {
    let path = path.to_string_lossy();
    let database = turso::Builder::new_local(path.as_ref())
        .experimental_multiprocess_wal(true)
        .build()
        .await
        .expect("open schema lifecycle database");
    let connection = database
        .connect()
        .expect("connect schema lifecycle database");
    (database, connection)
}

async fn schema_version(connection: &turso::Connection) -> i64 {
    connection
        .query(
            "SELECT schema_version FROM asp_db_engine_bootstrap LIMIT 1",
            (),
        )
        .await
        .expect("query schema version")
        .next()
        .await
        .expect("advance schema version row")
        .expect("schema version row")
        .get(0)
        .expect("decode schema version")
}

#[tokio::test]
async fn schema_v1_bootstrap_is_idempotent_and_preserves_authority_data() {
    let path = temp_db("idempotent");
    let (_database, mut connection) = open(&path).await;
    bootstrap_turso_schema_version(&mut connection)
        .await
        .expect("bootstrap stable schema v1");
    assert_eq!(schema_version(&connection).await, 1);
    assert!(
        turso_table_exists(&connection, "asp_db_engine_migration")
            .await
            .unwrap()
    );
    assert!(
        turso_table_exists(&connection, "asp_artifact_pointer")
            .await
            .unwrap()
    );
    assert!(
        turso_table_exists(&connection, "asp_failed_artifact_attempt")
            .await
            .unwrap()
    );

    connection
        .execute(
            "INSERT INTO asp_artifact_pointer (\
                repo_id, workspace_id, scope_id, pointer_kind, pointer_name,\
                current_root_hash, revision, updated_at_ms\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            turso::params![
                "repo:schema-test",
                "workspace:schema-test",
                "scope:schema-test",
                "topology-root",
                "main",
                "sha256:durable",
                1_i64,
                1_i64
            ],
        )
        .await
        .expect("insert authority fixture");

    bootstrap_turso_schema_version(&mut connection)
        .await
        .expect("repeated bootstrap is a verified no-op");
    let count: i64 = connection
        .query("SELECT COUNT(*) FROM asp_artifact_pointer", ())
        .await
        .expect("query preserved pointer")
        .next()
        .await
        .expect("advance preserved pointer count")
        .expect("preserved pointer count row")
        .get(0)
        .expect("decode pointer count");
    assert_eq!(count, 1);
}

#[tokio::test]
async fn incomplete_schema_v1_is_stabilized_once_without_losing_preexisting_data() {
    let path = temp_db("v1-stabilization");
    let (_database, mut connection) = open(&path).await;
    connection
        .execute_batch(
            "CREATE TABLE asp_db_engine_bootstrap(schema_version INTEGER NOT NULL);\
             INSERT INTO asp_db_engine_bootstrap(schema_version) VALUES (1);\
             CREATE TABLE preexisting_fixture(id INTEGER PRIMARY KEY, value TEXT NOT NULL);\
             INSERT INTO preexisting_fixture(id, value) VALUES (1, 'preserve-me');",
        )
        .await
        .expect("create schema v1 fixture");

    bootstrap_turso_schema_version(&mut connection)
        .await
        .expect("stabilize existing v1");
    assert_eq!(schema_version(&connection).await, 1);
    let fixture: String = connection
        .query("SELECT value FROM preexisting_fixture WHERE id = 1", ())
        .await
        .expect("query preexisting fixture")
        .next()
        .await
        .expect("advance preexisting fixture")
        .expect("preexisting fixture row")
        .get(0)
        .expect("decode preexisting fixture");
    assert_eq!(fixture, "preserve-me");
    let migration_count: i64 = connection
        .query(
            "SELECT COUNT(*) FROM asp_db_engine_migration WHERE schema_version = 1",
            (),
        )
        .await
        .expect("query migration history")
        .next()
        .await
        .expect("advance migration history")
        .expect("migration history row")
        .get(0)
        .expect("decode migration count");
    assert_eq!(migration_count, 1);
}

#[tokio::test]
async fn unknown_newer_schema_version_fails_closed_without_mutation() {
    let path = temp_db("newer-version");
    let (_database, mut connection) = open(&path).await;
    connection
        .execute_batch(
            "CREATE TABLE asp_db_engine_bootstrap(schema_version INTEGER NOT NULL);\
             INSERT INTO asp_db_engine_bootstrap(schema_version) VALUES (99);",
        )
        .await
        .expect("create newer schema fixture");

    let error = bootstrap_turso_schema_version(&mut connection)
        .await
        .expect_err("newer schema must fail closed");
    assert!(error.contains("unsupported newer"));
    assert_eq!(schema_version(&connection).await, 99);
    assert!(
        !turso_table_exists(&connection, "asp_artifact_pointer")
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn failed_v1_stabilization_rolls_back_tables_and_version() {
    let path = temp_db("rollback");
    let (_database, mut connection) = open(&path).await;
    connection
        .execute_batch(
            "CREATE TABLE asp_db_engine_bootstrap(schema_version INTEGER NOT NULL);\
             INSERT INTO asp_db_engine_bootstrap(schema_version) VALUES (1);\
             CREATE VIEW asp_db_engine_migration AS SELECT 1 AS schema_version;",
        )
        .await
        .expect("create migration failure fixture");

    let error = bootstrap_turso_schema_version(&mut connection)
        .await
        .expect_err("invalid migration history object must fail migration");
    assert!(error.contains("record Turso client DB stabilization"));
    assert_eq!(schema_version(&connection).await, 1);
    assert!(
        !turso_table_exists(&connection, "asp_artifact_pointer")
            .await
            .unwrap()
    );
    assert!(
        !turso_table_exists(&connection, "asp_failed_artifact_attempt")
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn exact_selector_projection_round_trip_hydrates_a_validated_merkle_record() {
    let root = temp_db("exact-selector-merkle-round-trip");
    std::fs::create_dir_all(&root).expect("create exact-selector client directory");
    let owner_path = "src/lib.rs";
    let selector = "rust://src/lib.rs#item/function/cached_symbol";
    let source = b"fn cached_symbol() -> usize { 255 }\n";
    let source_blob_digest =
        agent_semantic_content_identity::exact_selector_merkle::blake3_content_digest_v1(source);
    let parser_identity_digest =
        agent_semantic_content_identity::exact_selector_merkle::canonical_content_digest_v1(
            b"parser",
            &[b"rs-harness"],
        );
    let query_pack_digest =
        agent_semantic_content_identity::exact_selector_merkle::canonical_content_digest_v1(
            b"query-pack",
            &[b"rust"],
        );
    let tree = agent_semantic_content_identity::workspace_merkle_v1::WorkspacePathMerkleTreeV1::from_file_digests([
        (owner_path.to_string(), source_blob_digest.clone()),
    ])
    .expect("build exact-selector workspace tree");
    let packet = agent_semantic_content_identity::exact_selector_projection_packet::build_exact_selector_projection_packet_v1(
        "rust",
        "rs-harness",
        &parser_identity_digest,
        &query_pack_digest,
        owner_path,
        selector,
        agent_semantic_content_identity::exact_selector_merkle::ExactProjectionModeV1::Code,
        source,
        br#"{"kind":"fn","name":"cached_symbol"}"#,
        source,
    );
    let record = packet
        .enrich_projection_record(&tree)
        .expect("enrich exact-selector projection with Merkle proof");
    let key =
        agent_semantic_content_identity::exact_selector_cache::ExactSelectorMerkleLookupKeyV1 {
            language_id: "rust",
            workspace_root_digest: tree.root_digest(),
            owner_path,
            owner_subtree_digest: tree
                .owner_subtree_digest(owner_path)
                .expect("resolve exact-selector owner subtree"),
            source_blob_digest: &source_blob_digest,
            parser_identity_digest: &parser_identity_digest,
            query_pack_digest: &query_pack_digest,
            structural_selector: selector,
            projection_mode:
                agent_semantic_content_identity::exact_selector_merkle::ExactProjectionModeV1::Code,
        };

    crate::ClientDbEngine::persist_exact_selector_projection_v1_from_client_dir(
        &root, &key, &record,
    )
    .expect("persist validated exact-selector projection");
    let validated =
        crate::ClientDbEngine::lookup_exact_selector_projection_v1_from_client_dir(&root, &key)
            .expect("lookup exact-selector projection")
            .expect("warm exact-selector projection");
    let hit = validated
        .validate_warm_hit(&key)
        .expect("validate hydrated exact-selector projection");

    assert_eq!(hit.projection_payload, source);
    assert_eq!(hit.side_effects.parser_process_count, 0);
    assert_eq!(hit.side_effects.content_store_write_count, 0);
    assert_eq!(hit.side_effects.turso_write_count, 0);
    assert_eq!(hit.side_effects.manifest_write_count, 0);
    std::fs::remove_dir_all(root).expect("remove exact-selector client directory");
}
