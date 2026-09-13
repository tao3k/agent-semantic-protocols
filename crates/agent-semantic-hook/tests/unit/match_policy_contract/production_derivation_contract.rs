// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::AgentActionMatch;
use super::AgentActionMatchConfig;
use crate::HookRuntime;
use crate::tool_action::ToolAction;

fn runtime() -> HookRuntime {
    HookRuntime {
        project_root: ".".to_owned(),
        policy_providers: Vec::new(),
    }
}

#[test]
fn production_derivation_is_host_fact_then_parser_fact() {
    let matcher = AgentActionMatch::new(AgentActionMatchConfig::default());
    let action = ToolAction::normalized_shell_command_action(
        "opaque < input.rs > output.rs".to_owned(),
        "Bash".to_owned(),
    );
    let receipt = matcher
        .derive_agent_action_for_rule(&runtime(), "codex", &action, None, None)
        .expect("AgentAction")
        .receipt_value();

    assert_eq!(receipt["hostInvocation"]["toolName"], "Bash");
    assert_eq!(receipt["hostInvocation"]["action"], "execute");
    assert_eq!(
        receipt["semanticCapabilities"][0]["evidence"],
        "host-matcher"
    );
    assert!(
        receipt["semanticCapabilities"]
            .as_array()
            .is_some_and(|items| {
                items
                    .iter()
                    .any(|item| item["action"] == "read" && item["evidence"] == "shell-redirection")
                    && items.iter().any(|item| {
                        item["action"] == "edit" && item["evidence"] == "shell-redirection"
                    })
            })
    );
}
