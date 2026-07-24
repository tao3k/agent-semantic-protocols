use agent_semantic_search::{DependencyActionNodeV1, matched_dependency_action_targets};

#[test]
fn dependency_actions_require_typed_query_match_edges() {
    assert_eq!(
        matched_dependency_action_targets(
            [
                DependencyActionNodeV1 {
                    id: "dependency:serde",
                    dependency: "serde",
                },
                DependencyActionNodeV1 {
                    id: "dependency:tokio",
                    dependency: "tokio",
                },
            ],
            ["dependency:serde"],
        ),
        vec!["serde".to_string()]
    );
}

#[test]
fn dependency_actions_fail_closed_without_a_match_edge() {
    assert!(
        matched_dependency_action_targets(
            [DependencyActionNodeV1 {
                id: "dependency:serde",
                dependency: "serde",
            }],
            [],
        )
        .is_empty()
    );
}
