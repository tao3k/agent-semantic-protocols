use super::{
    Arc, Duration, RuntimeServer, RuntimeServerExit, WorkspaceDbRegistry, fixture_endpoint,
    record_admission_fixture_candidate,
};

#[tokio::test(flavor = "multi_thread")]
async fn runtime_generation_mutation_submission_is_non_blocking_and_single_flight() {
    let _performance = crate::test_support::performance_lock();
    let runtime_dir = tempfile::tempdir().expect("create isolated runtime server directory");
    let project_fixture = crate::test_support::TestDir::new("single-flight-admission");
    let project_root = project_fixture.path().join("project");
    tokio::fs::create_dir_all(&project_root)
        .await
        .expect("create admission project");
    record_admission_fixture_candidate(&project_root);
    let workspace_identity =
        agent_semantic_client_core::state_core::ResolvedState::resolve(&project_root)
            .expect("resolve admission workspace")
            .workspace
            .workspace_id
            .to_string();
    let (endpoint, artifact_catalog) = fixture_endpoint(&runtime_dir, 28).await;
    let source_build_count = Arc::new(tokio::sync::Mutex::new(0_u32));
    let source_build_release = Arc::new(tokio::sync::Semaphore::new(0));
    let state_home = runtime_dir.path().join("state");
    let server = RuntimeServer::bind_with_catalog(
        endpoint.clone(),
        Arc::new(WorkspaceDbRegistry::with_state_home(&state_home)),
        artifact_catalog,
    )
    .await
    .expect("bind Runtime Server")
    .with_workspace_generation_builder(Arc::new({
        let source_build_count = Arc::clone(&source_build_count);
        let source_build_release = Arc::clone(&source_build_release);
        move |_workspace_identity, _project_root, _changed_paths, _provider_target| {
            let source_build_count = Arc::clone(&source_build_count);
            let source_build_release = Arc::clone(&source_build_release);
            Box::pin(async move {
                *source_build_count.lock().await += 1;
                source_build_release
                    .acquire_owned()
                    .await
                    .map_err(|_| "fixture generation release closed".to_owned())?
                    .forget();
                Err("fixture stops before source publication".to_owned())
            })
        }
    }));
    let shutdown = server.shutdown_handle();
    let server = tokio::spawn(server.serve());
    let session = Arc::new(
        agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession::for_runtime_server(
            &endpoint,
            workspace_identity,
            project_root.clone(),
        ),
    );
    session
        .health()
        .await
        .expect("establish the Runtime data-plane connection before timing submissions");

    let parallelism = std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(1);
    let request_count = parallelism.saturating_mul(32).max(64);
    let changed_path = project_root
        .join("src/lib.rs")
        .to_string_lossy()
        .into_owned();
    let mut requests = tokio::task::JoinSet::new();
    for _ in 0..request_count {
        let session = Arc::clone(&session);
        let changed_path = changed_path.clone();
        requests.spawn(async move {
            let started = tokio::time::Instant::now();
            (
                session
                    .submit_runtime_generation_mutation(
                        "mutation-single-flight",
                        vec![changed_path],
                    )
                    .await,
                started.elapsed(),
            )
        });
    }
    let mut queued_count = 0_usize;
    let mut latencies = Vec::with_capacity(request_count);
    while let Some(result) = requests.join_next().await {
        let (receipt, latency) = result.expect("join Runtime mutation submission");
        let receipt = receipt.expect("Runtime mutation submission receipt");
        receipt
            .validate()
            .expect("valid mutation admission receipt");
        assert_eq!(receipt.mutation_id, "mutation-single-flight");
        queued_count += usize::from(matches!(
            receipt.state,
            agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationMutationSubmissionState::Queued
        ));
        latencies.push(latency);
    }
    latencies.sort_unstable();
    let p99 = latencies[(latencies.len() * 99 / 100).min(latencies.len() - 1)];
    eprintln!(
        "runtime-server-single-workspace-submission requestCount={request_count} p99Micros={}",
        p99.as_micros()
    );
    assert_eq!(queued_count, 1);
    assert!(
        p99 < Duration::from_millis(10),
        "Runtime mutation submission IPC p99 exceeded 10ms: {p99:?}"
    );
    tokio::time::timeout(Duration::from_secs(1), async {
        while *source_build_count.lock().await == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("background mutation submission must reach the Runtime Server");

    let successor_b = session
        .submit_runtime_generation_mutation("mutation-successor-b", vec![changed_path.clone()])
        .await
        .expect("queue first successor mutation");
    let successor_c = session
        .submit_runtime_generation_mutation("mutation-successor-c", vec![changed_path.clone()])
        .await
        .expect("queue second successor mutation");
    let duplicate_b = session
        .submit_runtime_generation_mutation("mutation-successor-b", vec![changed_path.clone()])
        .await
        .expect("coalesce duplicate mutation already present in the active chain");
    assert!(matches!(
        successor_b.state,
        agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationMutationSubmissionState::Queued
    ));
    assert!(matches!(
        successor_c.state,
        agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationMutationSubmissionState::Queued
    ));
    assert!(matches!(
        duplicate_b.state,
        agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationMutationSubmissionState::Coalesced
    ));

    source_build_release.add_permits(3);
    tokio::time::timeout(Duration::from_millis(100), async {
        while *source_build_count.lock().await < 3 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the resident writer lane must start all queued generation attempts within 100ms");
    shutdown.shutdown();
    assert_eq!(
        server.await.expect("join Runtime Server").expect("serve"),
        RuntimeServerExit::ShutdownRequested
    );
    assert_eq!(*source_build_count.lock().await, 3);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn multi_workspace_multi_session_admission_is_single_flight_and_bounded() {
    let _performance = crate::test_support::performance_lock();
    const WORKSPACE_COUNT: usize = 3;
    const SESSION_COUNT_PER_WORKSPACE: usize = 12;
    const CALL_COUNT_PER_SESSION: usize = 16;
    let runtime_dir = tempfile::tempdir().expect("create isolated runtime server directory");
    let project_fixture = crate::test_support::TestDir::new("multi-workspace-admission");
    let (endpoint, artifact_catalog) = fixture_endpoint(&runtime_dir, 30).await;
    let source_build_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let source_build_release = Arc::new(tokio::sync::Semaphore::new(0));
    let state_home = runtime_dir.path().join("state");
    let server = RuntimeServer::bind_with_catalog(
        endpoint.clone(),
        Arc::new(WorkspaceDbRegistry::with_state_home(&state_home)),
        artifact_catalog,
    )
    .await
    .expect("bind Runtime Server")
    .with_workspace_generation_builder(Arc::new({
        let source_build_count = Arc::clone(&source_build_count);
        let source_build_release = Arc::clone(&source_build_release);
        move |_workspace_identity, _project_root, _changed_paths, _provider_target| {
            let source_build_count = Arc::clone(&source_build_count);
            let source_build_release = Arc::clone(&source_build_release);
            Box::pin(async move {
                source_build_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                source_build_release
                    .acquire_owned()
                    .await
                    .map_err(|_| "fixture generation release closed".to_owned())?
                    .forget();
                Err("fixture stops before source publication".to_owned())
            })
        }
    }));
    let generation_admission = server
        .workspace_generation_admission()
        .expect("configured generation admission");
    let shutdown = server.shutdown_handle();
    let server = tokio::spawn(server.serve());

    let mut requests = tokio::task::JoinSet::new();
    let mut workspace_projects = Vec::with_capacity(WORKSPACE_COUNT);
    for workspace_index in 0..WORKSPACE_COUNT {
        let project_root = project_fixture
            .path()
            .join(format!("project-{workspace_index}"));
        tokio::fs::create_dir_all(&project_root)
            .await
            .expect("create admission project");
        gix::init(&project_root).expect("initialize independent admission workspace with Gix");
        record_admission_fixture_candidate(&project_root);
        let workspace_identity =
            agent_semantic_client_core::state_core::ResolvedState::resolve(&project_root)
                .expect("resolve independent admission workspace")
                .workspace
                .workspace_id
                .to_string();
        workspace_projects.push((workspace_identity.clone(), project_root.clone()));
        let mutation_id = format!("mutation-workspace-{workspace_index}");
        for _ in 0..SESSION_COUNT_PER_WORKSPACE {
            let session = Arc::new(
                agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession::for_runtime_server(
                    &endpoint,
                    workspace_identity.clone(),
                    project_root.clone(),
                ),
            );
            for _ in 0..CALL_COUNT_PER_SESSION {
                let session = Arc::clone(&session);
                let changed_path = project_root
                    .join("src/lib.rs")
                    .to_string_lossy()
                    .into_owned();
                let mutation_id = mutation_id.clone();
                requests.spawn(async move {
                    let started = tokio::time::Instant::now();
                    (
                        session
                            .submit_runtime_generation_mutation(mutation_id, vec![changed_path])
                            .await,
                        started.elapsed(),
                    )
                });
            }
        }
    }

    let request_count = WORKSPACE_COUNT * SESSION_COUNT_PER_WORKSPACE * CALL_COUNT_PER_SESSION;
    let mut queued_by_workspace = std::collections::BTreeMap::<String, usize>::new();
    let mut latencies = Vec::with_capacity(request_count);
    while let Some(result) = requests.join_next().await {
        let (receipt, latency) = result.expect("join multi-session Runtime submission");
        let receipt = receipt.expect("multi-session Runtime submission receipt");
        receipt
            .validate()
            .expect("valid mutation admission receipt");
        if matches!(
            receipt.state,
            agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationMutationSubmissionState::Queued
        ) {
            *queued_by_workspace
                .entry(receipt.workspace_identity.clone())
                .or_default() += 1;
        }
        latencies.push(latency);
    }
    latencies.sort_unstable();
    let p99 = latencies[(latencies.len() * 99 / 100).min(latencies.len() - 1)];
    eprintln!(
        "runtime-server-multi-workspace-session-admission workspaceCount={WORKSPACE_COUNT} sessionCount={} requestCount={request_count} p99Micros={}",
        WORKSPACE_COUNT * SESSION_COUNT_PER_WORKSPACE,
        p99.as_micros()
    );

    tokio::time::timeout(Duration::from_secs(1), async {
        while source_build_count.load(std::sync::atomic::Ordering::Relaxed) < WORKSPACE_COUNT {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("background workspace submissions must reach the Runtime Server");

    source_build_release.add_permits(WORKSPACE_COUNT);
    let build_start = tokio::time::Instant::now();
    let mut terminal_receipts = Vec::with_capacity(WORKSPACE_COUNT);
    for (workspace_identity, project_root) in &workspace_projects {
        terminal_receipts.push(
            tokio::time::timeout(
                Duration::from_secs(5),
                generation_admission.wait_terminal(workspace_identity, project_root),
            )
            .await
            .expect("admitted workspace generation task must reach a terminal state")
            .expect("read workspace generation terminal receipt"),
        );
    }
    eprintln!(
        "runtime-server-multi-workspace-build-start micros={} terminalReceipts={terminal_receipts:?}",
        build_start.elapsed().as_micros(),
    );
    shutdown.shutdown();
    assert_eq!(
        server.await.expect("join Runtime Server").expect("serve"),
        RuntimeServerExit::ShutdownRequested
    );
    assert_eq!(queued_by_workspace.len(), WORKSPACE_COUNT);
    assert!(
        queued_by_workspace.values().all(|queued| *queued == 1),
        "each workspace must enqueue exactly one mutation flight: {queued_by_workspace:?}"
    );
    assert!(
        p99 < Duration::from_millis(10),
        "multi-workspace Runtime submission IPC p99 exceeded 10ms: {p99:?}"
    );
    assert_eq!(
        source_build_count.load(std::sync::atomic::Ordering::Relaxed),
        WORKSPACE_COUNT
    );
}
