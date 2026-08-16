use std::path::Path;
use std::time::Duration;

use serde_json::Value;
use tokio::process::Command;

async fn asp_with_budget(
    state_home: &Path,
    args: &[&str],
    budget: Duration,
) -> std::process::Output {
    tokio::time::timeout(
        budget,
        Command::new(env!("CARGO_BIN_EXE_asp"))
            .args(args)
            .env("ASP_STATE_HOME", state_home)
            .env("AGENT_SEMANTIC_PROTOCOLS_HOME", state_home)
            .env_remove("CODEX_THREAD_ID")
            .env_remove("CODEX_TURN_ID")
            .output(),
    )
    .await
    .unwrap_or_else(|error| {
        panic!("isolated ASP lifecycle command exceeded 500ms: args={args:?} error={error:?}")
    })
    .expect("run isolated ASP lifecycle command")
}

async fn lifecycle_asp(state_home: &Path, args: &[&str]) -> std::process::Output {
    asp_with_budget(state_home, args, Duration::from_millis(500)).await
}

async fn provision_asp(state_home: &Path, args: &[&str]) -> std::process::Output {
    asp_with_budget(state_home, args, Duration::from_secs(15)).await
}

#[tokio::test(flavor = "multi_thread")]
async fn healthcheck_preserves_operator_stop_without_spawning() {
    let state = tempfile::tempdir().expect("create isolated ASP State Home");

    let install_target = state.path().join("bin/asp");
    let install_target = install_target.to_string_lossy().into_owned();
    let install = provision_asp(
        state.path(),
        &["install", "binary", "--target", install_target.as_str()],
    )
    .await;
    assert!(
        install.status.success(),
        "install failed: {}",
        String::from_utf8_lossy(&install.stderr)
    );

    let start = lifecycle_asp(state.path(), &["server", "start"]).await;
    assert!(
        start.status.success(),
        "start failed: {}",
        String::from_utf8_lossy(&start.stderr)
    );

    let stop = lifecycle_asp(state.path(), &["server", "stop"]).await;
    assert!(
        stop.status.success(),
        "stop failed: {}",
        String::from_utf8_lossy(&stop.stderr)
    );
    let stop_receipt: Value =
        serde_json::from_slice(&stop.stdout).expect("parse typed stop receipt");
    assert_eq!(stop_receipt["operatorStopRecorded"], true);
    let endpoint = stop_receipt["endpointPath"]
        .as_str()
        .expect("typed stop receipt endpointPath");
    assert!(
        !tokio::fs::try_exists(endpoint)
            .await
            .expect("inspect endpoint")
    );

    let healthcheck = lifecycle_asp(state.path(), &["healthcheck"]).await;
    let endpoint_exists_after_healthcheck = tokio::fs::try_exists(endpoint)
        .await
        .expect("inspect endpoint after healthcheck");

    let cleanup = lifecycle_asp(state.path(), &["server", "stop"]).await;
    assert!(
        cleanup.status.success(),
        "cleanup stop failed: {}",
        String::from_utf8_lossy(&cleanup.stderr)
    );

    assert!(
        !endpoint_exists_after_healthcheck,
        "healthcheck must not spawn Runtime after an operator stop; stdout={} stderr={}",
        String::from_utf8_lossy(&healthcheck.stdout),
        String::from_utf8_lossy(&healthcheck.stderr)
    );
}
