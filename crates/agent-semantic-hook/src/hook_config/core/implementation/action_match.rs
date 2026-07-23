use agent_semantic_command_match::{
    CommandFlagPresenceV1, CommandInvocationShapeV1, CommandWrapperMatchV1, CommandWrapperSpecV1,
    SemanticCommandInvocationV1, normalize_bash_command_invocations,
};
use agent_semantic_config::AgentActionEffectRule;
use agent_semantic_config::{
    AgentActionAuthorityRule, HookClientActionAuthority, HookClientActionKind,
    HookClientActionSubjectKind, HookClientCommandWrapper, HookClientFlagPresence,
    HookClientInvocationShape, HookClientWrapperMatch,
};

use crate::HookRuntime;
use crate::tool_action::{AgentActionKind, ToolAction};

#[derive(Debug)]
pub(super) struct AgentActionMatch {
    authority_rules: Vec<AgentActionAuthorityRule>,
    effect_rules: Vec<AgentActionEffectRule>,
    command_wrappers: Vec<CommandWrapperSpecV1>,
    invocation_shape_any: Vec<HookClientInvocationShape>,
    wrapper_match_any: Vec<HookClientWrapperMatch>,
    flag_presence_any: Vec<HookClientFlagPresence>,
    action_any: Vec<HookClientActionKind>,
    effect_any: Vec<HookClientActionKind>,
    subject_kind_any: Vec<HookClientActionSubjectKind>,
    authority_any: Vec<HookClientActionAuthority>,
    authority_exclude_any: Vec<HookClientActionAuthority>,
}

impl AgentActionMatch {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        command_wrappers: Vec<HookClientCommandWrapper>,
        invocation_shape_any: Vec<HookClientInvocationShape>,
        wrapper_match_any: Vec<HookClientWrapperMatch>,
        flag_presence_any: Vec<HookClientFlagPresence>,
        action_any: Vec<HookClientActionKind>,
        effect_any: Vec<HookClientActionKind>,
        subject_kind_any: Vec<HookClientActionSubjectKind>,
        authority_any: Vec<HookClientActionAuthority>,
        authority_exclude_any: Vec<HookClientActionAuthority>,
        authority_rules: Vec<AgentActionAuthorityRule>,
        effect_rules: Vec<AgentActionEffectRule>,
    ) -> Self {
        Self {
            authority_rules,
            effect_rules,
            command_wrappers: command_wrappers
                .into_iter()
                .map(|wrapper| CommandWrapperSpecV1 {
                    executable: wrapper.executable,
                })
                .collect(),
            invocation_shape_any,
            wrapper_match_any,
            flag_presence_any,
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
        let (agent_action, invocations) = self.derive_agent_action(
            registry,
            action,
            match_paths,
            !self.subject_kind_any.is_empty(),
        );
        self.matches_envelope(&agent_action, &invocations)
    }

    pub(super) fn derive_agent_action_for_rule(
        &self,
        registry: &HookRuntime,
        action: &ToolAction,
        match_paths: Option<&[String]>,
    ) -> Option<crate::tool_action::AgentAction> {
        Some(
            self.derive_agent_action(registry, action, match_paths, true)
                .0,
        )
    }

    fn derive_agent_action(
        &self,
        registry: &HookRuntime,
        action: &ToolAction,
        match_paths: Option<&[String]>,
        include_subjects: bool,
    ) -> (
        crate::tool_action::AgentAction,
        Vec<SemanticCommandInvocationV1>,
    ) {
        let mut agent_action = action.derive_agent_action();
        if let Some(authority) = self.infer_authority(registry, action) {
            agent_action.authority = authority;
        }
        let needs_invocations = include_subjects
            || !self.effect_rules.is_empty()
            || !self.invocation_shape_any.is_empty()
            || !self.wrapper_match_any.is_empty()
            || !self.flag_presence_any.is_empty()
            || !self.effect_any.is_empty();
        let invocations = if needs_invocations {
            self.command_invocations(action)
        } else {
            Vec::new()
        };
        if let Some(effect) = self.infer_effect(&invocations, action.semantic_command_text()) {
            agent_action.effect = effect;
        }
        if include_subjects {
            let invocation_operands = invocations
                .iter()
                .flat_map(|invocation| invocation.operands.iter().cloned())
                .collect::<Vec<_>>();
            let mut subject_paths = match_paths.unwrap_or_default().to_vec();
            for operand in invocation_operands {
                if !subject_paths.contains(&operand) {
                    subject_paths.push(operand);
                }
            }
            agent_action.subjects =
                crate::source_selector::derive_agent_action_subjects(registry, &subject_paths);
        }

        (agent_action, invocations)
    }

    fn matches_envelope(
        &self,
        agent_action: &crate::tool_action::AgentAction,
        invocations: &[SemanticCommandInvocationV1],
    ) -> bool {
        self.matches_invocation_facts(agent_action.action, invocations)
            && (self.action_any.is_empty()
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

    fn command_invocations(&self, action: &ToolAction) -> Vec<SemanticCommandInvocationV1> {
        action
            .semantic_command_text()
            .and_then(|command| {
                normalize_bash_command_invocations(command, &self.command_wrappers).ok()
            })
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
        invocations: &[agent_semantic_command_match::SemanticCommandInvocationV1],
        command: Option<&str>,
    ) -> Option<crate::tool_action::AgentActionKind> {
        self.effect_rules.iter().find_map(|rule| {
            let prefix_matches = !rule.argv_prefix.is_empty()
                && matches!(
                    agent_semantic_command_match::semantic_invocations_match_prefix(
                        invocations,
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

    fn matches_invocation_facts(
        &self,
        action_kind: AgentActionKind,
        invocations: &[SemanticCommandInvocationV1],
    ) -> bool {
        let matches_fact = |shape, wrapper_match, flag_presence| {
            (self.invocation_shape_any.is_empty() || self.invocation_shape_any.contains(&shape))
                && (self.wrapper_match_any.is_empty()
                    || self.wrapper_match_any.contains(&wrapper_match))
                && (self.flag_presence_any.is_empty()
                    || self.flag_presence_any.contains(&flag_presence))
        };

        if action_kind != AgentActionKind::Execute {
            return matches_fact(
                HookClientInvocationShape::HostNative,
                HookClientWrapperMatch::Unmatched,
                HookClientFlagPresence::Absent,
            );
        }
        if invocations.is_empty() {
            return matches_fact(
                HookClientInvocationShape::Command,
                HookClientWrapperMatch::Unknown,
                HookClientFlagPresence::Absent,
            );
        }

        invocations.iter().any(|invocation| {
            let shape = match invocation.shape {
                CommandInvocationShapeV1::Command => HookClientInvocationShape::Command,
                CommandInvocationShapeV1::WrappedCommand => {
                    HookClientInvocationShape::WrappedCommand
                }
            };
            let wrapper_match = match invocation.wrapper_match {
                CommandWrapperMatchV1::Matched => HookClientWrapperMatch::Matched,
                CommandWrapperMatchV1::Unmatched => HookClientWrapperMatch::Unmatched,
            };
            let flag_presence = match invocation.flag_presence {
                CommandFlagPresenceV1::Present => HookClientFlagPresence::Present,
                CommandFlagPresenceV1::Absent => HookClientFlagPresence::Absent,
            };
            matches_fact(shape, wrapper_match, flag_presence)
        })
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/hook_config/action_match.rs"]
mod tests;
