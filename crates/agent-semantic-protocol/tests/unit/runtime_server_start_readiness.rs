use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

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
fn start_fails_closed_when_the_spawned_owner_never_publishes_an_endpoint() {
    let state_home = isolated_state_home();
    let installed_binary = state_home.join("runtime/bin/asp");
    std::fs::create_dir_all(&state_home).expect("create isolated ASP state home");

    let install = Command::new(env!("CARGO_BIN_EXE_asp"))
        .env("ASP_STATE_HOME", &state_home)
        .args([
            "install",
            "binary",
            "--target",
            installed_binary
                .to_str()
                .expect("isolated binary path must be UTF-8"),
        ])
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
        !start.status.success(),
        "owner spawn without endpoint publication must not be terminal success: {output}"
    );
    assert!(
        output.contains("reasonKind=runtime-server-endpoint-publication-timeout"),
        "missing typed endpoint-publication failure: {output}"
    );

    let _ = Command::new(&installed_binary)
        .env("ASP_STATE_HOME", &state_home)
        .args(["server", "stop"])
        .output();
    std::fs::remove_dir_all(&state_home).expect("remove isolated ASP state home");
}
