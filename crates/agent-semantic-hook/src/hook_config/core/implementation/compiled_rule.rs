// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Coordinates compiled Hook rule owners through crate-level facades.

use super::action_match_facade as action_match;

use agent_semantic_config::HookClientConfigDecision;
use agent_semantic_config::HookClientConfigFile;
use agent_semantic_config::HookClientConfigRouteKind;
use agent_semantic_config::HookClientRuleConfig;
use agent_semantic_config::HookClientRuleMatchConfig;
use agent_semantic_config::HookClientRuleRouteConfig;

use crate::AgentOrgArtifactsArchiveWarning;
use crate::AgentOrgArtifactsRecovery;
use crate::CompiledAgentOrgArtifactsConfig;
use crate::HookRuntime;
use crate::collect_source_selector_matches;
use crate::hook_config::compile_agent_org_artifacts_config;
use crate::protocol::DecisionKind;
use crate::protocol::DecisionRouteKind;
use crate::protocol::HOOK_DECISION_SCHEMA_ID;
use crate::protocol::HOOK_DECISION_SCHEMA_VERSION;
use crate::protocol::HOOK_PROTOCOL_ID;
use crate::protocol::HOOK_PROTOCOL_VERSION;
use crate::protocol::HookDecision;
use crate::protocol::ReasonKind;
use crate::protocol::StdinMode;
use crate::tool_action::ToolAction;
use crate::tool_action::subject_for_action;

#[path = "compiled_rule_conversion.rs"]
mod conversion;
#[path = "compiled_rule_environment_assignment.rs"]
mod environment_assignment;
#[path = "compiled_rule_matcher_helpers.rs"]
mod matcher_helpers;
#[path = "compiled_rule_structured_projection_template.rs"]
mod structured_projection_template;
use matcher_helpers::fast_path_token;
use matcher_helpers::path_without_line_range;
use matcher_helpers::structured_document_format;
#[path = "compiled_hook_rule.rs"]
mod compiled_hook_rule;
#[path = "compiled_rule_match.rs"]
mod rule_match_helpers;

#[derive(Debug)]
/// Compiled hook rules loaded from the global ASP state root.
pub struct ClientHookConfig {
    source_config: Option<agent_semantic_config::HookClientConfigFile>,
    pub(in crate::hook_config) rules: Vec<CompiledHookRule>,
    rule_candidates: RuleCandidateIndex,
    policy_receipt: std::sync::OnceLock<HookPolicyReceipt>,
    profiles: std::collections::BTreeMap<String, agent_semantic_config::HookClientProfileConfig>,
    policy_generation_digest: String,
    provider_projections:
        Vec<crate::protocol_activation::protocol_activation_manifest::HookProviderProjection>,
    contract_fingerprint: Option<String>,
    agent_org_artifacts: CompiledAgentOrgArtifactsConfig,
    command_action_patterns: Vec<agent_semantic_config::HookClientCommandActionPatternConfig>,
}

#[derive(Debug)]
struct HookPolicyReceipt {
    generation_digest: String,
    kernel_version: &'static str,
}

type RuleCandidateIndex = std::collections::BTreeMap<
    String,
    std::collections::BTreeMap<String, std::collections::BTreeMap<String, Vec<usize>>>,
>;

#[derive(Debug)]
pub(in crate::hook_config) struct CompiledHookRule {
    id: String,
    priority: i64,
    terminal: bool,
    intent: Option<String>,
    fields: std::collections::BTreeMap<String, String>,
    pub(in crate::hook_config) dispatch: Option<CompiledRuleDispatch>,
    decision: HookClientConfigDecision,
    reason_kind: ReasonKind,
    message: Option<String>,
    language_ids: Vec<agent_semantic_config::LanguageId>,
    event: Option<String>,
    platform: Option<String>,
    match_config: RuleMatch,
    routes: Vec<RuleRoute>,
}

#[derive(Debug)]
pub(in crate::hook_config) struct CompiledRuleDispatch {
    pub(super) transport: agent_semantic_config::HookClientRuleDispatchTransport,
    pub(in crate::hook_config) target_agent: String,
    pub(super) receipt_kind: String,
    pub(super) calling: agent_semantic_config::HookClientAgentCallingConfig,
    lazy_provider: Option<agent_semantic_config::HookClientLazyProviderPolicy>,
}

