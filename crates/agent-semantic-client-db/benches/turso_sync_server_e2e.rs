use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use agent_semantic_client_db::turso_sync_storage::{
    DEFAULT_TURSO_SYNC_OPERATION_TIMEOUT, TursoSyncOperationOutcome, TursoSyncProfileConfig,
    TursoSyncProfileMode, TursoSyncStorage,
};
use serde::Serialize;

const ITERATIONS: usize = 32;

struct SyncServerGuard {
    child: Child,
}

impl Drop for SyncServerGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LatencyMicros {
    sample_count: usize,
    p50: u64,
    p95: u64,
    p99: u64,
    max: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncServerBenchmarkReceipt {
    schema_id: &'static str,
    turso_version: &'static str,
    iterations: usize,
    pushed_rows: usize,
    pulled_rows: i64,
    push_latency_micros: LatencyMicros,
    pull_latency_micros: LatencyMicros,
    checkpoint_latency_micros: u64,
    network_received_bytes: u64,
    network_sent_bytes: u64,
    main_wal_size: u64,
}

fn latency(mut samples: Vec<u64>) -> LatencyMicros {
    assert!(!samples.is_empty());
    samples.sort_unstable();
    let nearest_rank = |percent: usize| {
        let rank = (samples.len() * percent).div_ceil(100).max(1);
        samples[rank - 1]
    };
    LatencyMicros {
        sample_count: samples.len(),
        p50: nearest_rank(50),
        p95: nearest_rank(95),
        p99: nearest_rank(99),
        max: *samples.last().expect("non-empty samples"),
    }
}

fn start_sync_server(database_path: &Path) -> (SyncServerGuard, String) {
    let binary = std::env::var_os("TURSO_SYNC_SERVER_BIN")
        .expect("TURSO_SYNC_SERVER_BIN must point to the pinned tursodb 0.7 binary");
    let listener = TcpListener::bind("127.0.0.1:0").expect("reserve sync server port");
    let address = listener.local_addr().expect("read reserved address");
    drop(listener);

    let child = Command::new(binary)
        .arg(database_path)
        .arg("--sync-server")
        .arg(address.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start pinned Turso sync server");
    let mut guard = SyncServerGuard { child };
    for _ in 0..250 {
        if TcpStream::connect(address).is_ok() {
            return (guard, format!("http://{address}"));
        }
        if let Some(status) = guard.child.try_wait().expect("poll sync server") {
            panic!("Turso sync server exited before readiness: {status}");
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!("Turso sync server did not listen at {address} within five seconds");
}

fn config(path: &Path, remote_url: &str) -> TursoSyncProfileConfig {
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

#[tokio::main]
async fn main() {
    let root = std::env::temp_dir().join(format!(
        "asp-turso-sync-e2e-bench-{}-{}",
        std::process::id(),
        Instant::now().elapsed().as_nanos()
    ));
    std::fs::create_dir_all(&root).expect("create benchmark directory");
    let (_server, remote_url) = start_sync_server(&root.join("server.db"));
    let client_a = TursoSyncStorage::open(config(&root.join("client-a.db"), &remote_url))
        .await
        .expect("open sync client A");
    let client_b = TursoSyncStorage::open(config(&root.join("client-b.db"), &remote_url))
        .await
        .expect("open sync client B");
    let connection_a = client_a.connect().await.expect("connect client A");
    let connection_b = client_b.connect().await.expect("connect client B");
    connection_a
        .execute(
            "CREATE TABLE IF NOT EXISTS sync_bench (id INTEGER PRIMARY KEY, body TEXT NOT NULL)",
            (),
        )
        .await
        .expect("create benchmark table");

    let mut push_samples = Vec::with_capacity(ITERATIONS);
    let mut pull_samples = Vec::with_capacity(ITERATIONS);
    for index in 0..ITERATIONS {
        connection_a
            .execute(
                "INSERT INTO sync_bench(id, body) VALUES (?1, ?2)",
                (index as i64, format!("sync-row-{index}")),
            )
            .await
            .expect("insert local benchmark row");
        let started = Instant::now();
        let push = client_a.push().await;
        push_samples.push(started.elapsed().as_micros() as u64);
        assert_eq!(push.outcome, TursoSyncOperationOutcome::Applied, "{push:?}");

        let started = Instant::now();
        let pull = client_b.pull().await;
        pull_samples.push(started.elapsed().as_micros() as u64);
        assert_eq!(pull.outcome, TursoSyncOperationOutcome::Applied, "{pull:?}");
        assert_eq!(pull.pulled_changes, Some(true), "{pull:?}");
    }

    let mut rows = connection_b
        .query("SELECT COUNT(*) FROM sync_bench", ())
        .await
        .expect("count pulled rows");
    let row = rows
        .next()
        .await
        .expect("advance count row")
        .expect("count row exists");
    let pulled_rows: i64 = row.get(0).expect("read pulled row count");
    assert_eq!(pulled_rows, ITERATIONS as i64);
    drop(row);
    drop(rows);
    drop(connection_b);
    drop(connection_a);

    let started = Instant::now();
    let checkpoint = client_b.checkpoint().await;
    let checkpoint_latency_micros = started.elapsed().as_micros() as u64;
    assert_eq!(
        checkpoint.outcome,
        TursoSyncOperationOutcome::Applied,
        "{checkpoint:?}"
    );
    let stats = client_b.stats().await;
    assert_eq!(
        stats.outcome,
        TursoSyncOperationOutcome::Observed,
        "{stats:?}"
    );
    let stats = stats.stats.expect("typed sync stats");

    let receipt = SyncServerBenchmarkReceipt {
        schema_id: "agent.semantic-protocols.client-db.turso-sync-server-benchmark-receipt.v1",
        turso_version: "0.7.0",
        iterations: ITERATIONS,
        pushed_rows: ITERATIONS,
        pulled_rows,
        push_latency_micros: latency(push_samples),
        pull_latency_micros: latency(pull_samples),
        checkpoint_latency_micros,
        network_received_bytes: stats.network_received_bytes,
        network_sent_bytes: stats.network_sent_bytes,
        main_wal_size: stats.main_wal_size,
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&receipt).expect("serialize benchmark receipt")
    );
    drop(client_b);
    drop(client_a);
    drop(_server);
    std::fs::remove_dir_all(&root).expect("remove benchmark directory");
}
