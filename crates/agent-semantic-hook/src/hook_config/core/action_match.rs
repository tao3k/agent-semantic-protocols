use agent_semantic_command_match::{
    CommandFlagPresenceV1, CommandInvocationShapeV1, CommandWrapperMatchV1, CommandWrapperSpecV1,
    SemanticCommandInvocationV1, normalize_bash_command_invocations,
};
use agent_semantic_config::HookClientEffectProjection;
use agent_semantic_config::{
    HookClientActionAuthority, HookClientActionKind, HookClientActionSubjectKind,
    HookClientAuthorityProjection, HookClientCommandWrapper, HookClientFlagPresence,
    HookClientInvocationShape, HookClientWrapperMatch,
};

use super::HookRuntime;
use crate::tool_action::{HostActionKind, ToolAction};

#[derive(Debug)]
pub(super) struct SemanticActionMatch {
    authority_projections: Vec<HookClientAuthorityProjection>,
    effect_projections: Vec<HookClientEffectProjection>,
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

impl SemanticActionMatch {
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
        authority_projections: Vec<HookClientAuthorityProjection>,
        effect_projections: Vec<HookClientEffectProjection>,
    ) -> Self {
        Self {
            authority_projections,
            effect_projections,
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
        let (semantic, invocations) = self.projected_action(registry, action, match_paths);
        self.matches_envelope(&semantic, &invocations)
    }

    pub(super) fn projected_host_action(
        &self,
        registry: &HookRuntime,
        action: &ToolAction,
        match_paths: Option<&[String]>,
    ) -> Option<crate::tool_action::SemanticHostAction> {
        self.is_configured()
            .then(|| self.projected_action(registry, action, match_paths).0)
    }

    fn is_configured(&self) -> bool {
        !self.authority_projections.is_empty()
            || !self.effect_projections.is_empty()
            || !self.command_wrappers.is_empty()
            || !self.invocation_shape_any.is_empty()
            || !self.wrapper_match_any.is_empty()
            || !self.flag_presence_any.is_empty()
            || !self.action_any.is_empty()
            || !self.effect_any.is_empty()
            || !self.subject_kind_any.is_empty()
            || !self.authority_any.is_empty()
            || !self.authority_exclude_any.is_empty()
    }

    fn projected_action(
        &self,
        registry: &HookRuntime,
        action: &ToolAction,
        match_paths: Option<&[String]>,
    ) -> (
        crate::tool_action::SemanticHostAction,
        Vec<SemanticCommandInvocationV1>,
    ) {
        let mut semantic = action.semantic_host_action();
        if let Some(authority) = self.projected_authority(registry, action) {
            semantic.authority = authority;
        }
        let invocations = self.command_invocations(action);
        if let Some(effect) = self.projected_effect(&invocations) {
            semantic.effect = effect;
        }
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
        semantic.subjects =
            crate::source_selector::semantic_action_subjects(registry, &subject_paths);

        (semantic, invocations)
    }

    fn matches_envelope(
        &self,
        semantic: &crate::tool_action::SemanticHostAction,
        invocations: &[SemanticCommandInvocationV1],
    ) -> bool {
        self.matches_invocation_facts(semantic.action, invocations)
            && (self.action_any.is_empty()
                || self.action_any.iter().copied().any(|configured| {
                    crate::tool_action::action_kind_matches(semantic.action, configured)
                }))
            && (self.effect_any.is_empty()
                || self.effect_any.iter().copied().any(|configured| {
                    crate::tool_action::action_kind_matches(semantic.effect, configured)
                }))
            && (self.authority_any.is_empty()
                || self.authority_any.iter().copied().any(|configured| {
                    crate::tool_action::authority_matches(semantic.authority, configured)
                }))
            && !self
                .authority_exclude_any
                .iter()
                .copied()
                .any(|configured| {
                    crate::tool_action::authority_matches(semantic.authority, configured)
                })
            && (self.subject_kind_any.is_empty()
                || semantic.subjects.iter().any(|subject| {
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

    fn projected_authority(
        &self,
        registry: &HookRuntime,
        action: &ToolAction,
    ) -> Option<crate::tool_action::ActionAuthority> {
        self.authority_projections.iter().find_map(|projection| {
            crate::hook_config::core::registered_asp::match_registered_asp_command(
                std::slice::from_ref(&projection.argv_prefix),
                registry,
                action,
            )
            .is_some()
            .then(|| crate::tool_action::action_authority_from_config(projection.authority))
        })
    }

    fn projected_effect(
        &self,
        invocations: &[agent_semantic_command_match::SemanticCommandInvocationV1],
    ) -> Option<crate::tool_action::HostActionKind> {
        self.effect_projections.iter().find_map(|projection| {
            matches!(
                agent_semantic_command_match::semantic_invocations_match_prefix(
                    invocations,
                    &projection.argv_prefix,
                ),
                agent_semantic_command_match::PrefixMatch::Matched
            )
            .then(|| crate::tool_action::action_kind_from_config(projection.effect))
            .flatten()
        })
    }

    fn matches_invocation_facts(
        &self,
        action_kind: HostActionKind,
        invocations: &[SemanticCommandInvocationV1],
    ) -> bool {
        let matches_fact = |shape, wrapper_match, flag_presence| {
            (self.invocation_shape_any.is_empty() || self.invocation_shape_any.contains(&shape))
                && (self.wrapper_match_any.is_empty()
                    || self.wrapper_match_any.contains(&wrapper_match))
                && (self.flag_presence_any.is_empty()
                    || self.flag_presence_any.contains(&flag_presence))
        };

        if action_kind != HostActionKind::Execute {
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
#[path = "../../../tests/unit/hook_config/action_match.rs"]
mod tests;
