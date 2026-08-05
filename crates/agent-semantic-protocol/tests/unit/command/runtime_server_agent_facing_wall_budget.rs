use crate::server::runtime_server::{
    RUNTIME_SERVER_SUPERVISOR_BOUNDARY, RUNTIME_SERVER_SUPERVISOR_EXECUTION_BUDGET,
    agent_facing_runtime_wait_remaining, block_on_runtime_server_supervisor_client,
};

#[test]
fn execution_slice_keeps_supervisor_and_reply_reserves() {
    let remaining = agent_facing_runtime_wait_remaining(
        std::time::Duration::from_millis(799),
        "search",
        "resident-workspace-generation-open",
        std::path::Path::new("."),
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
        std::path::Path::new("."),
    )
    .expect_err("800 ms is the strict awaited runtime boundary");

    assert!(failure.contains("agent.semantic-protocols.agent-facing-search-wall-failure"));
    assert!(failure.contains("agent-facing-search-wall-budget-exceeded"));
    assert!(failure.contains("\"budgetMicros\":800000"));
}

#[test]
fn supervisor_boundary_is_nine_hundred_milliseconds() {
    assert_eq!(
        RUNTIME_SERVER_SUPERVISOR_BOUNDARY,
        std::time::Duration::from_millis(900)
    );
    assert_eq!(
        RUNTIME_SERVER_SUPERVISOR_EXECUTION_BUDGET,
        std::time::Duration::from_millis(800)
    );
    assert!(RUNTIME_SERVER_SUPERVISOR_EXECUTION_BUDGET < RUNTIME_SERVER_SUPERVISOR_BOUNDARY);
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

#[test]
fn hook_evaluation_uses_the_agent_facing_boundary_not_the_supervisor_boundary() {
    let started = tokio::time::Instant::now();
    let failure = crate::server::runtime_server::block_on_agent_facing_runtime_server_client(
        started,
        "hook",
        "runtime-server-hook-evaluation",
        std::path::Path::new("."),
        async {
            tokio::time::sleep(std::time::Duration::from_millis(850)).await;
            Ok(())
        },
    )
    .expect_err("hook evaluation must not outlive the 800 ms agent-facing slice");

    assert!(failure.contains("agent-facing-search-wall-budget-exceeded"));
    assert!(failure.contains("\"surface\":\"hook\""));
    assert!(failure.contains("\"stage\":\"runtime-server-hook-evaluation\""));
    assert!(!failure.contains("runtime-server-supervisor-boundary-exceeded"));
}
