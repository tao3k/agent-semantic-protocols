#![allow(dead_code)]

#[path = "../../../src/command/search_router_graph_state.rs"]
mod search_router_graph_state;

use search_router_graph_state::{
    AscentClosureBudget, AscentClosureError, run_bounded_ascent_graph_closure,
};

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

    assert!(matches!(
        error,
        AscentClosureError::InputEdgeBudgetExceeded { .. }
    ));
}
