// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Pinned Turso 0.7 MVCC capability gate over the production source-index schema.

use blake3::Hasher;
use tempfile::TempDir;
use turso::{Error, Value};

#[tokio::test(flavor = "current_thread")]
async fn pinned_turso_0_7_mvcc_supports_production_source_index_contract() {
    let fixture = TempDir::new().expect("create MVCC capability tempfile");
    let db_path = fixture.path().join("facts.turso");
    let database = open_existing_mvcc_database(&db_path).await;
    let bootstrap = database
        .connect()
        .expect("connect production schema bootstrap");

    let mut mode_rows = bootstrap
        .query("PRAGMA journal_mode=mvcc", ())
        .await
        .expect("enable MVCC for capability database");
    let mode = mode_rows
        .next()
        .await
        .expect("read MVCC mode result")
        .expect("MVCC mode result row")
        .get::<String>(0)
        .expect("decode MVCC mode");
    assert_eq!(mode, "mvcc", "pinned Turso must actually enter MVCC mode");
    drop(mode_rows);

    super::schema::bootstrap_turso_source_index_schema(&bootstrap)
        .await
        .expect("production source-index schema and indexes must bootstrap under pinned MVCC");
    insert_typed_fixtures(&bootstrap).await;
    assert_production_index_hit(&bootstrap).await;
    assert_typed_roundtrip(&bootstrap).await;

    insert_generation(
        &bootstrap,
        "provider-single-writer",
        "generation-single-writer",
    )
    .await;

    let before_restart = production_digest(&bootstrap).await;
    bootstrap
        .cacheflush()
        .expect("flush MVCC capability database");
    let mut checkpoint = bootstrap
        .query("PRAGMA wal_checkpoint(TRUNCATE)", ())
        .await
        .expect("checkpoint MVCC capability database");
    let checkpoint_row = checkpoint
        .next()
        .await
        .expect("read checkpoint result")
        .expect("checkpoint result row");
    assert_eq!(
        checkpoint_row
            .get::<i64>(0)
            .expect("decode checkpoint busy"),
        0
    );
    drop(checkpoint);
    drop(bootstrap);
    drop(database);

    let reopened = open_existing_mvcc_database(&db_path).await;
    let connection = reopened.connect().expect("connect reopened MVCC database");
    assert_journal_mode_mvcc(&connection).await;
    assert_eq!(
        production_digest(&connection).await,
        before_restart,
        "checkpoint/drop/reopen must preserve production schema and facts"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn pinned_turso_0_7_1_characterizes_generic_same_row_conflict() {
    let fixture = TempDir::new().expect("create MVCC conflict characterization tempfile");
    let db_path = fixture.path().join("facts.turso");
    let database = open_existing_mvcc_database(&db_path).await;
    let bootstrap = database
        .connect()
        .expect("connect characterization bootstrap");
    let mut mode_rows = bootstrap
        .query("PRAGMA journal_mode=mvcc", ())
        .await
        .expect("enable MVCC for conflict characterization");
    let mode = mode_rows
        .next()
        .await
        .expect("read characterization MVCC result")
        .expect("characterization MVCC row")
        .get::<String>(0)
        .expect("decode characterization MVCC mode");
    assert_eq!(mode, "mvcc");
    drop(mode_rows);
    super::schema::bootstrap_turso_source_index_schema(&bootstrap)
        .await
        .expect("bootstrap production schema for conflict characterization");
    insert_generation(&bootstrap, "provider-conflict", "generation-conflict").await;

    let writer_a = database
        .connect()
        .expect("connect first characterization writer");
    let writer_b = database
        .connect()
        .expect("connect second characterization writer");
    writer_a
        .execute("BEGIN CONCURRENT", ())
        .await
        .expect("begin first characterization transaction");
    writer_b
        .execute("BEGIN CONCURRENT", ())
        .await
        .expect("begin second characterization transaction");
    update_generation(&writer_a, "provider-conflict")
        .await
        .expect("update first characterization writer");
    let error = update_generation(&writer_b, "provider-conflict")
        .await
        .expect_err("pinned Turso 0.7.1 must expose current generic conflict");
    assert!(
        matches!(&error, Error::Error(message) if message == "Write-write conflict"),
        "pinned Turso 0.7.1 conflict characterization drifted: {error:?}"
    );
}

async fn open_existing_mvcc_database(path: &std::path::Path) -> turso::Database {
    turso::Builder::new_local(path.to_str().expect("UTF-8 MVCC tempfile path"))
        .experimental_mvcc_passive_checkpoint(true)
        .build()
        .await
        .expect("open pinned Turso 0.7 MVCC tempfile")
}

async fn assert_journal_mode_mvcc(connection: &turso::Connection) {
    let mut rows = connection
        .query("PRAGMA journal_mode", ())
        .await
        .expect("query reopened journal mode");
    let mode = rows
        .next()
        .await
        .expect("read reopened journal mode")
        .expect("reopened journal mode row")
        .get::<String>(0)
        .expect("decode reopened journal mode");
    assert_eq!(mode, "mvcc");
}

async fn insert_typed_fixtures(connection: &turso::Connection) {
    connection
        .execute(
            "INSERT INTO asp_source_index_owner_v1 (
                project_root, schema_id, schema_version, generation_id, file_hash,
                owner_path, language_id, provider_id, source_kind, line_count,
                query_keys_json, selector_facts_json, term_tokens_json, selector_count
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            vec![
                Value::Text("/tmp/workspace".into()),
                Value::Text("source-index".into()),
                Value::Text("1".into()),
                Value::Text("generation-1".into()),
                Value::Text("file-digest".into()),
                Value::Text("src/lib.rs".into()),
                Value::Null,
                Value::Text("rust-provider".into()),
                Value::Text("provider-index".into()),
                Value::Real(12.5),
                Value::Text("[\"run\"]".into()),
                Value::Text("[]".into()),
                Value::Text("[\"run\"]".into()),
                Value::Integer(1),
            ],
        )
        .await
        .expect("insert NULL/real/integer/text production row");
    connection
        .execute(
            "INSERT INTO asp_source_index_blob_v1 (
                content_digest, size_bytes, source_bytes
             ) VALUES (?1, ?2, ?3)",
            vec![
                Value::Text("blob-digest".into()),
                Value::Integer(19),
                Value::Blob(br#"{"kind":"function"}"#.to_vec()),
            ],
        )
        .await
        .expect("insert BLOB production row");
    connection
        .execute(
            "INSERT INTO provider_owner_inventory_entry_v1 (
                project_root, workspace_identity, provider_workspace_identity_digest,
                provider_id, inventory_generation, owner_path,
                owner_content_digest, owner_state
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            (
                "/tmp/workspace",
                "workspace-1",
                "provider-workspace-digest",
                "rust-provider",
                "inventory-1",
                "src/lib.rs",
                Option::<String>::None,
                "known",
            ),
        )
        .await
        .expect("insert indexed production inventory row");
}

async fn assert_production_index_hit(connection: &turso::Connection) {
    let mut rows = connection
        .query(
            "EXPLAIN QUERY PLAN
             SELECT owner_path FROM provider_owner_inventory_entry_v1
             WHERE project_root = ?1
               AND workspace_identity = ?2
               AND provider_workspace_identity_digest = ?3
               AND provider_id = ?4
               AND owner_path = ?5",
            (
                "/tmp/workspace",
                "workspace-1",
                "provider-workspace-digest",
                "rust-provider",
                "src/lib.rs",
            ),
        )
        .await
        .expect("plan production indexed lookup");
    let mut plan = Vec::new();
    while let Some(row) = rows.next().await.expect("read production query plan") {
        plan.push(row.get::<String>(3).expect("decode production query plan"));
    }
    assert!(
        plan.iter()
            .any(|line| line.contains("provider_owner_inventory_entry_v1_queue_idx")),
        "production lookup did not use explicit index: {plan:?}"
    );
}

async fn assert_typed_roundtrip(connection: &turso::Connection) {
    let mut rows = connection
        .query(
            "SELECT owner_path, language_id, line_count, selector_count
             FROM asp_source_index_owner_v1 WHERE owner_path = ?1",
            ["src/lib.rs"],
        )
        .await
        .expect("query typed owner row");
    let row = rows
        .next()
        .await
        .expect("read typed owner row")
        .expect("typed owner row");
    assert_eq!(
        row.get_value(0).expect("text"),
        Value::Text("src/lib.rs".into())
    );
    assert_eq!(row.get_value(1).expect("null"), Value::Null);
    assert_eq!(row.get_value(2).expect("real"), Value::Real(12.5));
    assert_eq!(row.get_value(3).expect("integer"), Value::Integer(1));

    let mut blobs = connection
        .query(
            "SELECT source_bytes FROM asp_source_index_blob_v1
             WHERE content_digest = ?1",
            ["blob-digest"],
        )
        .await
        .expect("query BLOB row");
    let blob = blobs
        .next()
        .await
        .expect("read BLOB row")
        .expect("BLOB row");
    assert_eq!(
        blob.get_value(0).expect("blob"),
        Value::Blob(br#"{"kind":"function"}"#.to_vec())
    );
}

async fn insert_generation(connection: &turso::Connection, provider_id: &str, generation_id: &str) {
    connection
        .execute(
            "INSERT INTO provider_active_generation_v1 (
                project_root, workspace_identity, provider_workspace_identity_digest,
                provider_id, language_id, provider_workspace_root,
                generation_id, owner_count, updated_at_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            (
                "/tmp/workspace",
                "workspace-1",
                "provider-workspace-digest",
                provider_id,
                "rust",
                "/tmp/workspace",
                generation_id,
                1_i64,
                1_i64,
            ),
        )
        .await
        .expect("insert active generation");
}

async fn update_generation(
    connection: &turso::Connection,
    provider_id: &str,
) -> Result<(), turso::Error> {
    connection
        .execute(
            "UPDATE provider_active_generation_v1
             SET owner_count = owner_count + 1
             WHERE project_root = ?1
               AND workspace_identity = ?2
               AND provider_workspace_identity_digest = ?3
               AND provider_id = ?4",
            (
                "/tmp/workspace",
                "workspace-1",
                "provider-workspace-digest",
                provider_id,
            ),
        )
        .await
        .map(|_| ())
}

async fn production_digest(connection: &turso::Connection) -> String {
    let mut hasher = Hasher::new();
    hash_rows(
        connection,
        &mut hasher,
        "SELECT type, name, tbl_name, sql FROM sqlite_schema
         WHERE sql IS NOT NULL AND name NOT LIKE 'sqlite_%' AND name NOT LIKE '__turso_%'
         ORDER BY type, name",
        4,
    )
    .await;
    hash_rows(
        connection,
        &mut hasher,
        "SELECT owner_path, language_id, line_count, selector_count
         FROM asp_source_index_owner_v1 ORDER BY owner_path",
        4,
    )
    .await;
    hash_rows(
        connection,
        &mut hasher,
        "SELECT provider_id, generation_id, owner_count
         FROM provider_active_generation_v1 ORDER BY provider_id",
        3,
    )
    .await;
    hasher.finalize().to_hex().to_string()
}

async fn hash_rows(connection: &turso::Connection, hasher: &mut Hasher, sql: &str, columns: usize) {
    let mut rows = connection.query(sql, ()).await.expect("query digest rows");
    while let Some(row) = rows.next().await.expect("read digest row") {
        for index in 0..columns {
            hash_value(hasher, &row.get_value(index).expect("decode digest value"));
        }
    }
}

fn hash_value(hasher: &mut Hasher, value: &Value) {
    let encoded = match value {
        Value::Null => vec![0],
        Value::Integer(value) => [vec![1], value.to_le_bytes().to_vec()].concat(),
        Value::Real(value) => [vec![2], value.to_bits().to_le_bytes().to_vec()].concat(),
        Value::Text(value) => [vec![3], value.as_bytes().to_vec()].concat(),
        Value::Blob(value) => [vec![4], value.clone()].concat(),
    };
    hasher.update(&(encoded.len() as u64).to_le_bytes());
    hasher.update(&encoded);
}
