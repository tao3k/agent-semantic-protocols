use agent_semantic_command_match::{CommandStageV1, parse_bash_command_candidates};
use agent_semantic_config::AgentActionEffectRule;
use agent_semantic_config::{
    AgentActionAuthorityRule, HookClientActionAuthority, HookClientActionKind,
    HookClientActionSubjectKind,
};

use crate::HookRuntime;
use crate::tool_action::ToolAction;

#[derive(Debug)]
pub(super) struct AgentActionMatch {
    authority_rules: Vec<AgentActionAuthorityRule>,
    effect_rules: Vec<AgentActionEffectRule>,
    action_any: Vec<HookClientActionKind>,
    effect_any: Vec<HookClientActionKind>,
    subject_kind_any: Vec<HookClientActionSubjectKind>,
    authority_any: Vec<HookClientActionAuthority>,
    authority_exclude_any: Vec<HookClientActionAuthority>,
}

#[cfg(test)]
#[path = "../../../../tests/unit/match_policy_contract/production_derivation_contract.rs"]
mod production_derivation_contract;

#[derive(Default)]
pub(super) struct AgentActionMatchConfig {
    pub(super) authority_rules: Vec<AgentActionAuthorityRule>,
    pub(super) effect_rules: Vec<AgentActionEffectRule>,
    pub(super) action_any: Vec<HookClientActionKind>,
    pub(super) effect_any: Vec<HookClientActionKind>,
    pub(super) subject_kind_any: Vec<HookClientActionSubjectKind>,
    pub(super) authority_any: Vec<HookClientActionAuthority>,
    pub(super) authority_exclude_any: Vec<HookClientActionAuthority>,
}

impl AgentActionMatch {
    pub(super) fn new(config: AgentActionMatchConfig) -> Self {
        let AgentActionMatchConfig {
            authority_rules,
            effect_rules,
            action_any,
            effect_any,
            subject_kind_any,
            authority_any,
            authority_exclude_any,
        } = config;
        Self {
            authority_rules,
            effect_rules,
            action_any,
            effect_any,
            subject_kind_any,
            authority_any,
            authority_exclude_any,
        }
    }

    pub(super) fn needs_subjects(&self) -> bool {
        !self.subject_kind_any.is_empty()
    }

    pub(super) fn matches(
        &self,
        registry: &HookRuntime,
        action: &ToolAction,
        match_paths: Option<&[String]>,
    ) -> bool {
        if self.authority_rules.is_empty()
            && self.effect_rules.is_empty()
            && self.action_any.is_empty()
            && self.effect_any.is_empty()
            && self.subject_kind_any.is_empty()
            && self.authority_any.is_empty()
            && self.authority_exclude_any.is_empty()
        {
            return true;
        }
        let agent_action = self.derive_agent_action(registry, action, match_paths, None, false);
        if !self.matches_non_subject_envelope(&agent_action) {
            return false;
        }
        if self.subject_kind_any.is_empty() {
            return true;
        }
        let agent_action = self.derive_agent_action(registry, action, match_paths, None, true);
        self.matches_envelope(&agent_action)
    }

    fn matches_non_subject_envelope(&self, agent_action: &crate::tool_action::AgentAction) -> bool {
        (self.action_any.is_empty()
            || self.action_any.iter().copied().any(|configured| {
                crate::tool_action::action_kind_matches(agent_action.action, configured)
            }))
            && (self.effect_any.is_empty()
                || self.effect_any.iter().copied().any(|configured| {
                    crate::tool_action::action_kind_matches(agent_action.effect, configured)
                }))
            && (self.authority_any.is_empty()
                || self.authority_any.iter().copied().any(|configured| {
                    crate::tool_action::authority_matches(agent_action.authority, configured)
                }))
            && !self
                .authority_exclude_any
                .iter()
                .copied()
                .any(|configured| {
                    crate::tool_action::authority_matches(agent_action.authority, configured)
                })
    }

    pub(super) fn derive_agent_action_for_rule(
        &self,
        registry: &HookRuntime,
        action: &ToolAction,
        match_paths: Option<&[String]>,
        structured_source_operands: Option<&[String]>,
    ) -> Option<crate::tool_action::AgentAction> {
        Some(self.derive_agent_action(
            registry,
            action,
            match_paths,
            structured_source_operands,
            true,
        ))
    }

    pub(super) fn matching_subject_paths(
        &self,
        registry: &HookRuntime,
        action: &ToolAction,
        match_paths: &[String],
        structured_source_operands: Option<&[String]>,
    ) -> Vec<String> {
        self.derive_agent_action(
            registry,
            action,
            Some(match_paths),
            structured_source_operands,
            true,
        )
        .subjects
        .into_iter()
        .map(|subject| subject.value)
        .collect()
    }