#[derive(Debug)]
pub(super) struct RuleMatch {
    agent_action: action_match::AgentActionMatch,
    wrapper_match: agent_semantic_config::WrapperMatchMode,
    tool_any: Vec<String>,
    command_any: Vec<String>,
    argv_pattern_any: Vec<Vec<String>>,
    argv_prefix_any: Vec<Vec<String>>,
    argv_token_all: Vec<String>,
    process_environment_assignment_any: Vec<String>,
    command_contains_any: CompiledCommandContains,
    path_any: Vec<String>,
    path_glob_any: CompiledPathGlobs,
    profile_extension_any: Vec<String>,
    profile_any: Vec<agent_semantic_config::HookClientProfileConfig>,
    pub(super) argv_source_any: Vec<String>,
    pub(super) argv_source_glob_any: CompiledPathGlobs,
    pub(super) argv_source_exclude_flag_any: Vec<String>,
    pub(super) argv_workspace_regular_file: bool,
    pub(super) argv_structured_document_file: bool,
    pub(super) argv_registered_source_file: bool,
    structured_projection: Option<CompiledStructuredProjection>,
}

#[derive(Debug)]
struct CompiledStructuredProjection {
    config: agent_semantic_config::HookClientStructuredProjectionMatchConfig,
}

#[derive(Default)]
struct ResolvedCapabilityPolicies {
    all: Vec<agent_semantic_config::HookClientCapabilityPolicyConfig>,
    any: Vec<agent_semantic_config::HookClientCapabilityPolicyConfig>,
    none: Vec<agent_semantic_config::HookClientCapabilityPolicyConfig>,
}

#[derive(Debug)]
struct RuleRoute {
    provider_id: agent_semantic_config::ProviderId,
    language_id: agent_semantic_config::LanguageId,
    binary: Option<String>,
    kind: DecisionRouteKind,
    argv: Vec<String>,
    stdin_mode: Option<StdinMode>,
}

impl CompiledHookRule {
    fn matches_after_paths(&self, runtime: &HookRuntime, paths: &[String]) -> bool {
        self.matches_language(runtime, paths) && self.match_config.matches_paths(paths)
    }

    fn decision(
        &self,
        runtime: &HookRuntime,
        platform: &str,
        event: &str,
        action: &ToolAction,
        paths: &[String],
        structured_source_operands: Option<&[String]>,
    ) -> HookDecision {
        let matched_subject_paths = (self.match_config.agent_action.needs_subjects()
            || self.match_config.needs_profile_subjects())
        .then(|| {
            self.match_config.agent_action.matching_subject_paths(
                platform,
                runtime,
                action,
                paths,
                structured_source_operands,
            )
        });
        let shell_subject_paths = (matched_subject_paths.is_none()
            && action.surface == crate::tool_action::ToolSurface::CodexShell)
            .then(|| crate::source_selector::project_shell_subject_paths(runtime, paths));
        let paths = matched_subject_paths
            .as_deref()
            .or(shell_subject_paths.as_deref())
            .unwrap_or(paths);
        let decision = match self.decision {
            HookClientConfigDecision::Allow => DecisionKind::Allow,
            HookClientConfigDecision::Block => DecisionKind::Block,
            HookClientConfigDecision::Deny => DecisionKind::Deny,
        };
        let mut routes = self
            .routes
            .iter()
            .map(|route| route.decision_route(runtime))
            .collect::<Vec<_>>();
        if routes.is_empty() {
            routes = self.materialize_profile_routes(runtime, paths);
        }
        let message = self.rendered_message(platform);
        let mut subject = subject_for_action(action);
        subject.paths = paths.to_vec();
        let mut decision_fields = self
            .fields
            .iter()
            .map(|(key, value)| (key.clone(), serde_json::Value::String(value.clone())))
            .collect::<std::collections::BTreeMap<_, _>>();
        if let Some(agent_action) =
            self.agent_action_receipt(runtime, platform, action, paths, structured_source_operands)
        {
            decision_fields.insert("agentAction".to_string(), agent_action);
        }
        super::dispatch_fields::extend_dispatch_fields(
            &mut decision_fields,
            self.dispatch.as_ref(),
            platform,
            action,
        );
        let registered_asp = crate::hook_config::core::registered_asp::match_registered_asp_command(
            &self.match_config.argv_pattern_any,
            runtime,
            action,
        );
        let language_ids = registered_asp
            .as_ref()
            .map(|matched| vec![matched.language_id.clone()])
            .unwrap_or_else(|| {
                if self.language_ids.is_empty() {
                    routes
                        .iter()
                        .map(|route| route.language_id.clone())
                        .collect()
                } else {
                    self.language_ids.clone()
                }
            });
        if let Some(matched) = registered_asp.as_ref() {
            crate::hook_config::core::registered_asp::append_materialization_fields(
                &mut decision_fields,
                matched,
                self.dispatch
                    .as_ref()
                    .and_then(|dispatch| dispatch.lazy_provider),
            );
        }
        decision_fields.insert(
            "configRuleId".to_string(),
            serde_json::Value::String(self.id.clone()),
        );
        if let Some(intent) = self.intent.as_ref() {
            decision_fields.insert(
                "intent".to_string(),
                serde_json::Value::String(intent.clone()),
            );
        }
        HookDecision {
            schema_id: HOOK_DECISION_SCHEMA_ID,
            schema_version: HOOK_DECISION_SCHEMA_VERSION,
            protocol_id: HOOK_PROTOCOL_ID,
            protocol_version: HOOK_PROTOCOL_VERSION,
            platform: platform.to_string(),
            event: event.to_string(),
            decision,
            reason_kind: self.reason_kind,
            language_ids,
            subject,
            routes,
            message,
            fields: decision_fields,
        }
    }
}

