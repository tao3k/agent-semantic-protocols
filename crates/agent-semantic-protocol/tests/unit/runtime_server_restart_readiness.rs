use std::process::Command;

#[test]
fn restart_publishes_a_healthy_endpoint_from_a_clean_state_home() {
    let state_home = tempfile::tempdir().expect("isolated ASP State Home");
    let asp = env!("CARGO_BIN_EXE_asp");
    let installed_asp = state_home.path().join("runtime/bin/asp");

    let install = Command::new(asp)
        .env("ASP_STATE_HOME", state_home.path())
        .args(["install", "binary", "--target"])
        .arg(&installed_asp)
        .output()
        .expect("install isolated ASP binary");
    assert!(
        install.status.success(),
        "isolated binary install failed: stdout={} stderr={}",
        String::from_utf8_lossy(&install.stdout),
        String::from_utf8_lossy(&install.stderr)
    );

    let restart = Command::new(&installed_asp)
        .env("ASP_STATE_HOME", state_home.path())
        .args(["server", "restart"])
        .output()
        .expect("restart isolated Runtime Server");
    assert!(
        restart.status.success(),
        "restart must publish a healthy endpoint from a clean State Home: stdout={} stderr={}",
        String::from_utf8_lossy(&restart.stdout),
        String::from_utf8_lossy(&restart.stderr)
    );
    assert!(
        String::from_utf8_lossy(&restart.stdout).contains("\"state\":\"healthy\""),
        "restart did not wait for the healthy endpoint receipt: stdout={} stderr={}",
        String::from_utf8_lossy(&restart.stdout),
        String::from_utf8_lossy(&restart.stderr)
    );

    let stop = Command::new(&installed_asp)
        .env("ASP_STATE_HOME", state_home.path())
        .args(["server", "stop"])
        .output()
        .expect("stop isolated Runtime Server");
    assert!(
        stop.status.success(),
        "isolated Runtime Server cleanup failed: stdout={} stderr={}",
        String::from_utf8_lossy(&stop.stdout),
        String::from_utf8_lossy(&stop.stderr)
    );
    assert!(
        !agent_semantic_client_db::runtime_server_endpoint_path(state_home.path())
            .expect("resolve isolated endpoint")
            .exists(),
        "stop must clean the published endpoint: stdout={} stderr={}",
        String::from_utf8_lossy(&stop.stdout),
        String::from_utf8_lossy(&stop.stderr)
    );
}
