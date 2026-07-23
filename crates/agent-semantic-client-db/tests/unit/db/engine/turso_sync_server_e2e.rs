use agent_semantic_client_db::turso_sync_storage::{
    DEFAULT_TURSO_SYNC_OPERATION_TIMEOUT, TursoSyncOperationOutcome, TursoSyncProfileConfig,
    TursoSyncProfileMode, TursoSyncStorage,
};

struct SyncServerGuard {
    child: std::process::Child,
}

impl Drop for SyncServerGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn start_sync_server(database_path: &std::path::Path) -> (SyncServerGuard, String) {
    let binary = std::env::var_os("TURSO_SYNC_SERVER_BIN")
        .expect("TURSO_SYNC_SERVER_BIN must point to the pinned tursodb 0.7 binary");
    let listener = std::net::TcpListener::bind("127.0.0.1:0")
        .expect("reserve sync server port");
    let address = listener.local_addr().expect("read reserved address");
    drop(listener);

    let child = std::process::Command::new(binary)
        .arg(database_path)
        .arg("--sync-server")
        .arg(address.to_string())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("start pinned Turso sync server");
    let mut guard = SyncServerGuard { child };

    for _ in 0..250 {
        if std::net::TcpStream::connect(address).is_ok() {
            return (guard, format!("http://{address}"));
        }
        if let Some(status) = guard.child.try_wait().expect("poll sync server") {
            panic!("Turso sync server exited before readiness: {status}");
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    panic!("Turso sync server did not listen at {address} within five seconds");
}

fn sync_server_config(path: &std::path::Path, remote_url: &str) -> TursoSyncProfileConfig {
    TursoSyncProfileConfig {
        path: path.to_path_buf(),
        mode: TursoSyncProfileMode::Remote {
            remote_url: remote_url.to_owned(),
            auth_token: "local-sync-server".to_owned(),
            bootstrap_if_empty: true,
        },
        operation_timeout: DEFAULT_TURSO_SYNC_OPERATION_TIMEOUT,
    }
}

#[tokio::test]
#[ignore = "run through scripts/test-turso-sync-server-v0.7.sh with the pinned local server"]
async fn pinned_v0_7_sync_server_push_pull_checkpoint_and_stats() {
    let root = temp_root("turso-sync-server-e2e");
    let (_server, remote_url) = start_sync_server(&root.join("server.db"));
    let client_a = TursoSyncStorage::open(sync_server_config(
        &root.join("client-a.db"),
        &remote_url,
    ))
    .await
    .expect("open sync client A");
    let client_b = TursoSyncStorage::open(sync_server_config(
        &root.join("client-b.db"),
        &remote_url,
    ))
    .await
    .expect("open sync client B");

    let connection_a = client_a.connect().await.expect("connect client A");
    connection_a
        .execute(
            "CREATE TABLE IF NOT EXISTS sync_notes (id TEXT PRIMARY KEY, body TEXT NOT NULL)",
            (),
        )
        .await
        .expect("create synced table locally");
    connection_a
        .execute(
            "INSERT INTO sync_notes(id, body) VALUES (?1, ?2)",
            ("note-1", "hello from client A"),
        )
        .await
        .expect("write client A row locally");

    let push = client_a.push().await;
    assert_eq!(push.outcome, TursoSyncOperationOutcome::Applied, "{push:?}");

    let pull = client_b.pull().await;
    assert_eq!(pull.outcome, TursoSyncOperationOutcome::Applied, "{pull:?}");
    assert_eq!(pull.pulled_changes, Some(true), "{pull:?}");

    let connection_b = client_b.connect().await.expect("connect client B");
    let mut rows = connection_b
        .query(
            "SELECT body FROM sync_notes WHERE id = ?1",
            ["note-1"],
        )
        .await
        .expect("query pulled row");
    let row = rows
        .next()
        .await
        .expect("advance pulled row")
        .expect("pulled row exists");
    let body: String = row.get(0).expect("read pulled body");
    assert_eq!(body, "hello from client A");
    drop(row);
    drop(rows);
    drop(connection_b);
    drop(connection_a);

    let checkpoint = client_b.checkpoint().await;
    assert_eq!(
        checkpoint.outcome,
        TursoSyncOperationOutcome::Applied,
        "{checkpoint:?}"
    );
    let stats = client_b.stats().await;
    assert_eq!(stats.outcome, TursoSyncOperationOutcome::Observed, "{stats:?}");
    assert!(stats.stats.is_some(), "{stats:?}");
}
