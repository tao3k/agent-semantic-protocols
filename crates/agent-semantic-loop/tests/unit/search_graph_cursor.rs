// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::InteractiveSearchGraphState;
use super::SearchGraphCursor;
use super::SearchGraphCursorArtifact;
use super::SearchGraphCursorBootstrapRequest;
use super::SearchGraphCursorError;
use super::SearchGraphCursorJumpRequest;
use super::UncheckedInteractiveSearchGraphState;
use super::UncheckedSearchGraphCursorArtifact;

fn fixture_state(field: &str) -> InteractiveSearchGraphState {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../schemas/fixtures/interactive-search-graph-step/valid-semantic-owner-step.v1.json"
    ))
    .expect("parse graph-step fixture");
    let unchecked: UncheckedInteractiveSearchGraphState =
        serde_json::from_value(fixture[field].clone()).expect("parse graph state");
    InteractiveSearchGraphState::validate(unchecked).expect("validate graph state")
}

fn cursor_for_state(state: InteractiveSearchGraphState) -> SearchGraphCursor {
    SearchGraphCursor::bootstrap(SearchGraphCursorBootstrapRequest {
        state,
        materialized_node_ids: vec![
            "goal:model-owner".to_string(),
            "symbol:ModelConfig".to_string(),
            "symbol:AgentRegistry".to_string(),
        ],
    })
    .expect("bootstrap cursor")
}

#[test]
fn graph_step_fixture_state_validates_against_route_budget() {
    let state = fixture_state("beforeState");

    assert_eq!(state.current_node(), "goal:model-owner");
    assert_eq!(state.graph_generation(), 7);
    assert_eq!(state.remaining_budget(), 4);
    assert_eq!(state.route_measure().semantic_graph_hops(), 2);
    assert_eq!(state.route_budget().max_graph_hops(), 4);
}

#[test]
fn jump_to_materialized_frontier_preserves_execution_and_token_measures() {
    let cursor = cursor_for_state(fixture_state("beforeState"));
    let before_measure = cursor.state().route_measure().clone();
    let before_budget = cursor.state().remaining_budget();
    let jumped = cursor
        .jump(SearchGraphCursorJumpRequest {
            target_node_id: "symbol:ModelConfig",
            next_state_digest: "state-7-jump-model-config",
        })
        .expect("jump to materialized frontier");

    assert_eq!(jumped.state().current_node(), "symbol:ModelConfig");
    assert_eq!(jumped.state().route_measure(), &before_measure);
    assert_eq!(jumped.state().remaining_budget(), before_budget);
    assert_eq!(jumped.state().graph_generation(), 7);
    assert_eq!(
        jumped.state().workspace_generation(),
        "workspace-generation-42"
    );
    assert!(
        jumped
            .state()
            .visited()
            .iter()
            .any(|node| node == "goal:model-owner")
    );
}

#[test]
fn jump_to_visited_node_does_not_reexecute_the_playbook() {
    let cursor = cursor_for_state(fixture_state("afterState"));
    let before_measure = cursor.state().route_measure().clone();
    let jumped = cursor
        .jump(SearchGraphCursorJumpRequest {
            target_node_id: "symbol:ModelConfig",
            next_state_digest: "state-8-history-jump",
        })
        .expect("jump to visited node");

    assert_eq!(jumped.state().current_node(), "symbol:ModelConfig");
    assert_eq!(jumped.state().route_measure(), &before_measure);
    assert_eq!(jumped.state().remaining_budget(), 3);
}

#[test]
fn cursor_rejects_unmaterialized_or_replayed_node() {
    let cursor = cursor_for_state(fixture_state("beforeState"));
    assert_eq!(
        cursor
            .jump(SearchGraphCursorJumpRequest {
                target_node_id: "symbol:Unknown",
                next_state_digest: "state-unknown",
            })
            .expect_err("unmaterialized node must fail"),
        SearchGraphCursorError::NodeNotMaterialized("symbol:Unknown".to_string())
    );
    assert_eq!(
        cursor
            .jump(SearchGraphCursorJumpRequest {
                target_node_id: "goal:model-owner",
                next_state_digest: "state-replay",
            })
            .expect_err("current node replay must fail"),
        SearchGraphCursorError::CurrentNodeReplay
    );
}

#[test]
fn cursor_bootstrap_rejects_state_node_outside_materialized_graph() {
    let error = SearchGraphCursor::bootstrap(SearchGraphCursorBootstrapRequest {
        state: fixture_state("beforeState"),
        materialized_node_ids: vec!["goal:model-owner".to_string()],
    })
    .expect_err("frontier node must be materialized");

    assert_eq!(
        error,
        SearchGraphCursorError::NodeNotMaterialized("symbol:ModelConfig".to_string())
    );
}

#[test]
fn durable_cursor_artifact_validates_and_round_trips() {
    let unchecked: UncheckedSearchGraphCursorArtifact = serde_json::from_str(include_str!(
        "../../../../schemas/fixtures/search-interactive-loop/graph-cursor.v1.json"
    ))
    .expect("parse cursor artifact fixture");
    let artifact =
        SearchGraphCursorArtifact::validate(unchecked).expect("validate cursor artifact");

    assert_eq!(artifact.loop_id().as_str(), "loop:search-1");
    assert_eq!(artifact.cursor().state().current_node(), "goal:model-owner");
    assert_eq!(artifact.cursor().materialized_node_ids().len(), 3);
    assert_eq!(
        artifact.graph_artifact().artifact_schema_id().as_str(),
        "agent.semantic-protocols.semantic-graph-turbo-result"
    );

    SearchGraphCursorArtifact::validate(artifact.into_unchecked())
        .expect("round-trip cursor artifact");
}
