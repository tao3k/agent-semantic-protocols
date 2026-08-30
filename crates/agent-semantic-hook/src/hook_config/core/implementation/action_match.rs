use crate::HookRuntime;
use crate::tool_action::ToolAction;
use agent_semantic_config::{
    HookClientActionKind, HookClientActionSubjectKind, HookClientHostInvocationKind,
};

#[derive(Debug)]
pub(super) struct AgentActionMatch {
    native_matcher_any: Vec<CodexHostMatcher>,
    host_invocation_any: Vec<HookClientHostInvocationKind>,
    subject_kind_any: Vec<HookClientActionSubjectKind>,
    policy_all: Vec<ActionPredicate>,
    policy_any: Vec<ActionPredicate>,
    policy_none: Vec<ActionPredicate>,
}

#[derive(Debug)]
enum CodexHostMatcher {
    All,
    Exact(Vec<String>),
    Invalid,
}

impl CodexHostMatcher {
    fn compile(expression: String) -> Self {
        if expression.is_empty() || expression == "*" {
            return Self::All;
        }
        let exact = expression.split('|').collect::<Vec<_>>();
        if exact.iter().all(|alias| {
            !alias.is_empty()
                && !alias.chars().any(|character| {
                    matches!(
                        character,
                        '^' | '$' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '\\'
                    )
                })
        }) {
            return Self::Exact(exact.into_iter().map(str::to_owned).collect());
        }
        Self::Invalid
    }

    fn matches(&self, candidate: &str) -> bool {
        match self {
            Self::All => true,
            Self::Exact(exact) => exact.iter().any(|value| value == candidate),
            Self::Invalid => false,
        }
    }

    fn exact_values(&self) -> Option<&[String]> {
        match self {
            Self::All => None,
            Self::Exact(exact) => Some(exact),
            Self::Invalid => Some(&[]),
        }
    }
}

#[derive(Debug)]
struct ActionPredicate {
    host_invocation_any: Vec<HookClientHostInvocationKind>,
    semantic_capability_any: Vec<HookClientActionKind>,
    subject_kind_any: Vec<HookClientActionSubjectKind>,
}

#[cfg(test)]
#[path = "../../../../tests/unit/match_policy_contract/production_derivation_contract.rs"]
mod production_derivation_contract;

#[derive(Default)]
pub(super) struct AgentActionMatchConfig {
    pub(super) native_matcher_any: Vec<String>,
    pub(super) host_invocation_any: Vec<HookClientHostInvocationKind>,
    pub(super) subject_kind_any: Vec<HookClientActionSubjectKind>,
    pub(super) policy_all: Vec<agent_semantic_config::HookClientCapabilityPolicyConfig>,
    pub(super) policy_any: Vec<agent_semantic_config::HookClientCapabilityPolicyConfig>,
    pub(super) policy_none: Vec<agent_semantic_config::HookClientCapabilityPolicyConfig>,
}

