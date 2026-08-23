use agent_semantic_config::{HookClientActionKind, HookClientActionSubjectKind};
use agent_semantic_shell_parser::{CommandStage, parse_bash_command_candidates};

use crate::HookRuntime;
use crate::tool_action::ToolAction;

#[derive(Debug)]
pub(super) struct AgentActionMatch {
    action_any: Vec<HookClientActionKind>,
    subject_kind_any: Vec<HookClientActionSubjectKind>,
    policy_all: Vec<ActionPredicate>,
    policy_any: Vec<ActionPredicate>,
    policy_none: Vec<ActionPredicate>,
}

#[derive(Debug)]
struct ActionPredicate {
    action_any: Vec<HookClientActionKind>,
    semantic_capability_any: Vec<HookClientActionKind>,
    subject_kind_any: Vec<HookClientActionSubjectKind>,
}

#[cfg(test)]
#[path = "../../../../tests/unit/match_policy_contract/production_derivation_contract.rs"]
mod production_derivation_contract;

#[derive(Default)]
pub(super) struct AgentActionMatchConfig {
    pub(super) action_any: Vec<HookClientActionKind>,
    pub(super) subject_kind_any: Vec<HookClientActionSubjectKind>,
    pub(super) policy_all: Vec<agent_semantic_config::HookClientCapabilityPolicyConfig>,
    pub(super) policy_any: Vec<agent_semantic_config::HookClientCapabilityPolicyConfig>,
    pub(super) policy_none: Vec<agent_semantic_config::HookClientCapabilityPolicyConfig>,
}

impl AgentActionMatch {
    pub(super) fn new(config: AgentActionMatchConfig) -> Self {
        let AgentActionMatchConfig {
            action_any,
            subject_kind_any,
            policy_all,
            policy_any,
            policy_none,
        } = config;
        Self {
            action_any,
            subject_kind_any,
            policy_all: policy_all.into_iter().map(ActionPredicate::from).collect(),
            policy_any: policy_any.into_iter().map(ActionPredicate::from).collect(),
            policy_none: policy_none.into_iter().map(ActionPredicate::from).collect(),
        }
    }

    pub(super) fn needs_subjects(&self) -> bool {
        !self.subject_kind_any.is_empty()
            || self.policy_all.iter().any(ActionPredicate::needs_subjects)
            || self.policy_any.iter().any(ActionPredicate::needs_subjects)
            || self.policy_none.iter().any(ActionPredicate::needs_subjects)
    }

    pub(super) fn matches(
        &self,
        registry: &HookRuntime,
        action: &ToolAction,
        match_paths: Option<&[String]>,
    ) -> bool {
        if self.action_any.is_empty()
            && self.subject_kind_any.is_empty()
            && self.policy_all.is_empty()
            && self.policy_any.is_empty()
            && self.policy_none.is_empty()
        {
            return true;
        }
        let agent_action = self.derive_agent_action(registry, action, match_paths, None, false);
        if !self.matches_non_subject_envelope(&agent_action) {
            return false;
        }
        if !self.needs_subjects() {
            return true;
        }
        let agent_action = self.derive_agent_action(registry, action, match_paths, None, true);
        self.matches_envelope(&agent_action)
    }

