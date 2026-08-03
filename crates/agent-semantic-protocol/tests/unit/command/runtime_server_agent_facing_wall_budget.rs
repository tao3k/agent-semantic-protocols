use super::runtime_server::{
    RUNTIME_SERVER_SUPERVISOR_BOUNDARY, agent_facing_runtime_wait_remaining,
    block_on_runtime_server_supervisor_client,
};

#[test]
fn execution_slice_keeps_supervisor_and_reply_reserves() {
    let remaining = agent_facing_runtime_wait_remaining(
        std::time::Duration::from_millis(799),
        "search",
        "resident-workspace-generation-open",
    )
    .expect("799 ms is inside the awaited runtime execution slice");

    assert_eq!(remaining, std::time::Duration::from_millis(1));
}

#[test]
fn execution_boundary_returns_schema_owned_failure() {
    let failure = agent_facing_runtime_wait_remaining(
        std::time::Duration::from_millis(800),
        "agent-session",
        "runtime-server-reconcile",
    )
    .expect_err("800 ms is the strict awaited runtime boundary");

    assert!(failure.contains("agent.semantic-protocols.agent-facing-search-wall-failure"));
    assert!(failure.contains("agent-facing-search-wall-budget-exceeded"));
    assert!(failure.contains("\"executionBudgetMicros\":800000"));
}

#[test]
fn supervisor_boundary_is_nine_hundred_milliseconds() {
    assert_eq!(
        RUNTIME_SERVER_SUPERVISOR_BOUNDARY,
        std::time::Duration::from_millis(900)
    );
}

#[test]
fn supervisor_boundary_returns_schema_owned_failure() {
    let failure = block_on_runtime_server_supervisor_client(
        "agent-session",
        "runtime-server-reconcile",
        async {
            tokio::time::sleep(std::time::Duration::from_millis(950)).await;
            Ok(())
        },
    )
    .expect_err("supervisor work must be bounded at 900 ms");

    assert!(failure.contains("agent.semantic-protocols.runtime-server-supervisor-wall-failure"));
    assert!(failure.contains("runtime-server-supervisor-boundary-exceeded"));
    assert!(failure.contains("\"boundaryMicros\":900000"));
}
