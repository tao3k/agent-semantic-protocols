use crate::server::runtime_server::{
    agent_facing_runtime_wait_remaining, linearize_reconcile_result_with_postcondition,
};

#[test]
fn reconcile_linearization_success_skips_postcondition() {
    let runtime = tokio::runtime::Runtime::new().expect("test runtime");
    let polled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let witness = polled.clone();
    runtime.block_on(async {
        linearize_reconcile_result_with_postcondition(Ok(()), || async move {
            witness.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        })
        .await
        .expect("success");
    });
    assert!(!polled.load(std::sync::atomic::Ordering::SeqCst));
}

#[test]
fn reconcile_linearization_preserves_original_failure_when_unhealthy() {
    let runtime = tokio::runtime::Runtime::new().expect("test runtime");
    let error = runtime.block_on(async {
        linearize_reconcile_result_with_postcondition(Err("original".to_owned()), || async {
            Err("unhealthy".to_owned())
        })
        .await
        .expect_err("original failure")
    });
    assert_eq!(error, "original");
}

#[test]
fn reconcile_linearization_accepts_healthy_postcondition_once() {
    let runtime = tokio::runtime::Runtime::new().expect("test runtime");
    let polls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let witness = polls.clone();
    runtime.block_on(async {
        linearize_reconcile_result_with_postcondition(Err("wall".to_owned()), || async move {
            witness.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        })
        .await
        .expect("healthy postcondition linearizes reconcile");
    });
    assert_eq!(polls.load(std::sync::atomic::Ordering::SeqCst), 1);
}

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

#[tokio::test]
async fn hook_evaluation_uses_the_agent_facing_boundary_not_the_supervisor_boundary() {
    let started = tokio::time::Instant::now();
    let failure = crate::server::runtime_server::await_agent_facing_runtime_server_client(
        started,
        "hook",
        "runtime-server-hook-evaluation",
        std::path::Path::new("."),
        async {
            tokio::time::sleep(std::time::Duration::from_millis(850)).await;
            Ok(())
        },
    )
    .await
    .expect_err("hook evaluation must not outlive the 800 ms agent-facing slice");

    assert!(failure.contains("agent-facing-search-wall-budget-exceeded"));
    assert!(failure.contains("\"surface\":\"hook\""));
    assert!(failure.contains("\"stage\":\"runtime-server-hook-evaluation\""));
    assert!(!failure.contains("runtime-server-supervisor-boundary-exceeded"));
}

#[test]
fn explicit_reconcile_repairs_provider_catalog_before_supervisor_start() {
    let source = include_str!("../../../src/server/runtime_server.rs");
    let reconcile = source
        .find("if operation == RuntimeServerOperation::Reconcile")
        .expect("explicit Runtime Server reconcile branch");
    let branch = &source[reconcile..];
    let provider_catalog = branch
        .find("reconcile_global_provider_catalog_for_runtime")
        .expect("provider catalog reconciliation");
    let supervisor = branch
        .find("reconcile_runtime_server_supervisor")
        .expect("platform supervisor reconciliation");

    assert!(
        provider_catalog < supervisor,
        "provider catalog must be current before starting a daemon generation"
    );
}
