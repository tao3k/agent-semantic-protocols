use std::hint::black_box;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_client_db::artifact_pointer_store::{
    ClientDbArtifactPointerCasOutcome, ClientDbArtifactPointerCasRequest,
    ClientDbArtifactPointerKey, TursoArtifactPointerStore,
};
use agent_semantic_content_identity::hash_blob;
use criterion::{Criterion, Throughput, criterion_group, criterion_main};

fn temp_db(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "asp-client-db-bench-artifact-pointer-{name}-{}-{nonce}.turso",
        std::process::id()
    ))
}

fn key(pointer_name: impl Into<String>) -> ClientDbArtifactPointerKey {
    ClientDbArtifactPointerKey {
        repo_id: "repo:benchmark".to_owned(),
        workspace_id: "workspace:benchmark".to_owned(),
        scope_id: "scope:benchmark".to_owned(),
        pointer_kind: "semantic-index-root".to_owned(),
        pointer_name: pointer_name.into(),
    }
}

fn root(label: &str) -> String {
    hash_blob(label.as_bytes()).to_string()
}

fn artifact_pointer_cas(c: &mut Criterion) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("benchmark runtime");

    let sequential_path = temp_db("sequential");
    let sequential_store = runtime
        .block_on(TursoArtifactPointerStore::open(&sequential_path))
        .expect("open sequential CAS store");
    let sequential_key = key("sequential");
    let mut expected_root = None;
    let mut expected_revision = 0_u64;

    let mut group = c.benchmark_group("client_db_artifact_pointer_cas");
    group.sample_size(20);
    group.throughput(Throughput::Elements(1));
    group.bench_function("durable_success", |b| {
        b.iter(|| {
            let new_root = root(&format!("revision-{}", expected_revision + 1));
            let receipt = runtime
                .block_on(
                    sequential_store.compare_and_set(&ClientDbArtifactPointerCasRequest {
                        key: sequential_key.clone(),
                        expected_root_hash: expected_root.clone(),
                        expected_revision,
                        new_root_hash: new_root.clone(),
                        updated_at_ms: expected_revision as i64 + 1,
                    }),
                )
                .expect("successful sequential CAS");
            assert_eq!(receipt.outcome, ClientDbArtifactPointerCasOutcome::Applied);
            expected_revision += 1;
            expected_root = Some(new_root);
            black_box(receipt);
        });
    });

    let storm_path = temp_db("storm");
    let stores: Vec<_> = runtime.block_on(async {
        let mut stores = Vec::with_capacity(16);
        for _ in 0..16 {
            stores.push(Arc::new(
                TursoArtifactPointerStore::open(&storm_path)
                    .await
                    .expect("open storm contender"),
            ));
        }
        stores
    });
    let storm_sequence = AtomicU64::new(0);
    group.throughput(Throughput::Elements(16));
    group.bench_function("sixteen_way_revision_zero_storm", |b| {
        b.iter(|| {
            let sequence = storm_sequence.fetch_add(1, Ordering::Relaxed);
            let storm_key = key(format!("storm-{sequence}"));
            let outcomes = runtime.block_on(async {
                let mut tasks = tokio::task::JoinSet::new();
                for (candidate, store) in stores.iter().cloned().enumerate() {
                    let contender_key = storm_key.clone();
                    tasks.spawn(async move {
                        store
                            .compare_and_set(&ClientDbArtifactPointerCasRequest {
                                key: contender_key,
                                expected_root_hash: None,
                                expected_revision: 0,
                                new_root_hash: root(&format!(
                                    "storm-{sequence}-candidate-{candidate}"
                                )),
                                updated_at_ms: candidate as i64,
                            })
                            .await
                            .expect("storm contender returns typed receipt")
                            .outcome
                    });
                }
                let mut outcomes = Vec::with_capacity(16);
                while let Some(result) = tasks.join_next().await {
                    outcomes.push(result.expect("storm contender task"));
                }
                outcomes
            });
            assert_eq!(
                outcomes
                    .iter()
                    .filter(|outcome| **outcome == ClientDbArtifactPointerCasOutcome::Applied)
                    .count(),
                1
            );
            assert_eq!(
                outcomes
                    .iter()
                    .filter(|outcome| **outcome == ClientDbArtifactPointerCasOutcome::Conflict)
                    .count(),
                15
            );
            black_box(outcomes);
        });
    });
    group.finish();
}

criterion_group!(benches, artifact_pointer_cas);
criterion_main!(benches);
