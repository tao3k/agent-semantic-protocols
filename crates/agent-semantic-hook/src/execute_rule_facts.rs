// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Execute-only Rule DSL facts projected from a Bash command envelope.
//!
//! Native Codex actions are represented by `AgentAction`. This module is only
//! reachable from Execute rules, so Read/Edit never acquire shell syntax facts.

use crate::tool_action::ToolAction;

pub(crate) struct ExecuteRuleFacts<'command, 'tokens> {
    command: Option<&'command str>,
    shell_envelope_command: Option<&'command str>,
    command_tokens: Option<&'tokens [String]>,
}

impl ToolAction {
    pub(crate) fn execute_rule_facts<'command, 'tokens>(
        &'command self,
        command_tokens: Option<&'tokens [String]>,
    ) -> ExecuteRuleFacts<'command, 'tokens> {
        ExecuteRuleFacts::project(self, command_tokens)
    }
}

impl<'command, 'tokens> ExecuteRuleFacts<'command, 'tokens> {
    pub(crate) fn project(
        action: &'command ToolAction,
        command_tokens: Option<&'tokens [String]>,
    ) -> Self {
        Self {
            command: action.command.as_deref(),
            shell_envelope_command: action.shell_envelope_command.as_deref(),
            command_tokens,
        }
    }

    pub(crate) fn command(&self) -> Option<&str> {
        self.command
    }

    pub(crate) fn shell_envelope_command(&self) -> Option<&str> {
        self.shell_envelope_command.or(self.command)
    }

    pub(crate) fn matches_wrapped_prefix(
        &self,
        wrapper_match: agent_semantic_config::WrapperMatchMode,
        prefix: &[String],
    ) -> agent_semantic_shell_parser::PrefixMatch {
        let Some(command) = self.command else {
            return agent_semantic_shell_parser::PrefixMatch::NotMatched;
        };
        match wrapper_match {
            agent_semantic_config::WrapperMatchMode::Enable => self.command_tokens.map_or_else(
                || {
                    crate::shell_parser::bash::parse_bash_command_candidates(command).map_or(
                        agent_semantic_shell_parser::PrefixMatch::BudgetExceeded,
                        |stages| {
                            agent_semantic_shell_parser::command_stages_match_wrapped_prefix(
                                &stages, prefix,
                            )
                        },
                    )
                },
                |tokens| {
                    if tokens.len() > agent_semantic_shell_parser::MAX_STAGE_TOKENS {
                        agent_semantic_shell_parser::PrefixMatch::BudgetExceeded
                    } else if prefix.is_empty()
                        || tokens.windows(prefix.len()).any(|candidate| {
                            agent_semantic_shell_parser::candidate_matches_prefix(candidate, prefix)
                        })
                    {
                        agent_semantic_shell_parser::PrefixMatch::Matched
                    } else {
                        agent_semantic_shell_parser::PrefixMatch::NotMatched
                    }
                },
            ),
            agent_semantic_config::WrapperMatchMode::Off => self.command_tokens.map_or_else(
                || {
                    crate::shell_parser::bash::parse_bash_command_candidates(command).map_or(
                        agent_semantic_shell_parser::PrefixMatch::BudgetExceeded,
                        |stages| {
                            agent_semantic_shell_parser::command_stages_match_prefix(
                                &stages, prefix,
                            )
                        },
                    )
                },
                |tokens| {
                    if tokens.len() > agent_semantic_shell_parser::MAX_STAGE_TOKENS {
                        agent_semantic_shell_parser::PrefixMatch::BudgetExceeded
                    } else if agent_semantic_shell_parser::candidate_matches_prefix(tokens, prefix)
                    {
                        agent_semantic_shell_parser::PrefixMatch::Matched
                    } else {
                        agent_semantic_shell_parser::PrefixMatch::NotMatched
                    }
                },
            ),
        }
    }
}
