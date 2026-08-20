use std::process::Command;

#[test]
fn restart_fails_closed_when_the_replacement_never_publishes_an_endpoint() {
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

    let restart = Command::new(asp)
        .env("ASP_STATE_HOME", state_home.path())
        .args(["server", "restart"])
        .output()
        .expect("restart isolated Runtime Server");
    assert!(
        !restart.status.success(),
        "restart reported success before endpoint publication: stdout={} stderr={}",
        String::from_utf8_lossy(&restart.stdout),
        String::from_utf8_lossy(&restart.stderr)
    );
    let stderr = String::from_utf8_lossy(&restart.stderr);
    assert!(
        stderr.contains("reasonKind=runtime-server-endpoint-publication-timeout"),
        "restart failure did not preserve the typed readiness reason: {stderr}"
    );

    let stop = Command::new(asp)
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
}
