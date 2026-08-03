use std::time::Duration;

use super::RuntimeServerWorkspaceStore;

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

    let store = RuntimeServerWorkspaceStore::for_runtime_base(temporary.path());
    let cold_started = tokio::time::Instant::now();
    store
        .prepare()
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
        store
            .prepare()
            .await
            .expect("prepare warm Runtime Server workspace store");
        warm_latencies.push(started.elapsed());
    }
    warm_latencies.sort_unstable();
    let p99 = warm_latencies[126];
    eprintln!(
        "[runtime-workspace-store-performance] admittedWorkspaces=512 coldNanos={} warmSamples=128 warmP99Nanos={} coldBudgetNanos=5000000 warmP99BudgetNanos=1000000",
        cold_elapsed.as_nanos(),
        p99.as_nanos()
    );
    assert!(
        p99 < Duration::from_millis(1),
        "warm workspace store admission exceeded 1ms at p99: {p99:?}"
    );
}

#[tokio::test]
async fn workspace_store_authority_is_independent_of_transport_endpoint() {
    let temporary = tempfile::tempdir().expect("create Runtime Server layout fixture");
    let runtime_base = temporary
        .path()
        .join("state")
        .join("runtime")
        .join("server");
    let transport_base = temporary.path().join("transport");

    let store = super::prepare_runtime_server_workspace_store(&runtime_base)
        .await
        .expect("prepare canonical workspace store");

    assert_eq!(store.root(), runtime_base.join("workspaces"));
    assert!(
        !store.root().starts_with(&transport_base),
        "workspace persistence must not be derived from the transport endpoint"
    );
    assert!(
        tokio::fs::try_exists(store.root())
            .await
            .expect("inspect canonical workspace store")
    );
}
