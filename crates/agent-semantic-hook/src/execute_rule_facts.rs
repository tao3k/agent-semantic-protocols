//! Execute-only Rule DSL facts projected from a Bash command envelope.
//!
//! Native Codex actions are represented by `AgentAction`. This module is only
//! reachable from Execute rules, so Read/Edit never acquire shell syntax facts.

use crate::tool_action::ToolAction;

pub(crate) struct ExecuteRuleFacts<'a> {
    command: Option<&'a str>,
    command_tokens: Option<Vec<String>>,
    leading_shell_stage: bool,
}

impl ToolAction {
    pub(crate) fn execute_rule_facts<'a>(
        &'a self,
        command_tokens: Option<&'a [String]>,
    ) -> ExecuteRuleFacts<'a> {
        ExecuteRuleFacts::project(self, command_tokens)
    }
}

impl<'a> ExecuteRuleFacts<'a> {
    pub(crate) fn project(action: &'a ToolAction, command_tokens: Option<&[String]>) -> Self {
        Self {
            command: action.command.as_deref(),
            command_tokens: command_tokens.map(ToOwned::to_owned),
            leading_shell_stage: action.leading_shell_stage,
        }
    }

    pub(crate) fn command(&self) -> Option<&str> {
        self.command
    }

    pub(crate) fn leading_shell_stage(&self) -> bool {
        self.leading_shell_stage
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
            agent_semantic_config::WrapperMatchMode::Enable => {
                self.command_tokens.as_deref().map_or_else(
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
                                agent_semantic_shell_parser::candidate_matches_prefix(
                                    candidate, prefix,
                                )
                            })
                        {
                            agent_semantic_shell_parser::PrefixMatch::Matched
                        } else {
                            agent_semantic_shell_parser::PrefixMatch::NotMatched
                        }
                    },
                )
            }
            agent_semantic_config::WrapperMatchMode::Off => {
                self.command_tokens.as_deref().map_or_else(
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
                        } else if agent_semantic_shell_parser::candidate_matches_prefix(
                            tokens, prefix,
                        ) {
                            agent_semantic_shell_parser::PrefixMatch::Matched
                        } else {
                            agent_semantic_shell_parser::PrefixMatch::NotMatched
                        }
                    },
                )
            }
        }
    }
}
