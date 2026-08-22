use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_client_db::runtime_server_endpoint_path;

fn isolated_state_home() -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock must be after the Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "asp-runtime-start-readiness-{}-{nonce}",
        std::process::id()
    ))
}

#[test]
fn start_publishes_a_healthy_endpoint_after_owner_spawn() {
    let state_home = isolated_state_home();
    std::fs::create_dir_all(&state_home).expect("create isolated ASP state home");
    let state_home = std::fs::canonicalize(state_home).expect("canonical ASP state home");
    let installed_binary = state_home.join("runtime/bin/asp");

    let install = Command::new(env!("CARGO_BIN_EXE_asp"))
        .env("ASP_STATE_HOME", &state_home)
        .args(["install", "binary"])
        .output()
        .expect("install isolated ASP binary");
    assert!(
        install.status.success(),
        "isolated binary install failed: stdout={} stderr={}",
        String::from_utf8_lossy(&install.stdout),
        String::from_utf8_lossy(&install.stderr)
    );

    let start = Command::new(&installed_binary)
        .env("ASP_STATE_HOME", &state_home)
        .args(["server", "start"])
        .output()
        .expect("start isolated Runtime Server");
    let output = format!(
        "{}\n{}",
        String::from_utf8_lossy(&start.stdout),
        String::from_utf8_lossy(&start.stderr)
    );
    assert!(
        start.status.success(),
        "owner spawn must publish a healthy endpoint: {output}"
    );
    assert!(
        output.contains("\"state\":\"healthy\""),
        "start must wait for the published healthy receipt: {output}"
    );

    let stop = Command::new(&installed_binary)
        .env("ASP_STATE_HOME", &state_home)
        .args(["server", "stop"])
        .output()
        .expect("stop isolated Runtime Server");
    assert!(
        stop.status.success(),
        "stop must clean the endpoint after a ready owner: stdout={} stderr={}",
        String::from_utf8_lossy(&stop.stdout),
        String::from_utf8_lossy(&stop.stderr)
    );

    let stale_endpoint =
        runtime_server_endpoint_path(&state_home).expect("resolve isolated Runtime endpoint");
    std::fs::create_dir_all(
        stale_endpoint
            .parent()
            .expect("Runtime endpoint has a parent directory"),
    )
    .expect("create stale Runtime endpoint parent");
    std::fs::write(&stale_endpoint, b"stale endpoint")
        .expect("publish deliberately stale Runtime endpoint");

    let restarted = Command::new(&installed_binary)
        .env("ASP_STATE_HOME", &state_home)
        .args(["server", "start"])
        .output()
        .expect("self-heal stale Runtime endpoint");
    let restarted_output = format!(
        "{}\n{}",
        String::from_utf8_lossy(&restarted.stdout),
        String::from_utf8_lossy(&restarted.stderr)
    );
    assert!(
        restarted.status.success() && restarted_output.contains("\"state\":\"healthy\""),
        "stale endpoint must self-heal to a healthy owner: {restarted_output}"
    );

    let final_stop = Command::new(&installed_binary)
        .env("ASP_STATE_HOME", &state_home)
        .args(["server", "stop"])
        .output()
        .expect("stop self-healed Runtime Server");
    assert!(
        final_stop.status.success() && !stale_endpoint.exists(),
        "stop must remove the self-healed endpoint: stdout={} stderr={}",
        String::from_utf8_lossy(&final_stop.stdout),
        String::from_utf8_lossy(&final_stop.stderr)
    );
    std::fs::remove_dir_all(&state_home).expect("remove isolated ASP state home");
}

#[test]
fn concurrent_starts_share_one_runtime_server_owner() {
    let state_home = isolated_state_home();
    std::fs::create_dir_all(&state_home).expect("create isolated ASP state home");
    let state_home = std::fs::canonicalize(state_home).expect("canonical ASP state home");
    let installed_binary = state_home.join("runtime/bin/asp");

    let install = Command::new(env!("CARGO_BIN_EXE_asp"))
        .env("ASP_STATE_HOME", &state_home)
        .args(["install", "binary"])
        .output()
        .expect("install isolated ASP binary");
    assert!(install.status.success(), "isolated binary install failed");

    let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
    let starts = (0..2)
        .map(|_| {
            let barrier = std::sync::Arc::clone(&barrier);
            let binary = installed_binary.clone();
            let state = state_home.clone();
            std::thread::spawn(move || {
                barrier.wait();
                Command::new(binary)
                    .env("ASP_STATE_HOME", state)
                    .args(["server", "start"])
                    .output()
                    .expect("concurrent Runtime Server start")
            })
        })
        .collect::<Vec<_>>();
    barrier.wait();
    let outputs = starts
        .into_iter()
        .map(|start| start.join().expect("concurrent start worker"))
        .collect::<Vec<_>>();
    assert!(
        outputs.iter().all(|output| output.status.success()),
        "all concurrent starts must converge on the one ready owner: {outputs:?}"
    );

    assert!(
        outputs.iter().all(|output| String::from_utf8_lossy(&output.stdout)
            .contains("\"state\":\"healthy\"")),
        "each concurrent caller must observe the same ready Runtime owner: {outputs:?}"
    );
    let owner_spawn = state_home.join("runtime/server/owner-spawn.v1.json");
    let owner: serde_json::Value = serde_json::from_slice(
        &std::fs::read(&owner_spawn).expect("read the single durable owner receipt"),
    )
    .expect("decode the single durable owner receipt");
    assert_eq!(
        owner["schemaId"],
        "agent.semantic-protocols.runtime-server-owner-spawn.v1"
    );
    assert!(
        owner["processId"].as_u64().is_some(),
        "the converged Runtime must retain one concrete owner process"
    );

    let stop = Command::new(&installed_binary)
        .env("ASP_STATE_HOME", &state_home)
        .args(["server", "stop"])
        .output()
        .expect("stop concurrent Runtime Server owner");
    assert!(stop.status.success(), "concurrent owner cleanup failed");
    std::fs::remove_dir_all(&state_home).expect("remove isolated ASP state home");
}
