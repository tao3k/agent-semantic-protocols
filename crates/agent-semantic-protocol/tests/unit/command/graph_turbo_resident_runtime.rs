use agent_semantic_protocol::{GraphTurboResidentLaunchSpec, GraphTurboResidentProcess};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

const DIGEST_A: &str =
    "blake3-256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const DIGEST_B: &str =
    "blake3-256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn hello() -> Value {
    json!({
        "schemaId": "agent.semantic-protocols.graph-turbo-resident-server",
        "schemaVersion": "1",
        "messageKind": "hello",
        "requestId": 1,
        "runtimeArtifactDigest": DIGEST_A,
        "executionCommandDigest": DIGEST_B,
    })
}

fn rank_payload() -> Value {
    let packet: Value = serde_json::from_str(include_str!(
        "../../../../../sandtables/fixtures/asp/graph-turbo-owner-query.json"
    ))
    .expect("canonical Graph Turbo request fixture");
    json!({
        "graph": packet["graph"].clone(),
        "seedIds": packet["seedIds"].clone(),
        "kindBudgets": packet["kindBudgets"].clone(),
        "windowMerge": packet["windowMerge"].clone(),
        "pathBudget": packet["pathBudget"].clone(),
        "pathMaxHops": packet["pathMaxHops"].clone(),
        "cache": packet["cache"].clone(),
        "queryClauses": packet["queryClauses"].clone(),
    })
}

fn rank(request_id: u64) -> Value {
    json!({
        "schemaId": "agent.semantic-protocols.graph-turbo-resident-server",
        "schemaVersion": "1",
        "messageKind": "rank",
        "requestId": request_id,
        "workspaceIdentity": "workspace-rust-runtime",
        "generationDigest": DIGEST_A,
        "pageRoots": {"owners": DIGEST_B},
        "terms": ["runtime"],
        "profile": "owner-query",
        "budget": 8,
        "rankPayload": rank_payload(),
    })
}

fn load_generation(request_id: u64) -> Value {
    json!({
        "schemaId": "agent.semantic-protocols.graph-turbo-resident-server",
        "schemaVersion": "1",
        "messageKind": "load-generation",
        "requestId": request_id,
        "workspaceIdentity": "workspace-rust-runtime",
        "generationDigest": DIGEST_A,
        "pageRoots": {"owners": DIGEST_B},
        "generationPayload": {"graph": rank_payload()["graph"].clone()},
    })
}

#[tokio::test]
async fn rust_process_and_python_resident_share_one_v1_ipc_lifecycle() {
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
        execution_command_digest: DIGEST_B.to_owned(),
        request_timeout: Duration::from_secs(30),
    })
    .await
    .expect("spawn Python Graph Turbo resident");
    let pid = process.process_id();
    let hello_receipt = process.request(&hello()).await.expect("v1 hello");
    assert_eq!(hello_receipt["state"], "ready");
    let load = process
        .request(&load_generation(2))
        .await
        .expect("load Runtime-owned generation");
    let cold = process.request(&rank(3)).await.expect("cold v1 rank");
    let warm = process.request(&rank(4)).await.expect("warm v1 rank");
    assert_eq!(process.process_id(), pid, "warm rank must reuse the child");
    assert_eq!(load["generationLoads"], 1);
    assert_eq!(cold["generationLoads"], 0);
    assert_eq!(warm["generationLoads"], 0);
    assert_eq!(warm["processSpawns"], 0);
    assert_eq!(warm["generationDigest"], DIGEST_A);
    let shutdown = process
        .shutdown(&json!({
            "schemaId": "agent.semantic-protocols.graph-turbo-resident-server",
            "schemaVersion": "1",
            "messageKind": "shutdown",
            "requestId": 5,
        }))
        .await
        .expect("typed shutdown joins Python child");
    assert_eq!(shutdown["state"], "cancelled");
}
