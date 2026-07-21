use std::hint::black_box;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_client_db::ClientDbEngine;
use criterion::{Criterion, Throughput, criterion_group, criterion_main};

fn temp_project(name: &str, sequence: u64) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "asp-client-db-schema-v1-{name}-{}-{nonce}-{sequence}",
        std::process::id()
    ))
}

fn prepare_project(root: &PathBuf) {
    std::fs::create_dir_all(root.join(".git")).expect("create benchmark project root");
}

fn schema_v1_lifecycle(c: &mut Criterion) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("benchmark runtime");
    let warm_root = temp_project("warm", 0);
    prepare_project(&warm_root);
    let warm_engine = ClientDbEngine::resolve(&warm_root).expect("resolve warm client DB engine");
    runtime
        .block_on(warm_engine.bootstrap_active_turso())
        .expect("initial stable schema v1 bootstrap");

    let mut group = c.benchmark_group("client_db_schema_v1_lifecycle");
    group.sample_size(20);
    group.throughput(Throughput::Elements(1));
    group.bench_function("warm_verified_noop_bootstrap", |b| {
        b.iter(|| {
            let report = runtime
                .block_on(warm_engine.bootstrap_active_turso())
                .expect("stable v1 no-op bootstrap");
            assert_eq!(report.schema_version, 1);
            black_box(report);
        });
    });

    let cold_sequence = AtomicU64::new(0);
    group.bench_function("cold_new_project_bootstrap", |b| {
        b.iter(|| {
            let sequence = cold_sequence.fetch_add(1, Ordering::Relaxed);
            let root = temp_project("cold", sequence);
            prepare_project(&root);
            let engine = ClientDbEngine::resolve(&root).expect("resolve cold client DB engine");
            let report = runtime
                .block_on(engine.bootstrap_active_turso())
                .expect("cold stable schema v1 bootstrap");
            assert_eq!(report.schema_version, 1);
            std::fs::remove_dir_all(&root).expect("remove benchmark-owned temporary project");
            black_box(report);
        });
    });
    group.finish();

    std::fs::remove_dir_all(&warm_root).expect("remove warm benchmark project");
}

criterion_group!(benches, schema_v1_lifecycle);
criterion_main!(benches);