impl AgentActionMatch {
    pub(super) fn new(config: AgentActionMatchConfig) -> Self {
        let AgentActionMatchConfig {
            native_matcher_any,
            host_invocation_any,
            subject_kind_any,
            policy_all,
            policy_any,
            policy_none,
        } = config;
        Self {
            native_matcher_any: native_matcher_any
                .into_iter()
                .map(CodexHostMatcher::compile)
                .collect(),
            host_invocation_any,
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

    pub(super) fn needs_profile_subjects(&self) -> bool {
        self.native_matcher_any
            .iter()
            .any(|matcher| matcher.matches("Read"))
            || self
                .policy_all
                .iter()
                .chain(self.policy_any.iter())
                .any(|predicate| {
                    predicate.semantic_capability_any.iter().any(|capability| {
                        matches!(
                            capability,
                            HookClientActionKind::Read | HookClientActionKind::Unknown
                        )
                    }) || !predicate.subject_kind_any.is_empty()
                })
    }

    pub(super) fn indexed_host_matcher_keys(&self) -> Option<Vec<&str>> {
        let mut keys = Vec::new();
        for matcher in &self.native_matcher_any {
            let exact = matcher.exact_values()?;
            keys.extend(exact.iter().map(String::as_str));
        }
        Some(keys)
    }

    pub(super) fn can_match_indexed_host_tool(&self, tool_name: &str) -> bool {
        self.native_matcher_any.is_empty()
            || self.native_matcher_any.iter().any(|matcher| {
                std::iter::once(tool_name)
                    .chain(host_matcher_aliases(tool_name).iter().copied())
                    .any(|candidate| matcher.matches(candidate))
            })
    }

    pub(super) fn matches(
        &self,
        registry: &HookRuntime,
        platform: &str,
        action: &ToolAction,
        match_paths: Option<&[String]>,
    ) -> bool {
        if self.native_matcher_any.is_empty()
            && self.subject_kind_any.is_empty()
            && self.policy_all.is_empty()
            && self.policy_any.is_empty()
            && self.policy_none.is_empty()
        {
            return true;
        }
        let agent_action =
            self.derive_agent_action(registry, platform, action, match_paths, None, false);
        if !self.matches_non_subject_envelope(&agent_action) {
            return false;
        }
        if !self.needs_subjects() {
            return true;
        }
        let agent_action =
            self.derive_agent_action(registry, platform, action, match_paths, None, true);
        self.matches_envelope(&agent_action)
    }

    fn matches_non_subject_envelope(&self, agent_action: &crate::tool_action::AgentAction) -> bool {
        self.matches_native_matcher(agent_action)
            && self.matches_host_invocations(agent_action)
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
        platform: &str,
        action: &ToolAction,
        match_paths: Option<&[String]>,
        structured_source_operands: Option<&[String]>,
    ) -> Option<crate::tool_action::AgentAction> {
        Some(self.derive_agent_action(
            registry,
            platform,
            action,
            match_paths,
            structured_source_operands,
            true,
        ))
    }

    pub(super) fn matching_subject_paths(
        &self,
        platform: &str,
        registry: &HookRuntime,
        action: &ToolAction,
        match_paths: &[String],
        structured_source_operands: Option<&[String]>,
    ) -> Vec<String> {
        self.derive_agent_action(
            registry,
            platform,
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
        _platform: &str,
        action: &ToolAction,
        match_paths: Option<&[String]>,
        structured_source_operands: Option<&[String]>,
        include_subjects: bool,
    ) -> crate::tool_action::AgentAction {
        let mut agent_action = crate::action_ir::project_agent_action(
            registry,
            action,
            match_paths,
            structured_source_operands,
        );
        if include_subjects {
            if !self.subject_kind_any.is_empty() {
                agent_action.subjects.retain(|subject| {
                    self.subject_kind_any.iter().copied().any(|configured| {
                        crate::tool_action::subject_kind_matches(subject.kind, configured)
                    })
                });
            }
        } else {
            agent_action.subjects.clear();
        }

        agent_action
    }

    fn matches_envelope(&self, agent_action: &crate::tool_action::AgentAction) -> bool {
        self.matches_native_matcher(agent_action)
            && self.matches_host_invocations(agent_action)
            && (self.subject_kind_any.is_empty()
                || agent_action.subjects.iter().any(|subject| {
                    self.subject_kind_any.iter().copied().any(|configured| {
                        crate::tool_action::subject_kind_matches(subject.kind, configured)
                    })
                }))
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

    fn matches_native_matcher(&self, action: &crate::tool_action::AgentAction) -> bool {
        self.native_matcher_any.is_empty()
            || self.native_matcher_any.iter().any(|matcher| {
                std::iter::once(action.host.tool_name.as_str())
                    .chain(
                        host_matcher_aliases(action.host.tool_name.as_str())
                            .iter()
                            .copied(),
                    )
                    .any(|candidate| matcher.matches(candidate))
            })
    }

    fn matches_host_invocations(&self, action: &crate::tool_action::AgentAction) -> bool {
        self.host_invocation_any.is_empty()
            || self.host_invocation_any.iter().copied().any(|configured| {
                crate::action_ir::host_invocation_kind_matches(action.host.action, configured)
            })
    }
}

fn host_matcher_aliases(tool_name: &str) -> &'static [&'static str] {
    match tool_name {
        "apply_patch" => &["Edit", "Write"],
        "spawn_agent" => &["Agent"],
        _ => &[],
    }
}

struct ActionPredicateRef<'a> {
    host_invocation_any: &'a [HookClientHostInvocationKind],
    semantic_capability_any: &'a [HookClientActionKind],
    subject_kind_any: &'a [HookClientActionSubjectKind],
}

impl ActionPredicate {
    fn needs_subjects(&self) -> bool {
        !self.subject_kind_any.is_empty()
    }

    fn matches_non_subject(&self, action: &crate::tool_action::AgentAction) -> bool {
        ActionPredicateRef {
            host_invocation_any: &self.host_invocation_any,
            semantic_capability_any: &self.semantic_capability_any,
            subject_kind_any: &self.subject_kind_any,
        }
        .matches_non_subject(action)
    }

    fn matches(&self, action: &crate::tool_action::AgentAction) -> bool {
        ActionPredicateRef {
            host_invocation_any: &self.host_invocation_any,
            semantic_capability_any: &self.semantic_capability_any,
            subject_kind_any: &self.subject_kind_any,
        }
        .matches(action)
    }
}

impl ActionPredicateRef<'_> {
    fn matches_non_subject(&self, action: &crate::tool_action::AgentAction) -> bool {
        (self.host_invocation_any.is_empty()
            || self.host_invocation_any.iter().copied().any(|configured| {
                crate::action_ir::host_invocation_kind_matches(action.host.action, configured)
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
            host_invocation_any: policy.host_invocation_any,
            semantic_capability_any: policy.semantic_capability_any,
            subject_kind_any: policy.subject_kind_any,
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/hook_config/action_match.rs"]
mod tests;
