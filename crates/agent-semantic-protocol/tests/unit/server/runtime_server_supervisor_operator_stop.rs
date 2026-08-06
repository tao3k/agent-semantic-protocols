use super::{
    clear_runtime_server_operator_stop, mark_runtime_server_operator_stopped,
    require_runtime_server_not_operator_stopped,
};

fn temporary_state_home() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "asp-runtime-server-operator-stop-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("unnamed")
    ))
}

#[tokio::test]
async fn automatic_reconciliation_cannot_override_an_explicit_operator_stop() {
    let state_home = temporary_state_home();
    let _ = tokio::fs::remove_dir_all(&state_home).await;

    mark_runtime_server_operator_stopped(&state_home)
        .await
        .expect("publish operator-stop marker");
    let error = require_runtime_server_not_operator_stopped(&state_home)
        .await
        .expect_err("automatic reconciliation must remain stopped");
    assert!(error.contains("operator-stop-is-authoritative"));

    clear_runtime_server_operator_stop(&state_home)
        .await
        .expect("explicit reconciliation clears marker");
    require_runtime_server_not_operator_stopped(&state_home)
        .await
        .expect("explicit reconciliation re-enables lifecycle");

    tokio::fs::remove_dir_all(state_home)
        .await
        .expect("remove temporary state home");
}
