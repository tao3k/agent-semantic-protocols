// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use agent_semantic_client_db::runtime_server_runtime::RuntimeServerOwnedTask;
use agent_semantic_client_db::runtime_server_runtime::RuntimeServerRuntimeBuilder;

#[test]
fn daemon_profile_owns_spawn_and_join_lifecycle() {
    let runtime = RuntimeServerRuntimeBuilder::new_daemon()
        .enable_all()
        .build()
        .expect("daemon runtime");
    let workers =
        runtime.block_on(async { tokio::runtime::Handle::current().metrics().num_workers() });
    assert!(
        workers == agent_semantic_client_db::runtime_server_runtime::adaptive_tokio_worker_count(),
        "daemon worker pool must follow the host's effective CPU allocation: {workers}"
    );
    let joined = runtime.block_on(async {
        let task = RuntimeServerOwnedTask::spawn("fixture-daemon-task", async { 42_u64 });
        task.join().await.expect("join daemon task")
    });
    assert_eq!(joined, 42);
}

#[test]
fn dropping_owned_daemon_task_aborts_instead_of_detaching() {
    let runtime = RuntimeServerRuntimeBuilder::new_daemon()
        .enable_all()
        .build()
        .expect("daemon runtime");
    runtime.block_on(async {
        let (dropped, observed_drop) = tokio::sync::oneshot::channel::<()>();
        let task = RuntimeServerOwnedTask::spawn("fixture-abort-on-drop", async move {
            let _guard = dropped;
            std::future::pending::<()>().await;
        });
        drop(task);
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(100), observed_drop)
                .await
                .is_ok(),
            "dropping an owned Runtime task must cancel its future"
        );
    });
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_task_scope_drains_without_leaks() {
    use agent_semantic_client_db::runtime_server_runtime::RuntimeServerTaskScope;

    let scope = RuntimeServerTaskScope::new("fixture-concurrent-lifecycle");
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(65));
    let mut tasks = Vec::with_capacity(64);
    for _ in 0..64 {
        let barrier = std::sync::Arc::clone(&barrier);
        tasks.push(
            scope
                .spawn("fixture-concurrent-task", async move {
                    barrier.wait().await;
                    tokio::task::yield_now().await;
                })
                .expect("accepting scope should admit concurrent task"),
        );
    }
    barrier.wait().await;
    scope.begin_drain();
    let late_admission = scope.spawn("fixture-late-task", async {});
    assert!(
        matches!(late_admission, Err(ref error) if error.contains("is draining")),
        "draining scope must reject new task admission"
    );
    for task in tasks {
        task.join().await.expect("owned task should join");
    }

    let receipt = scope.finish(0).expect("all admitted tasks should drain");
    assert_eq!(receipt.state, "terminated");
    assert_eq!(receipt.started, 64);
    assert_eq!(receipt.completed, 64);
    assert_eq!(receipt.cancelled, 0);
    assert_eq!(receipt.failed, 0);
    assert_eq!(receipt.active, 0);
    assert_eq!(receipt.leaked, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dropped_owned_tasks_are_accounted_as_cancelled() {
    use agent_semantic_client_db::runtime_server_runtime::RuntimeServerTaskScope;

    let scope = RuntimeServerTaskScope::new("fixture-cancel-on-drop");
    let task = scope
        .spawn("fixture-pending-task", std::future::pending::<()>())
        .expect("accepting scope should admit pending task");
    drop(task);
    scope.begin_drain();

    let receipt = scope
        .finish(0)
        .expect("drop must synchronously account cancellation");
    assert_eq!(receipt.started, 1);
    assert_eq!(receipt.cancelled, 1);
    assert_eq!(receipt.active, 0);
    assert_eq!(receipt.leaked, 0);
}

#[test]
fn client_profile_completes_bounded_control_work() {
    let runtime = RuntimeServerRuntimeBuilder::new_daemon()
        .enable_all()
        .build()
        .expect("client runtime");
    let workers =
        runtime.block_on(async { tokio::runtime::Handle::current().metrics().num_workers() });
    assert_eq!(
        workers,
        agent_semantic_client_db::runtime_server_runtime::adaptive_tokio_worker_count()
    );
    let value = runtime.block_on(async { 7_u64 });
    assert_eq!(value, 7);
}

#[test]
fn interactive_client_profile_is_a_single_server_first_scheduler() {
    let runtime = RuntimeServerRuntimeBuilder::new_client()
        .enable_all()
        .build()
        .expect("interactive client runtime");
    let workers =
        runtime.block_on(async { tokio::runtime::Handle::current().metrics().num_workers() });
    assert_eq!(workers, 1);
}

#[test]
fn hook_client_profile_is_a_single_current_thread_tokio_scheduler() {
    let runtime = RuntimeServerRuntimeBuilder::new_hook_client()
        .enable_all()
        .build()
        .expect("Hook client runtime");
    let workers =
        runtime.block_on(async { tokio::runtime::Handle::current().metrics().num_workers() });
    assert_eq!(workers, 1);
}

#[test]
fn client_control_path_requires_a_caller_runtime() {
    assert!(
        agent_semantic_client_db::runtime_server_runtime::RuntimeServerClientExecutor::get()
            .is_err(),
        "a client must not create a process-global Tokio runtime when none is active"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn client_control_path_uses_the_callers_runtime_under_concurrency() {
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..256 {
        tasks.spawn(async {
            agent_semantic_client_db::runtime_server_runtime::RuntimeServerClientExecutor::get()
                .is_ok()
        });
    }
    while let Some(result) = tasks.join_next().await {
        assert!(
            result.expect("caller task should join"),
            "client control work must borrow the active caller runtime"
        );
    }
}

#[test]
fn client_control_path_does_not_create_a_runtime_during_repeated_calls() {
    let _performance = crate::test_support::performance_lock();
    use std::time::Instant;

    let expected =
        agent_semantic_client_db::runtime_server_runtime::RuntimeServerClientExecutor::get()
            .is_err();
    let mut samples = Vec::with_capacity(10_000);
    for _ in 0..10_000 {
        let started = Instant::now();
        let actual =
            agent_semantic_client_db::runtime_server_runtime::RuntimeServerClientExecutor::get()
                .is_err();
        samples.push(started.elapsed().as_micros() as u64);
        assert_eq!(actual, expected);
    }
    samples.sort_unstable();
    let p99_micros = samples[samples.len() * 99 / 100];
    let max_micros = *samples.last().expect("performance samples");

    eprintln!(
        "[runtime-server-client-control-no-runtime-performance] sampleCount={} p99Micros={} maxMicros={}",
        samples.len(),
        p99_micros,
        max_micros
    );
    assert!(
        p99_micros < 1_000,
        "warm Runtime Server client executor lookup must remain sub-millisecond: p99Micros={p99_micros}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn connection_supervisor_bounds_completed_but_unreaped_tasks() {
    use agent_semantic_client_db::runtime_server_runtime::RuntimeServerConnectionSupervisor;

    let supervisor = RuntimeServerConnectionSupervisor::new("fixture-connections", 32);
    let mut connections = tokio::task::JoinSet::new();
    for _ in 0..32 {
        let lease = supervisor
            .try_admit()
            .expect("connection below the adaptive bound must be admitted");
        connections.spawn(async move { lease });
    }
    tokio::task::yield_now().await;

    let saturated = supervisor.snapshot();
    assert_eq!(saturated.active, 32);
    assert_eq!(saturated.high_watermark, 32);
    assert!(!supervisor.has_capacity());
    assert!(
        supervisor.try_admit().is_none(),
        "a completed task must keep its lease until the JoinSet result is reaped"
    );
    assert_eq!(supervisor.snapshot().rejected, 1);

    while let Some(lease) = connections
        .join_next()
        .await
        .transpose()
        .expect("connection task must join")
    {
        drop(lease);
    }
    let drained = supervisor.snapshot();
    assert_eq!(drained.active, 0);
    assert!(supervisor.has_capacity());
}

#[test]
fn connection_limit_adapts_to_runtime_worker_count_and_remains_bounded() {
    use agent_semantic_client_db::runtime_server_runtime::runtime_server_connection_limit;

    assert_eq!(runtime_server_connection_limit(1), 32);
    assert!(runtime_server_connection_limit(8) > runtime_server_connection_limit(2));
    assert_eq!(runtime_server_connection_limit(usize::MAX), 512);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stalled_connection_io_fails_inside_the_runtime_budget() {
    use agent_semantic_client_db::runtime_server_runtime::RUNTIME_SERVER_CONNECTION_IO_BUDGET;
    use agent_semantic_client_db::runtime_server_runtime::within_connection_io_budget;

    let started = tokio::time::Instant::now();
    let error = within_connection_io_budget(
        "fixture stalled frame",
        std::future::pending::<Result<(), String>>(),
    )
    .await
    .expect_err("stalled connection I/O must fail closed");

    assert!(error.contains("connection I/O budget"));
    assert!(started.elapsed() >= RUNTIME_SERVER_CONNECTION_IO_BUDGET);
    assert!(
        started.elapsed() < std::time::Duration::from_millis(100),
        "a stalled connection must not retain a daemon task beyond 100ms"
    );
}