impl RuleMatch {
    fn durable_matcher_artifact(&self) -> DurableRuleMatcherArtifact {
        DurableRuleMatcherArtifact {
            command_contains: self.command_contains_any.durable_artifact(),
            path_glob: self.path_glob_any.durable_artifact(),
            argv_source_glob: self.argv_source_glob_any.durable_artifact(),
            profile_extension_any: self.profile_extension_any.clone(),
            profile_any: self.profile_any.clone(),
        }
    }

    fn try_from_config(
        mut config: HookClientRuleMatchConfig,
        durable_matcher: Option<DurableRuleMatcherArtifact>,
        policies: ResolvedCapabilityPolicies,
        _executable_capabilities: Option<&std::collections::BTreeSet<String>>,
    ) -> Result<Self, String> {
        let mut tool_any = std::mem::take(&mut config.tool_any);
        if let Some(tool) = config.tool.take() {
            tool_any.push(tool);
        }
        let (
            command_contains_any,
            path_glob_any,
            argv_source_glob_any,
            profile_extension_any,
            profile_any,
        ) = match durable_matcher {
            Some(durable) => (
                CompiledCommandContains::from_durable(durable.command_contains)?,
                CompiledPathGlobs::from_durable(durable.path_glob)?,
                CompiledPathGlobs::from_durable(durable.argv_source_glob)?,
                durable.profile_extension_any,
                durable.profile_any,
            ),
            None => (
                compile_command_contains(std::mem::take(&mut config.command_contains_any))?,
                compile_globs("pathGlobAny", std::mem::take(&mut config.path_glob_any))?,
                compile_globs(
                    "argvSourceGlobAny",
                    std::mem::take(&mut config.argv_source_glob_any),
                )?,
                std::mem::take(&mut config.profile_extension_any),
                std::mem::take(&mut config.profile_any),
            ),
        };
        Ok(Self {
            agent_action: action_match::AgentActionMatch::new(
                action_match::AgentActionMatchConfig {
                    native_matcher_any: std::mem::take(&mut config.native_matcher_any),
                    host_invocation_any: std::mem::take(&mut config.host_invocation_any),
                    subject_kind_any: std::mem::take(&mut config.subject_kind_any),
                    policy_all: policies.all,
                    policy_any: policies.any,
                    policy_none: policies.none,
                    command_action_patterns: Vec::new(),
                },
            ),
            wrapper_match: agent_semantic_config::WrapperMatchMode::default(),
            tool_any,
            command_any: config.command_any,
            argv_pattern_any: config.argv_pattern_any,
            argv_prefix_any: config.argv_prefix_any,
            argv_token_all: config.argv_token_all,
            process_environment_assignment_any: config.process_environment_assignment_any,
            command_contains_any,
            path_any: config.path_any,
            path_glob_any,
            profile_extension_any,
            profile_any,
            argv_source_any: config.argv_source_any,
            argv_source_glob_any,
            argv_source_exclude_flag_any: config.argv_source_exclude_flag_any,
            argv_workspace_regular_file: config.argv_workspace_regular_file,
            argv_structured_document_file: config.argv_structured_document_file,
            argv_registered_source_file: config.argv_registered_source_file,
            structured_projection: config
                .structured_projection
                .map(|config| CompiledStructuredProjection { config }),
        })
    }

