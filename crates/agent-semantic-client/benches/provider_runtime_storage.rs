use std::sync::atomic::{AtomicU64, Ordering};

use agent_semantic_client::provider_runtime_storage::{
    ProviderExecutionStorageEvent, ProviderRuntimeStorageBinding,
};
use criterion::{Criterion, Throughput, criterion_group, criterion_main};

struct BenchProject(std::path::PathBuf);

impl BenchProject {
    fn new() -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "asp-provider-runtime-storage-bench-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir(&path).expect("create benchmark project");
        let status = std::process::Command::new("git")
            .arg("init")
            .arg("--quiet")
            .arg(&path)
            .status()
            .expect("run git init");
        assert!(status.success());
        Self(path)
    }
}

impl Drop for BenchProject {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn provider_runtime_storage(criterion: &mut Criterion) {
    let project = BenchProject::new();
    let binding = ProviderRuntimeStorageBinding::from_runtime_identity(
        &project.0,
        "codex",
        "benchmark-session",
        "benchmark-root",
    )
    .expect("open production storage binding");
    let sequence = AtomicU64::new(0);
    let mut group = criterion.benchmark_group("provider-runtime-storage");
    group.sample_size(20);
    group.throughput(Throughput::Elements(1));
    group.bench_function("mvcc-passive/invocation-event/rows=1", |bench| {
        bench.iter(|| {
            let sequence = sequence.fetch_add(1, Ordering::Relaxed);
            let event = ProviderExecutionStorageEvent::from_output(
                "benchmark",
                format!("search-{sequence}"),
                "rust",
                0,
                b"fixed stdout",
                b"",
                true,
                binding.context.root_session_id.clone(),
            );
            let receipt = binding
                .adapter
                .append_provider_execution(&binding.context, &event, sequence as i64)
                .expect("append production provider event");
            std::hint::black_box((
                receipt.committed_rows,
                receipt.retry_count,
                receipt.execution_digest,
            ));
        });
    });
    group.finish();
}

criterion_group!(benches, provider_runtime_storage);
criterion_main!(benches);
