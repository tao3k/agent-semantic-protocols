use agent_semantic_client_db::runtime_server_runtime::RuntimeServerRuntimeBuilder;

#[test]
fn daemon_profile_owns_spawn_and_join_lifecycle() {
    let runtime = RuntimeServerRuntimeBuilder::new_daemon()
        .enable_all()
        .build()
        .expect("daemon runtime");
    let joined = runtime.block_on(async {
        let task = tokio::spawn(async { 42_u64 });
        task.await.expect("join daemon task")
    });
    assert_eq!(joined, 42);
}

#[test]
fn client_profile_completes_bounded_control_work() {
    let runtime = RuntimeServerRuntimeBuilder::new_client()
        .enable_all()
        .build()
        .expect("client runtime");
    let value = runtime.block_on(async { 7_u64 });
    assert_eq!(value, 7);
}

#[test]
fn client_executor_is_load_once() {
    let first =
        agent_semantic_client_db::runtime_server_runtime::RuntimeServerClientExecutor::get()
            .expect("client executor should initialize");
    let second =
        agent_semantic_client_db::runtime_server_runtime::RuntimeServerClientExecutor::get()
            .expect("client executor should be reused");

    assert!(std::ptr::eq(first, second));
}

#[test]
fn client_executor_is_shared_across_concurrent_sessions() {
    let runtime_address =
        agent_semantic_client_db::runtime_server_runtime::RuntimeServerClientExecutor::get()
            .expect("client executor should initialize") as *const _ as usize;
    let sessions = (0..32)
        .map(|_| {
            std::thread::spawn(|| {
                agent_semantic_client_db::runtime_server_runtime::RuntimeServerClientExecutor::get()
                    .expect("client executor should be shared") as *const _ as usize
            })
        })
        .collect::<Vec<_>>();

    for session in sessions {
        assert_eq!(
            session.join().expect("client session should join"),
            runtime_address
        );
    }
}

#[test]
fn client_executor_warm_lookup_is_sub_millisecond() {
    use std::time::Instant;

    let expected =
        agent_semantic_client_db::runtime_server_runtime::RuntimeServerClientExecutor::get()
            .expect("client executor should initialize") as *const _ as usize;
    let mut samples = Vec::with_capacity(10_000);
    for _ in 0..10_000 {
        let started = Instant::now();
        let actual =
            agent_semantic_client_db::runtime_server_runtime::RuntimeServerClientExecutor::get()
                .expect("warm client executor lookup should succeed") as *const _
                as usize;
        samples.push(started.elapsed().as_micros() as u64);
        assert_eq!(actual, expected);
    }
    samples.sort_unstable();
    let p99_micros = samples[samples.len() * 99 / 100];
    let max_micros = *samples.last().expect("performance samples");

    eprintln!(
        "[runtime-server-client-executor-performance] sampleCount={} p99Micros={} maxMicros={}",
        samples.len(),
        p99_micros,
        max_micros
    );
    assert!(
        p99_micros < 1_000,
        "warm Runtime Server client executor lookup must remain sub-millisecond: p99Micros={p99_micros}"
    );
}