    fn matches_before_paths(
        &self,
        registry: &HookRuntime,
        platform: &str,
        action: &ToolAction,
        command_tokens: Option<&[String]>,
        match_paths: Option<&[String]>,
    ) -> bool {
        self.agent_action
            .matches(registry, platform, action, match_paths)
            && self.matches_untyped_facts(registry, action, command_tokens)
    }

    fn matches_untyped_facts(
        &self,
        runtime: &HookRuntime,
        action: &ToolAction,
        command_tokens: Option<&[String]>,
    ) -> bool {
        let execute_facts = action.execute_rule_facts(command_tokens);
        self.matches_tool(action)
            && self.matches_command(&execute_facts)
            && (self.argv_pattern_any.is_empty()
                || crate::hook_config::core::registered_asp::match_registered_asp_command(
                    &self.argv_pattern_any,
                    runtime,
                    action,
                )
                .is_some())
    }

    pub(in crate::hook_config) fn structured_projection_source_operands(
        &self,
        action: &ToolAction,
    ) -> Result<Option<Vec<String>>, ()> {
        let Some(projection) = self.structured_projection.as_ref() else {
            return Ok(None);
        };
        crate::hook_config::core::structured_projection::match_source_operands(
            &projection.config,
            action,
        )
        .map(Some)
        .ok_or(())
    }

    fn matches_command(&self, facts: &crate::execute_rule_facts::ExecuteRuleFacts<'_, '_>) -> bool {
        if self.command_any.is_empty()
            && self.argv_prefix_any.is_empty()
            && self.argv_token_all.is_empty()
            && self.process_environment_assignment_any.is_empty()
            && self.command_contains_any.is_empty()
        {
            return true;
        }
        let Some(command) = facts.command() else {
            return false;
        };
        let token_match = self.command_any.is_empty()
            || self.command_any.iter().any(|expected| {
                facts
                    .matches_wrapped_prefix(self.wrapper_match, std::slice::from_ref(expected))
                    .routes_protected()
            });
        let contains_match =
            self.command_contains_any.is_empty() || self.command_contains_any.matches(command);
        let prefix_match = self.argv_prefix_any.is_empty()
            || self.argv_prefix_any.iter().any(|prefix| {
                facts
                    .matches_wrapped_prefix(self.wrapper_match, prefix)
                    .routes_protected()
            });
        let argv_token_match = self.argv_token_all.is_empty()
            || agent_semantic_shell_parser::parse_bash_command_candidates(command).is_ok_and(
                |stages| {
                    stages.iter().any(|stage| {
                        self.argv_token_all.iter().all(|expected| {
                            stage
                                .words()
                                .iter()
                                .any(|actual| actual.eq_ignore_ascii_case(expected))
                        })
                    })
                },
            );
        let process_environment_match = environment_assignment::matches(
            facts.shell_envelope_command().unwrap_or(command),
            &self.process_environment_assignment_any,
        );
        token_match
            && prefix_match
            && argv_token_match
            && contains_match
            && process_environment_match
    }

