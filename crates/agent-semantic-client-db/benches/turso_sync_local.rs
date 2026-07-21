use std::hint::black_box;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_client_db::turso_sync_storage::{
    TursoSyncOperationOutcome, TursoSyncProfileConfig, TursoSyncStorage,
};
use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use turso::transaction::TransactionBehavior;

fn temp_db(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "asp-client-db-bench-sync-{name}-{}-{nonce}.turso",
        std::process::id()
    ))
}

fn config(path: PathBuf) -> TursoSyncProfileConfig {
    TursoSyncProfileConfig {
        path,
        remote_url: "http://127.0.0.1:1".to_owned(),
        auth_token: "fixed-benchmark-token".to_owned(),
        bootstrap_if_empty: false,
    }
}

fn turso_sync_local(c: &mut Criterion) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("benchmark runtime");
    let storage = runtime
        .block_on(TursoSyncStorage::open(config(temp_db("local"))))
        .expect("open local-first sync database");
    let mut connection = runtime
        .block_on(storage.connect())
        .expect("connect local-first sync database");
    runtime
        .block_on(connection.execute(
            "CREATE TABLE IF NOT EXISTS sync_bench(\
                id INTEGER PRIMARY KEY, payload BLOB NOT NULL\
             )",
            (),
        ))
        .expect("create sync benchmark table");
    let mut next_id = 0_i64;
    let payload = vec![0x5a_u8; 1024];

    let mut group = c.benchmark_group("client_db_turso_sync_local");
    group.sample_size(20);
    group.throughput(Throughput::Elements(32));
    group.bench_function("append_32_plus_stats", |b| {
        b.iter(|| {
            runtime.block_on(async {
                let transaction = connection
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .await
                    .expect("begin sync local batch");
                let mut statement = transaction
                    .prepare_cached("INSERT INTO sync_bench(id, payload) VALUES (?1, ?2)")
                    .await
                    .expect("prepare sync local insert");
                for _ in 0..32 {
                    next_id += 1;
                    statement
                        .execute((next_id, payload.as_slice()))
                        .await
                        .expect("insert sync local row");
                }
                drop(statement);
                transaction.commit().await.expect("commit sync local batch");
                let stats = storage.stats().await;
                assert_eq!(stats.outcome, TursoSyncOperationOutcome::Observed);
                black_box(stats);
            });
        });
    });

    group.throughput(Throughput::Elements(256));
    group.bench_function("append_256_plus_explicit_checkpoint", |b| {
        b.iter(|| {
            runtime.block_on(async {
                let transaction = connection
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .await
                    .expect("begin checkpoint batch");
                let mut statement = transaction
                    .prepare_cached("INSERT INTO sync_bench(id, payload) VALUES (?1, ?2)")
                    .await
                    .expect("prepare checkpoint batch insert");
                for _ in 0..256 {
                    next_id += 1;
                    statement
                        .execute((next_id, payload.as_slice()))
                        .await
                        .expect("insert checkpoint batch row");
                }
                drop(statement);
                transaction.commit().await.expect("commit checkpoint batch");
                let checkpoint = storage.checkpoint().await;
                assert_eq!(checkpoint.outcome, TursoSyncOperationOutcome::Applied);
                assert!(checkpoint.stats.is_some());
                black_box(checkpoint);
            });
        });
    });
    group.finish();
}

criterion_group!(benches, turso_sync_local);
criterion_main!(benches);
