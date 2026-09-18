// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Parser-owned environment-assignment predicate for compiled Hook rules.

use super::ClientHookConfig;
use crate::HookDecision;
use crate::HookRuntime;

pub(super) fn matches(command: &str, expected: &[String]) -> bool {
    expected.is_empty()
        || agent_semantic_shell_parser::parse_bash_command_candidates(command).is_ok_and(|stages| {
            agent_semantic_shell_parser::command_stages_match_process_environment_assignment(
                &stages, expected,
            )
        })
}

impl ClientHookConfig {
    pub(super) fn durable_process_environment_decisions(
        &self,
        runtime: &HookRuntime,
        shell_read_path: Option<&str>,
    ) -> Result<Vec<(Vec<String>, HookDecision)>, String> {
        self.durable_process_environment_assignments()?
            .into_iter()
            .map(|assignment| {
                let action = match shell_read_path {
                    Some(path) => crate::tool_action::ToolAction::normalized_shell_policy_action(
                        format!("{assignment} true {path}"),
                        path.to_owned(),
                    ),
                    None => crate::tool_action::ToolAction::normalized_shell_command_action(
                        format!("{assignment} true"),
                        "Bash".to_owned(),
                    ),
                };
                let candidate = self
                    .classify_candidate(runtime, "codex", "pre-tool", &action)
                    .ok_or_else(|| {
                        format!(
                            "process environment assignment has no compiled Hook decision: {assignment}"
                        )
                    })?;
                if !candidate.terminal {
                    return Err(format!(
                        "process environment assignment Hook decision must be terminal: {assignment}"
                    ));
                }
                let mut decision = candidate.decision;
                decision.fields.insert(
                    "hookPolicySnapshotDigest".to_owned(),
                    serde_json::Value::String(self.policy_generation_digest.clone()),
                );
                decision.fields.insert(
                    "hookPolicyKernelVersion".to_owned(),
                    serde_json::Value::String(
                        crate::protocol::HOOK_POLICY_KERNEL_VERSION.to_owned(),
                    ),
                );
                decision.fields.insert(
                    "hookPolicySynchronousDependencies".to_owned(),
                    serde_json::Value::Array(Vec::new()),
                );
                Ok((vec![assignment], decision))
            })
            .collect()
    }

    fn durable_process_environment_assignments(
        &self,
    ) -> Result<std::collections::BTreeSet<String>, String> {
        let source = self
            .source_config
            .as_ref()
            .ok_or_else(|| "only a source-compiled Hook config may publish shards".to_owned())?;
        Ok(source
            .rules
            .iter()
            .filter(|rule| rule.enabled && rule.terminal)
            .flat_map(|rule| rule.match_config.process_environment_assignment_any.iter())
            .cloned()
            .collect())
    }
}