    fn matches_path(&self, paths: &[String]) -> bool {
        if self.path_any.is_empty()
            && self.path_glob_any.is_empty()
            && self.profile_extension_any.is_empty()
        {
            return true;
        }
        let exact_match = !self.path_any.is_empty()
            && paths.iter().any(|path| {
                self.path_any
                    .iter()
                    .any(|expected| path == expected || path.ends_with(expected))
            });
        let glob_match = paths.iter().any(|path| self.path_glob_any.matches(path));
        let profile_extension_match = paths.iter().any(|path| {
            std::path::Path::new(path)
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| {
                    self.profile_extension_any
                        .iter()
                        .any(|expected| extension.eq_ignore_ascii_case(expected))
                })
        });
        exact_match || glob_match || profile_extension_match
    }

    pub(super) fn fast_argv_source_path(
        &self,
        project_root: &std::path::Path,
        token: &str,
    ) -> Option<String> {
        let path = fast_path_token(token)?;
        if self.matches_argv_source_path(project_root, path) {
            return Some(path.to_string());
        }
        let base = path_without_line_range(path)?;
        self.matches_argv_source_path(project_root, base)
            .then(|| base.to_string())
    }

    pub(super) fn matches_argv_source_path(
        &self,
        project_root: &std::path::Path,
        path: &str,
    ) -> bool {
        let exact_match = !self.argv_source_any.is_empty()
            && self
                .argv_source_any
                .iter()
                .any(|expected| path == expected || path.ends_with(expected));
        let glob_match = self.argv_source_glob_any.matches(path);
        exact_match || glob_match || self.matches_workspace_regular_file(project_root, path)
    }

    fn matches_workspace_regular_file(&self, project_root: &std::path::Path, path: &str) -> bool {
        if !self.argv_workspace_regular_file && !self.argv_structured_document_file {
            return false;
        }
        let candidate = std::path::Path::new(path);
        let candidate = if candidate.is_absolute() {
            candidate.to_path_buf()
        } else {
            project_root.join(candidate)
        };
        let Ok(project_root) = project_root.canonicalize() else {
            return false;
        };
        let Ok(candidate) = candidate.canonicalize() else {
            return false;
        };
        if !candidate.starts_with(&project_root)
            || !candidate
                .metadata()
                .is_ok_and(|metadata| metadata.is_file())
        {
            return false;
        }
        self.matches_configured_document_format(&candidate)
    }
}

impl TryFrom<HookClientRuleConfig> for CompiledHookRule {
    type Error = String;

    fn try_from(config: HookClientRuleConfig) -> Result<Self, Self::Error> {
        Self::try_from_with_policy(
            config,
            &[],
            &[],
            &[],
            &[],
            agent_semantic_config::WrapperMatchMode::default(),
            &agent_semantic_config::HookClientAgentCallingConfig::default(),
        )
    }
}

impl CompiledHookRule {
    pub(in crate::hook_config) fn try_from_with_policy(
        config: HookClientRuleConfig,
        command_profiles: &[agent_semantic_config::HookClientCommandProfileConfig],
        command_sets: &[agent_semantic_config::HookClientCommandSetConfig],
        capability_policies: &[agent_semantic_config::HookClientCapabilityPolicyConfig],
        command_action_patterns: &[agent_semantic_config::HookClientCommandActionPatternConfig],
        wrapper_match: agent_semantic_config::WrapperMatchMode,
        agent_calling: &agent_semantic_config::HookClientAgentCallingConfig,
    ) -> Result<Self, String> {
        Self::try_from_with_policy_and_matcher(
            config,
            command_profiles,
            command_sets,
            capability_policies,
            command_action_patterns,
            wrapper_match,
            agent_calling,
            None,
            None,
        )
    }

