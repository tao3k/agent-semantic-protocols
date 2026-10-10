// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Runtime Server connection admission and I/O budget tests.

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn connection_supervisor_bounds_completed_but_unreaped_tasks() {
    use agent_semantic_client_db::runtime_server_connection::RuntimeServerConnectionSupervisor;

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
    use agent_semantic_client_db::runtime_server_connection::runtime_server_connection_limit;

    assert_eq!(runtime_server_connection_limit(1), 32);
    assert!(runtime_server_connection_limit(8) > runtime_server_connection_limit(2));
    assert_eq!(runtime_server_connection_limit(usize::MAX), 512);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stalled_connection_io_fails_inside_the_runtime_budget() {
    use agent_semantic_client_db::runtime_server_connection::RUNTIME_SERVER_CONNECTION_IO_BUDGET;
    use agent_semantic_client_db::runtime_server_connection::within_connection_io_budget;

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
