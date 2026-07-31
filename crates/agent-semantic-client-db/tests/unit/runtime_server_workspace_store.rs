use std::time::Duration;

use super::prepare_runtime_server_workspace_store_at;

#[tokio::test]
async fn daemon_store_admission_preserves_generations_and_is_workspace_count_independent() {
    let temporary = tempfile::tempdir().expect("create Runtime Server store fixture");
    let workspaces = temporary.path().join("workspaces");
    for index in 0..512 {
        let generation = workspaces
            .join(format!("workspace-{index}"))
            .join("generations");
        tokio::fs::create_dir_all(&generation)
            .await
            .expect("create admitted generation directory");
        tokio::fs::write(generation.join("active-generation.pointer"), b"admitted")
            .await
            .expect("write admitted generation pointer");
    }

    let cold_started = tokio::time::Instant::now();
    prepare_runtime_server_workspace_store_at(temporary.path())
        .await
        .expect("prepare existing Runtime Server workspace store");
    let cold_elapsed = cold_started.elapsed();

    assert!(
        tokio::fs::try_exists(
            workspaces
                .join("workspace-511")
                .join("generations")
                .join("active-generation.pointer")
        )
        .await
        .expect("inspect preserved admitted generation")
    );
    assert!(
        cold_elapsed < Duration::from_millis(5),
        "process-cold workspace store admission exceeded 5ms: {cold_elapsed:?}"
    );

    let mut warm_latencies = Vec::with_capacity(128);
    for _ in 0..128 {
        let started = tokio::time::Instant::now();
        prepare_runtime_server_workspace_store_at(temporary.path())
            .await
            .expect("prepare warm Runtime Server workspace store");
        warm_latencies.push(started.elapsed());
    }
    warm_latencies.sort_unstable();
    let p99 = warm_latencies[126];
    assert!(
        p99 < Duration::from_millis(1),
        "warm workspace store admission exceeded 1ms at p99: {p99:?}"
    );
}
