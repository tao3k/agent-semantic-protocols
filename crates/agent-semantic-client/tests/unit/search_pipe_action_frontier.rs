// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::{ActionNode, ActionRoute};

#[test]
fn workspace_tree_sitter_discovery_materializes_as_search() {
    let action = ActionNode {
        id: "A1".to_string(),
        kind: "tree-sitter".to_string(),
        suffix: "exported-declarations".to_string(),
        route: ActionRoute::TreeSitterQuery {
            language_id: "rust".to_string(),
            recipe: "exported-declarations".to_string(),
            names: vec!["render_next_command_line".to_string()],
            scope: ".".to_string(),
        },
    };

    let command = action
        .materialized_command()
        .expect("materialize workspace Tree-sitter discovery");
    assert!(command.starts_with("asp search playbook --language rust --treesitter-query "));
    assert!(command.ends_with(" --workspace ."));
    assert!(!command.contains(" query --treesitter-query "));
}

#[test]
fn source_query_projection_is_explicit_on_the_exact_surface() {
    let action = ActionNode {
        id: "A1".to_string(),
        kind: "query-projection".to_string(),
        suffix: "source".to_string(),
        route: ActionRoute::QueryProjection {
            language_id: "rust".to_string(),
            selector: agent_semantic_content_identity::CanonicalItemSelector::parse(
                "rust://src/lib.rs#item/function/load",
            )
            .expect("canonical selector"),
            owner: "src/lib.rs".to_string(),
            symbol: "load".to_string(),
            workspace: ".".to_string(),
        },
    };

    assert_eq!(
        action.materialized_command().as_deref(),
        Some(
            "asp query playbook --language rust --selector 'rust://src/lib.rs#item/function/load' --workspace . --projection source"
        )
    );
    let entry = action.frontier_entry();
    assert_eq!(entry.kind, "query-code");
    assert_eq!(entry.capability_id, "query");
    assert_eq!(entry.target_role, "selector");
    assert_eq!(entry.target, "rust://src/lib.rs#item/function/load");
    assert_eq!(entry.fields["projection"], "source");
    assert_eq!(entry.fields["requiresExact"], true);
    let serialized = serde_json::to_value(entry).expect("serialize typed action frontier");
    assert!(serialized.get("command").is_none());
    assert!(serialized.get("argv").is_none());
}
