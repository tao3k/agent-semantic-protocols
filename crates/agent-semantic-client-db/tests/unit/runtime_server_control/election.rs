use agent_semantic_client_db::runtime_server_control::acquire_runtime_server_election;
use agent_semantic_client_db::runtime_server_control::acquire_runtime_server_supervisor_transaction;
use agent_semantic_client_db::runtime_server_control::wait_for_runtime_server_election;

#[tokio::test]
async fn election_handoff_awaits_the_kernel_lock_without_timer_polling() {
    let state = tempfile::tempdir().expect("runtime state");
    let owner = acquire_runtime_server_election(state.path())
        .await
        .expect("acquire owner election");
    let state_home = state.path().to_path_buf();
    let waiter = tokio::spawn(async move { wait_for_runtime_server_election(&state_home).await });
    tokio::task::yield_now().await;
    assert!(!waiter.is_finished());

    drop(owner);
    let election = tokio::time::timeout(std::time::Duration::from_millis(100), waiter)
        .await
        .expect("kernel lock handoff should be prompt")
        .expect("join waiter")
        .expect("acquire handed-off election");
    drop(election);
}

#[tokio::test]
async fn supervisor_transaction_does_not_wait_on_daemon_owner_election() {
    let state = tempfile::tempdir().expect("runtime state");
    let owner = acquire_runtime_server_election(state.path())
        .await
        .expect("acquire daemon owner election");
    let transaction = tokio::time::timeout(
        std::time::Duration::from_millis(100),
        acquire_runtime_server_supervisor_transaction(state.path()),
    )
    .await
    .expect("supervisor transaction must not contend with daemon lifetime ownership")
    .expect("acquire supervisor transaction");
    drop(transaction);
    drop(owner);
}

#[test]
fn election_owner_contains_no_sleep_or_second_scale_deadline() {
    let source = include_str!("../../../src/runtime_server_control/endpoint.rs");
    assert!(!source.contains("tokio::time::sleep"));
    assert!(!source.contains("Duration::from_secs"));
}