    fn derive_agent_action(
        &self,
        registry: &HookRuntime,
        action: &ToolAction,
        match_paths: Option<&[String]>,
        structured_source_operands: Option<&[String]>,
        include_subjects: bool,
    ) -> crate::tool_action::AgentAction {
        let mut agent_action = action.derive_agent_action();
        if let Some(authority) = self.infer_authority(registry, action) {
            agent_action.authority = authority;
        }
        let needs_command_stages =
            include_subjects || !self.effect_rules.is_empty() || !self.effect_any.is_empty();
        let command_stages = if needs_command_stages {
            self.command_stages(action)
        } else {
            Vec::new()
        };
        if let Some(effect) = self.infer_effect(&command_stages, action.semantic_command_text()) {
            agent_action.effect = effect;
        }
        if include_subjects {
            let mut subject_paths = if let Some(source_operands) = structured_source_operands {
                source_operands.to_vec()
            } else {
                let invocation_operands = command_stages
                    .iter()
                    .flat_map(|stage| stage.words().iter().skip(1).cloned())
                    .collect::<Vec<_>>();
                let invocation_operands = crate::source_selector::project_shell_subject_paths(
                    registry,
                    &invocation_operands,
                );
                let mut subject_paths =
                    if action.operation == crate::tool_action::OperationIntent::ShellCommand {
                        crate::source_selector::project_shell_subject_paths(
                            registry,
                            match_paths.unwrap_or_default(),
                        )
                    } else {
                        match_paths.unwrap_or_default().to_vec()
                    };
                for operand in invocation_operands {
                    if !subject_paths.contains(&operand) {
                        subject_paths.push(operand);
                    }
                }
                subject_paths
            };
            subject_paths.dedup();
            agent_action.subjects =
                crate::source_selector::derive_agent_action_subjects(registry, &subject_paths);
            if !self.subject_kind_any.is_empty() {
                agent_action.subjects.retain(|subject| {
                    self.subject_kind_any.iter().copied().any(|configured| {
                        crate::tool_action::subject_kind_matches(subject.kind, configured)
                    })
                });
            }
        }

        agent_action
    }

    fn matches_envelope(&self, agent_action: &crate::tool_action::AgentAction) -> bool {
        (self.action_any.is_empty()
            || self.action_any.iter().copied().any(|configured| {
                crate::tool_action::action_kind_matches(agent_action.action, configured)
            }))
            && (self.effect_any.is_empty()
                || self.effect_any.iter().copied().any(|configured| {
                    crate::tool_action::action_kind_matches(agent_action.effect, configured)
                }))
            && (self.authority_any.is_empty()
                || self.authority_any.iter().copied().any(|configured| {
                    crate::tool_action::authority_matches(agent_action.authority, configured)
                }))
            && !self
                .authority_exclude_any
                .iter()
                .copied()
                .any(|configured| {
                    crate::tool_action::authority_matches(agent_action.authority, configured)
                })
            && (self.subject_kind_any.is_empty()
                || agent_action.subjects.iter().any(|subject| {
                    self.subject_kind_any.iter().copied().any(|configured| {
                        crate::tool_action::subject_kind_matches(subject.kind, configured)
                    })
                }))
    }

    fn command_stages(&self, action: &ToolAction) -> Vec<CommandStageV1> {
        action
            .semantic_command_text()
            .and_then(|command| parse_bash_command_candidates(command).ok())
            .unwrap_or_default()
    }

    fn infer_authority(
        &self,
        registry: &HookRuntime,
        action: &ToolAction,
    ) -> Option<crate::tool_action::AgentActionAuthority> {
        self.authority_rules.iter().find_map(|rule| {
            crate::hook_config::core::registered_asp::match_registered_asp_command(
                std::slice::from_ref(&rule.argv_prefix),
                registry,
                action,
            )
            .is_some()
            .then(|| crate::tool_action::action_authority_from_config(rule.authority))
        })
    }

    fn infer_effect(
        &self,
        command_stages: &[CommandStageV1],
        command: Option<&str>,
    ) -> Option<crate::tool_action::AgentActionKind> {
        self.effect_rules.iter().find_map(|rule| {
            let prefix_matches = !rule.argv_prefix.is_empty()
                && matches!(
                    agent_semantic_command_match::command_stages_match_wrapped_prefix(
                        command_stages,
                        &rule.argv_prefix,
                    ),
                    agent_semantic_command_match::PrefixMatch::Matched
                );
            let command_matches = command.is_some_and(|command| {
                let command = command.to_ascii_lowercase();
                rule.command_contains_any.iter().any(|pattern| {
                    !pattern.is_empty() && command.contains(&pattern.to_ascii_lowercase())
                })
            });
            (prefix_matches || command_matches)
                .then(|| crate::tool_action::action_kind_from_config(rule.effect))
                .flatten()
        })
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/hook_config/action_match.rs"]
mod tests;
