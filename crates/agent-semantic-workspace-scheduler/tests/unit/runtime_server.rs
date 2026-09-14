// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use crate::{
    RuntimeServerClientExecutor, RuntimeServerOwnedTask, RuntimeServerRuntimeBuilder,
    RuntimeServerTaskScope, adaptive_tokio_worker_count,
};

#[test]
fn daemon_profile_owns_spawn_and_join_lifecycle() {
    let runtime = RuntimeServerRuntimeBuilder::new_daemon()
        .enable_all()
        .build()
        .expect("daemon runtime");
    let workers =
        runtime.block_on(async { tokio::runtime::Handle::current().metrics().num_workers() });
    assert_eq!(workers, adaptive_tokio_worker_count());
    let joined = runtime.block_on(async {
        RuntimeServerOwnedTask::spawn("fixture-daemon-task", async { 42_u64 })
            .join()
            .await
            .expect("join daemon task")
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
                .is_ok()
        );
    });
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_task_scope_drains_without_leaks() {
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
                .expect("admit concurrent task"),
        );
    }
    barrier.wait().await;
    scope.begin_drain();
    assert!(scope.spawn("fixture-late-task", async {}).is_err());
    for task in tasks {
        task.join().await.expect("join owned task");
    }
    let receipt = scope.finish(0).expect("drain task scope");
    assert_eq!(receipt.started, 64);
    assert_eq!(receipt.completed, 64);
    assert_eq!(receipt.active, 0);
    assert_eq!(receipt.leaked, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dropped_owned_tasks_are_accounted_as_cancelled() {
    let scope = RuntimeServerTaskScope::new("fixture-cancel-on-drop");
    let task = scope
        .spawn("fixture-pending-task", std::future::pending::<()>())
        .expect("admit pending task");
    drop(task);
    let receipt = scope.finish(0).expect("account cancellation");
    assert_eq!(receipt.started, 1);
    assert_eq!(receipt.cancelled, 1);
    assert_eq!(receipt.active, 0);
}

#[test]
fn client_profiles_are_current_thread_schedulers() {
    for mut builder in [
        RuntimeServerRuntimeBuilder::new_client(),
        RuntimeServerRuntimeBuilder::new_hook_client(),
    ] {
        let runtime = builder.enable_all().build().expect("client runtime");
        let workers =
            runtime.block_on(async { tokio::runtime::Handle::current().metrics().num_workers() });
        assert_eq!(workers, 1);
    }
}

#[test]
fn client_control_path_requires_a_caller_runtime() {
    assert!(RuntimeServerClientExecutor::get().is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn client_control_path_uses_the_callers_runtime_under_concurrency() {
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..256 {
        tasks.spawn(async { RuntimeServerClientExecutor::get().is_ok() });
    }
    while let Some(result) = tasks.join_next().await {
        assert!(result.expect("caller task should join"));
    }
}

#[test]
fn client_executor_lookup_is_sub_millisecond() {
    let expected = RuntimeServerClientExecutor::get().is_err();
    let mut samples = Vec::with_capacity(10_000);
    for _ in 0..10_000 {
        let started = std::time::Instant::now();
        assert_eq!(RuntimeServerClientExecutor::get().is_err(), expected);
        samples.push(started.elapsed().as_micros() as u64);
    }
    samples.sort_unstable();
    assert!(samples[samples.len() * 99 / 100] < 1_000);
}
