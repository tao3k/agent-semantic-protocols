use std::hint::black_box;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_client_db::turso_encrypted_storage::{
    TursoEncryptedProfileConfig, TursoEncryptedStorage, TursoEncryptionCipher, TursoEncryptionKey,
};
use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use turso::transaction::TransactionBehavior;

const KEY: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn temp_db(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "asp-client-db-bench-encryption-{name}-{}-{nonce}.turso",
        std::process::id()
    ))
}

async fn seed(connection: &mut turso::Connection, table: &str, rows: i64) {
    connection
        .execute(
            &format!("CREATE TABLE {table}(id INTEGER PRIMARY KEY, payload BLOB NOT NULL)"),
            (),
        )
        .await
        .expect("create encryption benchmark table");
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .await
        .expect("begin encryption benchmark seed");
    let mut statement = transaction
        .prepare_cached(&format!("INSERT INTO {table}(id, payload) VALUES (?1, ?2)"))
        .await
        .expect("prepare encryption benchmark seed");
    let payload = vec![0x73_u8; 1_024];
    for id in 1_i64..=rows {
        statement
            .execute((id, payload.as_slice()))
            .await
            .expect("insert encryption benchmark seed");
    }
    drop(statement);
    transaction
        .commit()
        .await
        .expect("commit encryption benchmark seed");
}

fn turso_encryption_profile(c: &mut Criterion) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("benchmark runtime");
    let encrypted_path = temp_db("encrypted");
    let encrypted = runtime
        .block_on(TursoEncryptedStorage::open(TursoEncryptedProfileConfig {
            path: encrypted_path,
            cipher: TursoEncryptionCipher::Aegis256,
            key: TursoEncryptionKey::from_hex(TursoEncryptionCipher::Aegis256, KEY)
                .expect("benchmark encryption key"),
        }))
        .expect("open encrypted benchmark database");
    let mut encrypted_connection = encrypted.connection();
    runtime.block_on(seed(&mut encrypted_connection, "encrypted_bench", 1_000));

    let plain_path = temp_db("plain");
    let plain_database = runtime
        .block_on(
            turso::Builder::new_local(plain_path.to_string_lossy().as_ref())
                .experimental_multiprocess_wal(true)
                .build(),
        )
        .expect("open plain comparison database");
    let mut plain_connection = plain_database
        .connect()
        .expect("connect plain comparison database");
    runtime.block_on(seed(&mut plain_connection, "plain_bench", 1_000));

    let payload = vec![0x45_u8; 1_024];
    let mut encrypted_id = 1_000_i64;
    let mut plain_id = 1_000_i64;
    let mut group = c.benchmark_group("client_db_turso_encryption_aegis256");
    group.sample_size(20);
    group.throughput(Throughput::Elements(32));

    group.bench_function("encrypted_append_32", |b| {
        b.iter(|| {
            runtime.block_on(async {
                let transaction = encrypted_connection
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .await
                    .expect("begin encrypted append");
                let mut statement = transaction
                    .prepare_cached("INSERT INTO encrypted_bench(id, payload) VALUES (?1, ?2)")
                    .await
                    .expect("prepare encrypted append");
                for _ in 0..32 {
                    encrypted_id += 1;
                    statement
                        .execute((encrypted_id, payload.as_slice()))
                        .await
                        .expect("insert encrypted benchmark row");
                }
                drop(statement);
                transaction.commit().await.expect("commit encrypted append");
            });
        });
    });

    group.bench_function("plain_append_32", |b| {
        b.iter(|| {
            runtime.block_on(async {
                let transaction = plain_connection
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .await
                    .expect("begin plain append");
                let mut statement = transaction
                    .prepare_cached("INSERT INTO plain_bench(id, payload) VALUES (?1, ?2)")
                    .await
                    .expect("prepare plain append");
                for _ in 0..32 {
                    plain_id += 1;
                    statement
                        .execute((plain_id, payload.as_slice()))
                        .await
                        .expect("insert plain benchmark row");
                }
                drop(statement);
                transaction.commit().await.expect("commit plain append");
            });
        });
    });

    group.throughput(Throughput::Elements(100));
    group.bench_function("encrypted_keyset_read_100", |b| {
        b.iter(|| {
            let count = runtime.block_on(async {
                let mut rows = encrypted_connection
                    .query(
                        "SELECT payload FROM encrypted_bench WHERE id > ?1 ORDER BY id LIMIT 100",
                        [500_i64],
                    )
                    .await
                    .expect("query encrypted keyset page");
                let mut count = 0;
                while rows
                    .next()
                    .await
                    .expect("advance encrypted keyset")
                    .is_some()
                {
                    count += 1;
                }
                count
            });
            assert_eq!(count, 100);
            black_box(count);
        });
    });

    group.bench_function("plain_keyset_read_100", |b| {
        b.iter(|| {
            let count = runtime.block_on(async {
                let mut rows = plain_connection
                    .query(
                        "SELECT payload FROM plain_bench WHERE id > ?1 ORDER BY id LIMIT 100",
                        [500_i64],
                    )
                    .await
                    .expect("query plain keyset page");
                let mut count = 0;
                while rows.next().await.expect("advance plain keyset").is_some() {
                    count += 1;
                }
                count
            });
            assert_eq!(count, 100);
            black_box(count);
        });
    });
    group.finish();
}

criterion_group!(benches, turso_encryption_profile);
criterion_main!(benches);