    fn try_from_with_policy_and_matcher(
        config: HookClientRuleConfig,
        command_profiles: &[agent_semantic_config::HookClientCommandProfileConfig],
        command_sets: &[agent_semantic_config::HookClientCommandSetConfig],
        capability_policies: &[agent_semantic_config::HookClientCapabilityPolicyConfig],
        command_action_patterns: &[agent_semantic_config::HookClientCommandActionPatternConfig],
        wrapper_match: agent_semantic_config::WrapperMatchMode,
        agent_calling: &agent_semantic_config::HookClientAgentCallingConfig,
        durable_matcher: Option<DurableRuleMatcherArtifact>,
        executable_capabilities: Option<&std::collections::BTreeSet<String>>,
    ) -> Result<Self, String> {
        let declared_actions = config.actions.clone();
        let wrapper_match =
            if config.matcher_policies.iter().any(|policy| {
                *policy == agent_semantic_config::HookClientMatcherPolicy::WrappedCommand
            }) {
                agent_semantic_config::WrapperMatchMode::Enable
            } else {
                wrapper_match
            };
        let dispatch = config
            .dispatch
            .map(|dispatch| {
                Ok::<CompiledRuleDispatch, String>(CompiledRuleDispatch {
                    transport: dispatch.transport,
                    target_agent: dispatch.agent.as_str().to_owned(),
                    receipt_kind: dispatch.receipt_kind.as_str().to_owned(),
                    calling: agent_calling.clone(),
                    lazy_provider: dispatch.lazy_provider,
                })
            })
            .transpose()?;
        let reason_kind = config
            .reason_kind
            .map(ReasonKind::from)
            .unwrap_or(ReasonKind::None);
        let dynamic_profile_route = !config.match_config.profile_extension_any.is_empty()
            || !config.match_config.profile_any.is_empty();
        if dynamic_profile_route && reason_kind == ReasonKind::RegisteredSourceRouteRequired {
            let referenced_semantic_capabilities = config
                .match_config
                .capability_policy_all
                .iter()
                .filter_map(|reference| {
                    capability_policies
                        .iter()
                        .find(|policy| policy.id == *reference)
                })
                .flat_map(|policy| policy.semantic_capability_any.iter().copied())
                .collect::<Vec<_>>();
            let native_matchers_are_read = !config.match_config.native_matcher_any.is_empty()
                && config
                    .match_config
                    .native_matcher_any
                    .iter()
                    .all(|matcher| matcher == "Read");
            let semantic_capabilities_are_typed_read = !referenced_semantic_capabilities.is_empty()
                && referenced_semantic_capabilities.iter().all(|capability| {
                    *capability == agent_semantic_config::HookClientActionKind::Read
                });
            let declared_actions_are_typed_read = !declared_actions.is_empty()
                && declared_actions
                    .iter()
                    .all(|action| *action == agent_semantic_config::HookClientActionKind::Read);
            if !native_matchers_are_read
                && !semantic_capabilities_are_typed_read
                && !declared_actions_are_typed_read
            {
                return Err(format!(
                    "rule `{}` expands registered source without an exact Read action contract",
                    config.id
                ));
            }
        }
        let mut raw_match_config = config.match_config;
        for prefix in agent_semantic_config::expand_command_profile_prefixes(
            &raw_match_config.command_profile_any,
            command_profiles,
        )? {
            if !raw_match_config.argv_prefix_any.contains(&prefix) {
                raw_match_config.argv_prefix_any.push(prefix);
            }
        }
        for prefix in agent_semantic_config::expand_command_set_prefixes(
            &raw_match_config.command_set_any,
            command_sets,
        )? {
            if !raw_match_config.argv_prefix_any.contains(&prefix) {
                raw_match_config.argv_prefix_any.push(prefix);
            }
        }
        let resolve_capability_policies = |axis: &str, references: &mut Vec<String>| {
            std::mem::take(references)
                .into_iter()
                .map(|reference| {
                    capability_policies
                        .iter()
                        .find(|policy| policy.id == reference)
                        .cloned()
                        .ok_or_else(|| {
                            format!(
                                "rule `{}` {axis} references unknown capability policy `{reference}`",
                                config.id
                            )
                        })
                })
                .collect::<Result<Vec<_>, String>>()
        };
        let mut policy_all = resolve_capability_policies(
            "capabilityPolicyAll",
            &mut raw_match_config.capability_policy_all,
        )?;
        if !declared_actions.is_empty() {
            policy_all.push(agent_semantic_config::HookClientCapabilityPolicyConfig {
                id: format!("rule-action:{}", config.id),
                host_invocation_any: Vec::new(),
                semantic_capability_any: declared_actions,
                subject_kind_any: Vec::new(),
            });
        }
        let policy_any = resolve_capability_policies(
            "capabilityPolicyAny",
            &mut raw_match_config.capability_policy_any,
        )?;
        let policy_none = resolve_capability_policies(
            "capabilityPolicyNone",
            &mut raw_match_config.capability_policy_none,
        )?;
        let mut match_config = RuleMatch::try_from_config(
            raw_match_config,
            durable_matcher,
            ResolvedCapabilityPolicies {
                all: policy_all,
                any: policy_any,
                none: policy_none,
            },
            executable_capabilities,
        )?;
        match_config.agent_action.command_action_patterns = command_action_patterns.to_vec();
        match_config.wrapper_match = wrapper_match;
        Ok(Self {
            id: config.id,
            priority: config.priority,
            terminal: config.terminal,
            intent: config.intent,
            fields: config.fields,
            dispatch,
            decision: config.decision,
            reason_kind,
            message: config.message,
            language_ids: config
                .language_ids
                .into_iter()
                .map(agent_semantic_config::LanguageId::try_new)
                .collect::<Result<Vec<_>, _>>()?,
            event: config.event,
            platform: config.platform,
            match_config,
            routes: config
                .routes
                .into_iter()
                .map(RuleRoute::try_from)
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

impl TryFrom<HookClientRuleMatchConfig> for RuleMatch {
    type Error = String;

    fn try_from(config: HookClientRuleMatchConfig) -> Result<Self, Self::Error> {
        Self::try_from_config(config, None, ResolvedCapabilityPolicies::default(), None)
    }
}

impl TryFrom<HookClientRuleRouteConfig> for RuleRoute {
    type Error = String;

    fn try_from(config: HookClientRuleRouteConfig) -> Result<Self, Self::Error> {
        let provider_id = agent_semantic_config::ProviderId::try_new(config.provider_id)?;
        let registered_language = crate::provider_registry::registered_language_ids()
            .into_iter()
            .find(|language_id| {
                crate::provider_registry::registered_provider_id(language_id.as_str())
                    .is_some_and(|registered| registered == provider_id.as_str())
            })
            .ok_or_else(|| {
                format!("hook route references unregistered provider `{provider_id}`")
            })?;
        let language_id = config
            .language_id
            .map(agent_semantic_config::LanguageId::try_new)
            .transpose()?
            .unwrap_or_else(|| registered_language.clone());
        if language_id != registered_language {
            return Err(format!(
                "hook route provider `{provider_id}` belongs to language `{}`, not `{language_id}`",
                registered_language
            ));
        }
        Ok(Self {
            provider_id,
            language_id,
            binary: config.binary,
            kind: DecisionRouteKind::from(config.kind),
            argv: config.argv,
            stdin_mode: config.stdin_mode.map(StdinMode::from),
        })
    }
}

impl From<HookClientConfigRouteKind> for DecisionRouteKind {
    fn from(kind: HookClientConfigRouteKind) -> Self {
        match kind {
            HookClientConfigRouteKind::Playbook => Self::Playbook,
            HookClientConfigRouteKind::Query => Self::Query,
            HookClientConfigRouteKind::Read => Self::Read,
            HookClientConfigRouteKind::CheckChanged => Self::CheckChanged,
        }
    }
}

#[path = "compiled_rule_client_config.rs"]
mod client_config;

#[path = "compiled_rule_durable_artifact.rs"]
mod durable_artifact;

pub(in crate::hook_config) use client_config::compile_config;
pub(in crate::hook_config) use client_config::compile_config_with_executable_capabilities;
use durable_artifact::DURABLE_HOOK_MATCHER_SCHEMA_ID;
use durable_artifact::DURABLE_HOOK_MATCHER_SCHEMA_VERSION;
pub use durable_artifact::DurableHookConfigArtifact;
use durable_artifact::DurableRuleMatcherArtifact;

use crate::hook_config::core::compile_command_contains;
use crate::hook_config::core::compile_globs;
use crate::hook_config::core::match_types::CompiledCommandContains;
use crate::hook_config::core::match_types::CompiledPathGlobs;

fn canonical_event(value: &str) -> String {
    value.to_ascii_lowercase().replace('_', "-")
}
#[path = "compiled_rule_message.rs"]
mod compiled_rule_message;
