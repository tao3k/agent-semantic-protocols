use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_client_db::ClientDbEngine;
use criterion::{Criterion, criterion_group, criterion_main};

fn turso_cache_hot_path(c: &mut Criterion) {
    c.bench_function("turso_engine_inspect_client_dir", |b| {
        b.iter(|| {
            let client_dir = std::env::temp_dir().join(format!(
                "asp-turso-cache-hot-path-{}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("system time before unix epoch")
                    .as_nanos()
            ));
            let report = ClientDbEngine::inspect_client_dir(&client_dir);
            assert_eq!(report.db_path, client_dir.join("client.turso"));
            let _ = std::fs::remove_dir_all(client_dir);
        });
    });
}

criterion_group!(benches, turso_cache_hot_path);
criterion_main!(benches);
