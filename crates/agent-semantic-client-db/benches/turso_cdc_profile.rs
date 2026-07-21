use std::hint::black_box;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_client_db::turso_cdc_storage::{
    TursoCdcCaptureMode, TursoCdcChangeKind, TursoCdcProfileConfig, TursoCdcStorage,
};
use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use turso::transaction::TransactionBehavior;

fn temp_db() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "asp-client-db-bench-cdc-{}-{nonce}.turso",
        std::process::id()
    ))
}

fn turso_cdc_profile(c: &mut Criterion) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("benchmark runtime");
    let storage = runtime
        .block_on(TursoCdcStorage::open(TursoCdcProfileConfig {
            path: temp_db(),
            mode: TursoCdcCaptureMode::Full,
            table_name: "asp_bench_cdc".to_owned(),
        }))
        .expect("open benchmark CDC profile");
    let mut connection = storage.connection();
    runtime
        .block_on(connection.execute(
            "CREATE TABLE cdc_bench_fixture(id INTEGER PRIMARY KEY, payload BLOB NOT NULL)",
            (),
        ))
        .expect("create CDC benchmark fixture");
    let mut cursor = runtime
        .block_on(storage.read_page(None, 1_000))
        .expect("read CDC setup cursor")
        .next_change_id;
    let mut next_id = 0_i64;
    let payload = vec![0x3c_u8; 1_024];

    let mut group = c.benchmark_group("client_db_turso_cdc_non_mvcc");
    group.sample_size(20);
    group.throughput(Throughput::Elements(256));
    group.bench_function("commit_256_plus_keyset_tail", |b| {
        b.iter(|| {
            runtime.block_on(async {
                let transaction = connection
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .await
                    .expect("begin CDC benchmark transaction");
                let mut statement = transaction
                    .prepare_cached("INSERT INTO cdc_bench_fixture(id, payload) VALUES (?1, ?2)")
                    .await
                    .expect("prepare CDC benchmark insert");
                for _ in 0..256 {
                    next_id += 1;
                    statement
                        .execute((next_id, payload.as_slice()))
                        .await
                        .expect("insert CDC benchmark row");
                }
                drop(statement);
                transaction
                    .commit()
                    .await
                    .expect("commit CDC benchmark batch");
                let page = storage
                    .read_page(cursor, 1_000)
                    .await
                    .expect("read CDC benchmark tail");
                assert_eq!(
                    page.changes
                        .iter()
                        .filter(|change| {
                            change.table_name.as_deref() == Some("cdc_bench_fixture")
                                && change.kind == TursoCdcChangeKind::Insert
                        })
                        .count(),
                    256
                );
                assert!(!page.has_more);
                cursor = page.next_change_id;
                black_box(page);
            });
        });
    });

    group.bench_function("rollback_256_plus_empty_tail", |b| {
        b.iter(|| {
            runtime.block_on(async {
                let transaction = connection
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .await
                    .expect("begin CDC rollback benchmark transaction");
                let mut statement = transaction
                    .prepare_cached("INSERT INTO cdc_bench_fixture(id, payload) VALUES (?1, ?2)")
                    .await
                    .expect("prepare CDC rollback benchmark insert");
                for offset in 1_i64..=256 {
                    statement
                        .execute((next_id + offset, payload.as_slice()))
                        .await
                        .expect("insert rolled-back CDC benchmark row");
                }
                drop(statement);
                transaction
                    .rollback()
                    .await
                    .expect("roll back CDC benchmark batch");
                let page = storage
                    .read_page(cursor, 1_000)
                    .await
                    .expect("read CDC tail after rollback");
                assert!(
                    page.changes.iter().all(|change| {
                        change.table_name.as_deref() != Some("cdc_bench_fixture")
                    }),
                    "rolled-back rows must not be visible in CDC"
                );
                black_box(page);
            });
        });
    });
    group.finish();
}

criterion_group!(benches, turso_cdc_profile);
criterion_main!(benches);
