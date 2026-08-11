use std::time::{Duration, Instant};

use super::{SEARCH_DATA_PLANE_IO_BUDGET, WorkspaceDbIpcSession, WorkspaceDbSessionBinding};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stalled_search_read_is_cancelled_and_discards_every_connection_lane() {
    let fixture = tempfile::tempdir().expect("search deadline fixture");
    let socket_path = fixture.path().join("workspace-search.sock");
    let listener =
        tokio::net::UnixListener::bind(&socket_path).expect("bind stalled search server");
    let stalled_server = tokio::spawn(async move {
        let (_stream, _) = listener.accept().await.expect("accept search client");
        tokio::time::sleep(Duration::from_secs(5)).await;
    });
    let session = WorkspaceDbIpcSession::from_binding(WorkspaceDbSessionBinding {
        workspace_identity: "workspace-search-deadline".to_owned(),
        transport_contract_digest:
            crate::workspace_db_ipc::workspace_db_owner_transport_contract_digest(),
        owner_epoch: 1,
        runtime_binary_path: "/runtime/asp".to_owned(),
        runtime_binary_digest: "blake3-256:search-deadline".to_owned(),
        binding_token: "search-deadline-binding".to_owned(),
        socket_path: socket_path.display().to_string(),
        project_root: Some(fixture.path().to_path_buf()),
        generation_pointer_path: None,
    });

    let started = Instant::now();
    let error = session
        .read_runtime_owner("src/lib.rs")
        .await
        .expect_err("stalled immutable search read must time out");
    let elapsed = started.elapsed();

    assert!(error.contains("runtime-search-io-timeout"), "{error}");
    assert!(
        elapsed >= SEARCH_DATA_PLANE_IO_BUDGET,
        "elapsed={elapsed:?}"
    );
    assert!(elapsed < Duration::from_millis(750), "elapsed={elapsed:?}");
    for lane in &session.shared.lanes {
        assert!(
            lane.lock().await.is_none(),
            "timed-out lane remained reusable"
        );
    }
    stalled_server.abort();
    let _ = stalled_server.await;
}
