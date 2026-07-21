use agent_semantic_client_db::turso_mvcc_store::{
    TursoMvccEvent, TursoMvccStore, TursoMvccStoreConfig,
};
use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use std::sync::atomic::{AtomicU64, Ordering};

const EVENTS_PER_PARTITION: usize = 512;
const PARTITIONS: [&str; 4] = ["agent-a", "agent-b", "agent-c", "agent-d"];

fn workload(run: u64) -> [Vec<TursoMvccEvent>; 4] {
    PARTITIONS.map(|partition| {
        (0..EVENTS_PER_PARTITION)
            .map(|offset| TursoMvccEvent {
                partition_key: partition.to_string(),
                event_id: format!("{run:016}-{offset:04}"),
                payload: vec![b'x'; 128],
                created_at_ms: run as i64,
            })
            .collect()
    })
}

fn run_concurrent_batches(store: &TursoMvccStore, batches: [Vec<TursoMvccEvent>; 4]) {
    std::thread::scope(|scope| {
        let handles = batches.map(|batch| {
            scope.spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("build Turso MVCC writer runtime");
                runtime.block_on(store.append_batch(&batch))
            })
        });
        for handle in handles {
            handle
                .join()
                .expect("join Turso MVCC writer")
                .expect("append Turso MVCC benchmark batch");
        }
    });
}

fn turso_mvcc_concurrent_write(criterion: &mut Criterion) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build benchmark setup runtime");
    let temp = std::env::temp_dir().join(format!("asp-turso-mvcc-bench-{}", std::process::id()));
    std::fs::create_dir_all(&temp).expect("create benchmark tempdir");
    let mut single_lane_config = TursoMvccStoreConfig::new(temp.join("single-lane.turso"));
    single_lane_config.connection_lanes = 1;
    let single_lane = runtime
        .block_on(TursoMvccStore::open(single_lane_config))
        .expect("open single-lane Turso MVCC store");
    let four_lane = runtime
        .block_on(TursoMvccStore::open(TursoMvccStoreConfig::new(
            temp.join("four-lane.turso"),
        )))
        .expect("open four-lane Turso MVCC store");
    let sequence = AtomicU64::new(0);

    let mut group = criterion.benchmark_group("turso_mvcc_concurrent_write");
    group.throughput(criterion::Throughput::Elements(
        (PARTITIONS.len() * EVENTS_PER_PARTITION) as u64,
    ));
    group.bench_function("one_lane_4x512_events", |bencher| {
        bencher.iter_batched(
            || workload(sequence.fetch_add(1, Ordering::Relaxed)),
            |batches| run_concurrent_batches(&single_lane, batches),
            BatchSize::SmallInput,
        );
    });
    group.bench_function("four_lanes_4x512_events", |bencher| {
        bencher.iter_batched(
            || workload(sequence.fetch_add(1, Ordering::Relaxed)),
            |batches| run_concurrent_batches(&four_lane, batches),
            BatchSize::SmallInput,
        );
    });
    group.finish();

    drop(single_lane);
    drop(four_lane);
    let _ = std::fs::remove_dir_all(temp);
}

criterion_group!(benches, turso_mvcc_concurrent_write);
criterion_main!(benches);
