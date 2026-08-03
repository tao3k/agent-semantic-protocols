use agent_semantic_client_db::graph_turbo_cache::{
    GraphTurboCacheEntry, GraphTurboCacheIdentity, GraphTurboCacheReadStatus, TursoGraphTurboCache,
};
use agent_semantic_protocol::{GraphTurboResidentLaunchSpec, GraphTurboResidentProcess};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const SNAPSHOT: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn create(label: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after Unix epoch")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("asp-{label}-{}-{unique}", std::process::id()));
        fs::create_dir_all(&path).expect("create test directory");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn envelope(kind: &str, request_id: &str) -> Value {
    json!({
        "schemaId": "agent.semantic-protocols.semantic-graph-turbo-resident-message",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "messageKind": kind,
        "requestId": request_id,
        "sessionId": "session:rust-runtime",
        "snapshotDigest": SNAPSHOT,
        "workspaceGenerationRootDigest": "generation:rust-runtime"
    })
}

fn rank_message(request_id: &str) -> Value {
    let mut message = envelope("rank", request_id);
    message["request"] = json!({
        "profile": "owner-query",
        "algorithm": "typed-ppr-diverse",
        "seedIds": ["q:parser"],
        "budget": 3,
        "pathBudget": 2,
        "pathMaxHops": 2,
        "queryTerms": ["parser"],
        "cache": {"enabled": true},
        "windowMerge": {"enabled": true, "maxGapLines": 8},
        "graph": {
            "nodes": [
                {"id": "q:parser", "kind": "query", "role": "term", "value": "parser"},
                {"id": "owner:cli", "kind": "owner", "role": "path", "value": "src/cli.py"}
            ],
            "edges": [
                {"source": "q:parser", "target": "owner:cli", "relation": "matches"}
            ]
        }
    });
    message
}

fn cache_identity(snapshot_digest: &str) -> GraphTurboCacheIdentity {
    GraphTurboCacheIdentity {
        snapshot_digest: snapshot_digest.to_string(),
        workspace_generation_root_digest: "generation:rust-runtime".into(),
        profile: "owner-query".into(),
        algorithm: "typed-ppr-diverse".into(),
        seed_digest: "b".repeat(64),
        parameter_digest: "c".repeat(64),
    }
}

#[tokio::test]
async fn rust_runtime_keeps_python_resident_and_persists_only_exact_candidate_receipts() {
    let root = workspace_root();
    let python = std::env::var_os("ASP_GRAPH_TURBO_PYTHON")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("packages/python/.venv/bin/python3"));
    let mut process = GraphTurboResidentProcess::spawn(GraphTurboResidentLaunchSpec {
        program: python,
        args: vec![
            OsString::from("-m"),
            OsString::from("asp_graph_turbo.resident_server"),
        ],
        cwd: root,
        env: BTreeMap::new(),
        execution_command_digest: "blake3-256:graph-turbo-resident-v1".into(),
        request_timeout: Duration::from_secs(60),
    })
    .expect("spawn Graph Turbo resident");
    let pid = process.process_id();
    let handshake = process
        .request(&envelope("handshake", "request:hello"))
        .expect("handshake");
    assert_eq!(handshake["status"], "handshake-accepted");

    let cold_started = std::time::Instant::now();
    let cold = process
        .request(&rank_message("request:cold"))
        .expect("cold rank");
    let cold_rank_micros = cold_started.elapsed().as_micros() as u64;
    let warm_started = std::time::Instant::now();
    let warm = process
        .request(&rank_message("request:warm"))
        .expect("warm rank");
    let warm_rank_micros = warm_started.elapsed().as_micros() as u64;
    assert_eq!(process.process_id(), pid);
    assert_eq!(cold["authority"], "candidate");
    assert_eq!(warm["accounting"]["graphTurboInvocations"], 2);
    assert_eq!(warm["accounting"]["semanticGraphHops"], 0);

    let directory = TestDirectory::create("graph-turbo-resident-runtime");
    let cache = TursoGraphTurboCache::open(&directory.path().join("graph-turbo.db"))
        .await
        .expect("open Turso cache");
    let identity = cache_identity(SNAPSHOT);
    let entry = GraphTurboCacheEntry::from_resident_receipt(identity.clone(), &warm)
        .expect("candidate cache entry");
    cache.put(&entry).await.expect("persist candidate");
    assert_eq!(
        cache.get(&identity).await.expect("exact cache read").status,
        GraphTurboCacheReadStatus::Hit
    );
    assert_eq!(
        cache
            .get(&cache_identity(&"d".repeat(64)))
            .await
            .expect("stale cache read")
            .status,
        GraphTurboCacheReadStatus::Miss
    );
    drop(cache);
    let restarted = TursoGraphTurboCache::open(&directory.path().join("graph-turbo.db"))
        .await
        .expect("restart Turso cache");
    assert_eq!(
        restarted
            .get(&identity)
            .await
            .expect("restart cache read")
            .status,
        GraphTurboCacheReadStatus::Hit
    );

    let shutdown = process
        .shutdown(&envelope("shutdown", "request:shutdown"))
        .expect("shutdown");
    assert_eq!(shutdown["status"], "shutdown-accepted");

    if let Some(path) = std::env::var_os("ASP_GRAPH_TURBO_RUNTIME_RECEIPT") {
        let runtime_artifact = std::env::var_os("ASP_GRAPH_TURBO_PYTHON")
            .map(PathBuf::from)
            .unwrap_or_else(|| workspace_root().join("packages/python/.venv/bin/python3"));
        let receipt = json!({
            "schemaId": "agent.semantic-protocols.semantic-graph-turbo-runtime-receipt",
            "schemaVersion": "1",
            "sessionId": "session:rust-runtime",
            "snapshotDigest": SNAPSHOT,
            "processId": pid,
            "runtimeArtifact": runtime_artifact,
            "executionCommandDigest": "blake3-256:graph-turbo-resident-v1",
            "coldRankMicros": cold_rank_micros,
            "warmRankMicros": warm_rank_micros,
            "graphTurboInvocations": 2,
            "semanticGraphHops": 0,
            "executedGraphHops": 0,
            "authority": "candidate",
            "tursoExactStatus": "hit",
            "tursoStaleSnapshotStatus": "miss",
            "tursoRestartStatus": "hit"
        });
        std::fs::write(
            PathBuf::from(path),
            serde_json::to_vec_pretty(&receipt).expect("encode runtime receipt"),
        )
        .expect("write runtime receipt");
    }
}

#[test]
fn runtime_rejects_forged_graph_turbo_authority() {
    let forged = json!({
        "status": "rank-completed",
        "authority": "proved",
        "result": {}
    });
    let error = agent_semantic_protocol::admit_candidate_rank_receipt(forged)
        .expect_err("proved authority must be rejected");
    assert!(error.contains("candidate-only authority boundary"));
}
