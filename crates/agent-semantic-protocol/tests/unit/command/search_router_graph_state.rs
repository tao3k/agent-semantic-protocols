#[path = "../../../src/command/search_router_graph_state.rs"]
mod search_router_graph_state;

use search_router_graph_state::{
    AscentClosureBudget, AscentClosureError, InteractiveGraphContinuationIdentity,
    InteractiveGraphHopAccounting, InteractiveGraphSearchState, run_bounded_ascent_graph_closure,
};

fn continuation(node_id: &str) -> InteractiveGraphContinuationIdentity {
    InteractiveGraphContinuationIdentity::new("session-01", node_id, "snapshot-01", "state-01")
        .expect("test continuation should be valid")
}

#[test]
fn continuation_identity_requires_session_node_snapshot_and_state() {
    assert!(InteractiveGraphContinuationIdentity::new("", "router", "snapshot", "state").is_none());
    assert!(
        InteractiveGraphContinuationIdentity::new("session-01", "", "snapshot", "state").is_none()
    );
    assert!(
        InteractiveGraphContinuationIdentity::new("session-01", "router", "", "state").is_none()
    );
    assert!(
        InteractiveGraphContinuationIdentity::new("session-01", "router", "snapshot", "").is_none()
    );
    assert!(
        InteractiveGraphContinuationIdentity::new("session-01", "router", "snapshot", "state",)
            .is_some()
    );
}

#[test]
fn executed_graph_hops_cannot_exceed_semantic_graph_hops() {
    assert!(InteractiveGraphHopAccounting::observed(2, 3).is_none());
    assert_eq!(
        InteractiveGraphHopAccounting::observed(3, 2),
        Some(InteractiveGraphHopAccounting {
            semantic_graph_hops: 3,
            executed_graph_hops: 2,
        })
    );
}

#[test]
fn cached_hop_reduces_execution_without_changing_semantic_cost() {
    let state = InteractiveGraphSearchState::start(continuation("topology"))
        .advance_to(continuation("router"), true)
        .expect("executed hop should advance")
        .advance_to(continuation("semantic-owner"), false)
        .expect("cached hop should advance");

    assert_eq!(state.hops.semantic_graph_hops, 2);
    assert_eq!(state.hops.executed_graph_hops, 1);
    assert_eq!(state.hops.cache_saved_graph_hops(), 1);
}

#[test]
fn tool_action_does_not_count_as_a_graph_hop() {
    let state = InteractiveGraphSearchState::start(continuation("router"))
        .after_tool_action()
        .expect("tool action accounting should advance");

    assert_eq!(state.tool_actions, 1);
    assert_eq!(state.hops.semantic_graph_hops, 0);
    assert_eq!(state.hops.executed_graph_hops, 0);
}

#[test]
fn graph_turbo_invocation_preserves_graph_hop_accounting() {
    let state = InteractiveGraphSearchState::start(continuation("router"))
        .advance_to(continuation("contract"), true)
        .expect("graph hop should advance");
    let hops = state.hops;
    let state = state
        .after_graph_turbo(search_router_graph_state::GraphTurboInvocationAccounting {
            input_nodes: 10,
            input_edges: 20,
            visited_nodes: 6,
            visited_edges: 9,
            iterations: 3,
            candidate_paths: 2,
            wall_time_ms: 7,
            peak_memory_bytes: 4_096,
        })
        .expect("graph turbo accounting should advance");

    assert_eq!(state.hops, hops);
    assert_eq!(state.tool_actions, 1);
    assert_eq!(state.graph_turbo.invocations, 1);
    assert_eq!(state.graph_turbo.visited_nodes, 6);
    assert_eq!(state.graph_turbo.candidate_paths, 2);
    assert_eq!(state.graph_turbo.peak_memory_bytes, 4_096);
}

#[test]
fn graph_turbo_proposal_status_cannot_deserialize_evidence_authority() {
    use search_router_graph_state::GraphTurboProposalStatus;

    assert!(serde_json::from_str::<GraphTurboProposalStatus>("\"candidate\"").is_ok());
    assert!(serde_json::from_str::<GraphTurboProposalStatus>("\"asserted\"").is_err());
    assert!(serde_json::from_str::<GraphTurboProposalStatus>("\"proved\"").is_err());
}

#[test]
fn resume_changes_identity_without_rewriting_hop_accounting() {
    let state = InteractiveGraphSearchState::start(continuation("router"))
        .advance_to(continuation("semantic-owner"), true)
        .expect("graph hop should advance");
    let resumed = state.clone().resume_at(continuation("evidence-assembly"));

    assert_ne!(resumed.continuation, state.continuation);
    assert_eq!(resumed.hops, state.hops);
}

#[test]
fn serialization_keeps_continuation_and_hop_accounting_explicit() {
    let state = InteractiveGraphSearchState::start(continuation("router"))
        .advance_to(continuation("semantic-owner"), false)
        .expect("cached graph hop should advance");
    let value = serde_json::to_value(state).expect("graph state should serialize");

    assert_eq!(value["continuation"]["sessionId"], "session-01");
    assert_eq!(value["continuation"]["nodeId"], "semantic-owner");
    assert_eq!(value["continuation"]["snapshotDigest"], "snapshot-01");
    assert_eq!(value["continuation"]["stateDigest"], "state-01");
    assert_eq!(value["hops"]["semanticGraphHops"], 1);
    assert_eq!(value["hops"]["executedGraphHops"], 0);
    assert!(value.get("jump").is_none());
}

#[test]
fn ascent_closure_derives_only_paths_within_the_graph_hop_budget() {
    let receipt = run_bounded_ascent_graph_closure(
        vec![(0, 1), (1, 2), (2, 3)],
        AscentClosureBudget {
            max_graph_hops: 2,
            max_input_edges: 3,
            max_derived_facts: 16,
            max_rule_firing_bound: 64,
        },
    )
    .expect("bounded closure should succeed");

    assert!(receipt.reachable.contains(&(0, 2, 2)));
    assert!(!receipt.reachable.contains(&(0, 3, 3)));
    assert!(receipt.reachable.iter().all(|(_, _, hops)| *hops <= 2));
}

#[test]
fn ascent_closure_rejects_input_that_exceeds_its_declared_budget() {
    let error = run_bounded_ascent_graph_closure(
        vec![(0, 1), (1, 2)],
        AscentClosureBudget {
            max_graph_hops: 2,
            max_input_edges: 1,
            max_derived_facts: 16,
            max_rule_firing_bound: 64,
        },
    )
    .expect_err("over-budget input must fail closed");

    assert_eq!(error, AscentClosureError::InputEdgeBudgetExceeded);
}
