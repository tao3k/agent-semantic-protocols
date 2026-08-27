#[tokio::test]
async fn operator_stop_admits_only_a_strictly_newer_activation_generation() {
    let temporary = tempfile::tempdir().expect("operator-stop state root");
    let state_home = temporary.path().join("state");

    agent_semantic_client_db::runtime_server_lifecycle::mark_operator_stopped(&state_home)
        .await
        .expect("publish operator-stop authority");
    assert!(
        agent_semantic_client_db::runtime_server_lifecycle::operator_stopped(&state_home)
            .await
            .expect("read operator-stop authority")
    );
    assert!(
        !agent_semantic_client_db::runtime_server_lifecycle::admit_activation_after_operator_stop(
            &state_home,
            0,
        )
        .await
        .expect("reject activation at stopped-through generation")
    );
    assert!(
        agent_semantic_client_db::runtime_server_lifecycle::admit_activation_after_operator_stop(
            &state_home,
            1,
        )
        .await
        .expect("admit strictly newer activation generation")
    );
    assert!(
        !agent_semantic_client_db::runtime_server_lifecycle::operator_stopped(&state_home)
            .await
            .expect("new activation clears operator-stop authority")
    );
}