    fn matches_non_subject_envelope(&self, agent_action: &crate::tool_action::AgentAction) -> bool {
        self.inline_predicate().matches_non_subject(agent_action)
            && self
                .policy_all
                .iter()
                .all(|predicate| predicate.matches_non_subject(agent_action))
            && (self.policy_any.is_empty()
                || self
                    .policy_any
                    .iter()
                    .any(|predicate| predicate.matches_non_subject(agent_action)))
            && self
                .policy_none
                .iter()
                .all(|predicate| !predicate.matches_non_subject(agent_action))
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
        let needs_command_stages = include_subjects
            || agent_action.host.action == crate::action_ir::AgentActionKind::Execute;
        let command_stages = if needs_command_stages {
            self.command_stages(action)
        } else {
            Vec::new()
        };
        let behavior_facts = command_stages
            .iter()
            .flat_map(agent_semantic_shell_parser::command_stage_behavior_facts)
            .collect::<Vec<_>>();
        for fact in &behavior_facts {
            let semantic_action = match fact.access {
                agent_semantic_shell_parser::ShellAccessKind::Read => {
                    crate::action_ir::AgentActionKind::Read
                }
                agent_semantic_shell_parser::ShellAccessKind::Write => {
                    crate::action_ir::AgentActionKind::Edit
                }
            };
            agent_action.add_capability(crate::action_ir::SemanticCapability {
                action: semantic_action,
                evidence: crate::action_ir::SemanticCapabilityEvidence::ShellRedirection,
            });
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
            for subject in behavior_facts
                .iter()
                .filter_map(|fact| fact.subject.as_ref())
            {
                if !subject_paths.contains(subject) {
                    subject_paths.push(subject.clone());
                }
            }
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
        self.inline_predicate().matches(agent_action)
            && self
                .policy_all
                .iter()
                .all(|predicate| predicate.matches(agent_action))
            && (self.policy_any.is_empty()
                || self
                    .policy_any
                    .iter()
                    .any(|predicate| predicate.matches(agent_action)))
            && self
                .policy_none
                .iter()
                .all(|predicate| !predicate.matches(agent_action))
    }

    fn inline_predicate(&self) -> ActionPredicateRef<'_> {
        ActionPredicateRef {
            action_any: &self.action_any,
            semantic_capability_any: &[],
            subject_kind_any: &self.subject_kind_any,
        }
    }

    fn command_stages(&self, action: &ToolAction) -> Vec<CommandStage> {
        action
            .semantic_command_text()
            .and_then(|command| parse_bash_command_candidates(command).ok())
            .unwrap_or_default()
    }
}

struct ActionPredicateRef<'a> {
    action_any: &'a [HookClientActionKind],
    semantic_capability_any: &'a [HookClientActionKind],
    subject_kind_any: &'a [HookClientActionSubjectKind],
}

impl ActionPredicate {
    fn needs_subjects(&self) -> bool {
        !self.subject_kind_any.is_empty()
    }

    fn matches_non_subject(&self, action: &crate::tool_action::AgentAction) -> bool {
        ActionPredicateRef {
            action_any: &self.action_any,
            semantic_capability_any: &self.semantic_capability_any,
            subject_kind_any: &self.subject_kind_any,
        }
        .matches_non_subject(action)
    }

    fn matches(&self, action: &crate::tool_action::AgentAction) -> bool {
        ActionPredicateRef {
            action_any: &self.action_any,
            semantic_capability_any: &self.semantic_capability_any,
            subject_kind_any: &self.subject_kind_any,
        }
        .matches(action)
    }
}

impl ActionPredicateRef<'_> {
    fn matches_non_subject(&self, action: &crate::tool_action::AgentAction) -> bool {
        (self.action_any.is_empty()
            || self.action_any.iter().copied().any(|configured| {
                crate::tool_action::action_kind_matches(action.host.action, configured)
            }))
            && (self.semantic_capability_any.is_empty()
                || action.capabilities.iter().any(|capability| {
                    self.semantic_capability_any
                        .iter()
                        .copied()
                        .any(|configured| {
                            crate::tool_action::action_kind_matches(capability.action, configured)
                        })
                }))
    }

    fn matches(&self, action: &crate::tool_action::AgentAction) -> bool {
        self.matches_non_subject(action)
            && (self.subject_kind_any.is_empty()
                || action.subjects.iter().any(|subject| {
                    self.subject_kind_any.iter().copied().any(|configured| {
                        crate::tool_action::subject_kind_matches(subject.kind, configured)
                    })
                }))
    }
}

impl From<agent_semantic_config::HookClientCapabilityPolicyConfig> for ActionPredicate {
    fn from(policy: agent_semantic_config::HookClientCapabilityPolicyConfig) -> Self {
        Self {
            action_any: policy.action_any,
            semantic_capability_any: policy.semantic_capability_any,
            subject_kind_any: policy.subject_kind_any,
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/hook_config/action_match.rs"]
mod tests;
