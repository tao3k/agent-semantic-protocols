//! Coordinates compiled Hook rule owners through crate-level facades.

use super::action_match_facade as action_match;

use agent_semantic_config::{
    HookClientConfigDecision, HookClientConfigFile, HookClientConfigReasonKind,
    HookClientConfigRouteKind, HookClientConfigStdinMode, HookClientRuleConfig,
    HookClientRuleMatchConfig, HookClientRuleRouteConfig,
};

use crate::hook_config::AspSessionPolicy;
use crate::hook_config::compile_agent_org_artifacts_config;
use crate::protocol::{
    DecisionKind, DecisionRoute, DecisionRouteKind, HOOK_DECISION_SCHEMA_ID,
    HOOK_DECISION_SCHEMA_VERSION, HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, HookDecision,
    ReasonKind, StdinMode,
};
use crate::{
    AgentOrgArtifactsArchiveWarning, AgentOrgArtifactsRecovery, CompiledAgentOrgArtifactsConfig,
    CompiledRecoveryPromptConfig, HookRuntime, collect_source_selector_matches,
};

use crate::tool_action::{ToolAction, subject_for_action};

#[path = "compiled_rule_structured_projection_template.rs"]
mod structured_projection_template;

#[derive(Debug)]
/// Compiled hook rules loaded from the global ASP state root.
pub struct ClientHookConfig {
    source_config: Option<agent_semantic_config::HookClientConfigFile>,
    pub(in crate::hook_config) rules: Vec<CompiledHookRule>,
    rule_candidates: RuleCandidateIndex,
    policy_receipt: std::sync::OnceLock<HookPolicyReceipt>,
    language_providers: Vec<agent_semantic_config::HookClientLanguageProviderConfig>,
    policy_generation_digest: String,
    provider_projections:
        Vec<crate::protocol_activation::protocol_activation_manifest::HookProviderProjection>,
    contract_fingerprint: Option<String>,
    semantic_ast_patch_disabled: bool,
    agent_org_artifacts: CompiledAgentOrgArtifactsConfig,
    recovery_prompt: CompiledRecoveryPromptConfig,
    asp_session_policy: AspSessionPolicy,
    agent_session_messages: agent_semantic_config::HookClientAgentSessionMessagesConfig,
}

#[derive(Debug)]
struct HookPolicyReceipt {
    generation_digest: String,
    kernel_version: &'static str,
}

type RuleCandidateIndex =
    std::collections::BTreeMap<String, std::collections::BTreeMap<String, Vec<usize>>>;

#[derive(Debug)]
pub(in crate::hook_config) struct CompiledHookRule {
    id: String,
    priority: i64,
    intent: Option<String>,
    fields: std::collections::BTreeMap<String, String>,
    pub(in crate::hook_config) dispatch: Option<CompiledRuleDispatch>,
    decision: HookClientConfigDecision,
    decision_materializer: Option<agent_semantic_config::HookClientDecisionMaterializer>,
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
    pub(in crate::hook_config) resident_name: String,
    pub(in crate::hook_config) resident_codex_agent_name: String,
    pub(in crate::hook_config) resident_role: String,
    pub(in crate::hook_config) resident_agent_kind: String,
    pub(in crate::hook_config) resident_display_role: String,
    pub(in crate::hook_config) resident_description: String,
    pub(super) receipt_kind: String,
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
    command_contains_any: CompiledCommandContains,
    path_any: Vec<String>,
    path_glob_any: CompiledPathGlobs,
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
    capability_available: bool,
}

#[derive(Default)]
struct ResolvedActionPolicies {
    all: Vec<agent_semantic_config::HookClientActionPolicyConfig>,
    any: Vec<agent_semantic_config::HookClientActionPolicyConfig>,
    none: Vec<agent_semantic_config::HookClientActionPolicyConfig>,
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
    fn canonical_event_key(&self) -> Option<String> {
        self.event.as_deref().map(canonical_event)
    }

    fn canonical_platform_key(&self) -> Option<String> {
        self.platform
            .as_ref()
            .map(|value| value.to_ascii_lowercase())
    }

    fn durable_matcher_artifact(&self) -> DurableRuleMatcherArtifact {
        self.match_config.durable_matcher_artifact()
    }

    fn rendered_message(&self) -> String {
        compiled_rule_message::render(self)
    }

    fn needs_decision_paths(&self) -> bool {
        self.match_config.needs_source_paths()
    }

    fn matches_before_paths(
        &self,
        runtime: &HookRuntime,
        platform: &str,
        event: &str,
        action: &ToolAction,
        command_tokens: Option<&[String]>,
    ) -> bool {
        self.platform
            .as_deref()
            .is_none_or(|expected| expected.eq_ignore_ascii_case(platform))
            && self
                .event
                .as_deref()
                .is_none_or(|expected| canonical_event(expected) == canonical_event(event))
            && self.match_config.matches_before_paths(
                runtime,
                action,
                command_tokens,
                Some(action.paths.as_slice()),
            )
    }

    fn matches_after_paths(&self, runtime: &HookRuntime, paths: &[String]) -> bool {
        self.matches_language(runtime, paths) && self.match_config.matches_paths(paths)
    }

    fn agent_action_receipt(
        &self,
        runtime: &HookRuntime,
        action: &ToolAction,
        paths: &[String],
        structured_source_operands: Option<&[String]>,
    ) -> Option<serde_json::Value> {
        self.match_config
            .agent_action
            .derive_agent_action_for_rule(runtime, action, Some(paths), structured_source_operands)
            .map(|agent_action| agent_action.receipt_value())
    }

    fn matches_language(&self, runtime: &HookRuntime, paths: &[String]) -> bool {
        if self.language_ids.is_empty() {
            return true;
        }
        if paths.is_empty() {
            return false;
        }
        !collect_source_selector_matches(runtime, paths.iter().map(String::as_str), |provider| {
            self.language_ids
                .iter()
                .any(|language_id| language_id == &provider.language_id)
        })
        .is_empty()
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
        let matched_subject_paths = self.match_config.agent_action.needs_subjects().then(|| {
            self.match_config.agent_action.matching_subject_paths(
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
        let routes = self
            .routes
            .iter()
            .map(|route| route.decision_route(runtime))
            .collect::<Vec<_>>();
        let mut message = self.rendered_message();
        if self.reason_kind == ReasonKind::SubagentReceiptRequired
            && let Some(command) = action.command.as_deref()
        {
            message.push_str(&format!("\nDenied command: `{command}`."));
        }
        let mut subject = subject_for_action(action);
        subject.paths = paths.to_vec();
        let mut decision_fields = self
            .fields
            .iter()
            .map(|(key, value)| (key.clone(), serde_json::Value::String(value.clone())))
            .collect::<std::collections::BTreeMap<_, _>>();
        if let Some(agent_action) =
            self.agent_action_receipt(runtime, action, paths, structured_source_operands)
        {
            decision_fields.insert("agentAction".to_string(), agent_action);
        }
        super::dispatch_fields::extend_dispatch_fields(
            &mut decision_fields,
            self.dispatch.as_ref(),
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
            .unwrap_or_else(|| self.language_ids.clone());
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
        }
    }

    fn try_from_config(
        mut config: HookClientRuleMatchConfig,
        durable_matcher: Option<DurableRuleMatcherArtifact>,
        policies: ResolvedActionPolicies,
        executable_capabilities: Option<&std::collections::BTreeSet<String>>,
    ) -> Result<Self, String> {
        let mut tool_any = std::mem::take(&mut config.tool_any);
        if let Some(tool) = config.tool.take() {
            tool_any.push(tool);
        }
        let (command_contains_any, path_glob_any, argv_source_glob_any) = match durable_matcher {
            Some(durable) => (
                CompiledCommandContains::from_durable(durable.command_contains)?,
                CompiledPathGlobs::from_durable(durable.path_glob)?,
                CompiledPathGlobs::from_durable(durable.argv_source_glob)?,
            ),
            None => (
                compile_command_contains(std::mem::take(&mut config.command_contains_any))?,
                compile_globs("pathGlobAny", std::mem::take(&mut config.path_glob_any))?,
                compile_globs(
                    "argvSourceGlobAny",
                    std::mem::take(&mut config.argv_source_glob_any),
                )?,
            ),
        };
        Ok(Self {
            agent_action: action_match::AgentActionMatch::new(
                action_match::AgentActionMatchConfig {
                    action_any: std::mem::take(&mut config.action_any),
                    effect_any: std::mem::take(&mut config.effect_any),
                    subject_kind_any: std::mem::take(&mut config.subject_kind_any),
                    authority_any: std::mem::take(&mut config.authority_any),
                    authority_exclude_any: std::mem::take(&mut config.authority_exclude_any),
                    authority_rules: std::mem::take(&mut config.authority_rules),
                    effect_rules: std::mem::take(&mut config.effect_rules),
                    policy_all: policies.all,
                    policy_any: policies.any,
                    policy_none: policies.none,
                },
            ),
            wrapper_match: agent_semantic_config::WrapperMatchMode::default(),
            tool_any,
            command_any: config.command_any,
            argv_pattern_any: config.argv_pattern_any,
            argv_prefix_any: config.argv_prefix_any,
            command_contains_any,
            path_any: config.path_any,
            path_glob_any,
            argv_source_any: config.argv_source_any,
            argv_source_glob_any,
            argv_source_exclude_flag_any: config.argv_source_exclude_flag_any,
            argv_workspace_regular_file: config.argv_workspace_regular_file,
            argv_structured_document_file: config.argv_structured_document_file,
            argv_registered_source_file: config.argv_registered_source_file,
            structured_projection: config.structured_projection.map(|config| {
                let capability_available = executable_capabilities.map_or_else(
                    || {
                        crate::executable::resolve_executable_with_status(&config.binary).status
                            == crate::executable::ExecutableStatus::Available
                    },
                    |available| available.contains(&config.binary),
                );
                CompiledStructuredProjection {
                    config,
                    capability_available,
                }
            }),
        })
    }

    fn matches_before_paths(
        &self,
        registry: &HookRuntime,
        action: &ToolAction,
        command_tokens: Option<&[String]>,
        match_paths: Option<&[String]>,
    ) -> bool {
        self.agent_action.matches(registry, action, match_paths)
            && self.matches_untyped_facts(registry, action, command_tokens)
    }

    fn matches_untyped_facts(
        &self,
        runtime: &HookRuntime,
        action: &ToolAction,
        command_tokens: Option<&[String]>,
    ) -> bool {
        self.matches_tool(action)
            && self.matches_command(action, command_tokens)
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
            projection.capability_available,
            action,
        )
        .map(Some)
        .ok_or(())
    }

    fn matches_paths(&self, paths: &[String]) -> bool {
        self.matches_path(paths)
    }

    fn matches_tool(&self, action: &ToolAction) -> bool {
        self.tool_any.is_empty()
            || self
                .tool_any
                .iter()
                .any(|tool| tool.eq_ignore_ascii_case(&action.tool_name))
    }

    fn needs_command_tokens(&self) -> bool {
        !self.command_any.is_empty()
            || !self.argv_prefix_any.is_empty()
            || !self.argv_source_any.is_empty()
            || !self.argv_source_glob_any.is_empty()
            || self.argv_workspace_regular_file
            || self.argv_structured_document_file
            || self.argv_registered_source_file
            || self.structured_projection.is_some()
    }

    fn needs_path_match(&self) -> bool {
        !self.path_any.is_empty() || !self.path_glob_any.is_empty()
    }

    fn needs_source_paths(&self) -> bool {
        self.agent_action.needs_subjects()
            || self.needs_path_match()
            || !self.argv_source_any.is_empty()
            || !self.argv_source_glob_any.is_empty()
            || self.argv_workspace_regular_file
            || self.argv_structured_document_file
            || self.argv_registered_source_file
    }

    fn needs_argv_source_match(&self) -> bool {
        !self.argv_source_any.is_empty()
            || !self.argv_source_glob_any.is_empty()
            || self.argv_workspace_regular_file
            || self.argv_structured_document_file
            || self.argv_registered_source_file
    }

    fn matches_command(&self, action: &ToolAction, command_tokens: Option<&[String]>) -> bool {
        if self.command_any.is_empty()
            && self.argv_prefix_any.is_empty()
            && self.command_contains_any.is_empty()
        {
            return true;
        }
        let Some(command) = action.command.as_deref() else {
            return false;
        };
        let matches_prefix = |prefix: &[String]| match self.wrapper_match {
            agent_semantic_config::WrapperMatchMode::Enable => command_tokens.map_or_else(
                || {
                    crate::command_match::bash::parse_bash_command_candidates(command).map_or(
                        agent_semantic_command_match::PrefixMatch::BudgetExceeded,
                        |stages| {
                            agent_semantic_command_match::command_stages_match_wrapped_prefix(
                                &stages, prefix,
                            )
                        },
                    )
                },
                |tokens| {
                    if tokens.len() > agent_semantic_command_match::MAX_STAGE_TOKENS {
                        agent_semantic_command_match::PrefixMatch::BudgetExceeded
                    } else {
                        if prefix.is_empty()
                            || tokens.windows(prefix.len()).any(|candidate| {
                                agent_semantic_command_match::candidate_matches_prefix(
                                    candidate, prefix,
                                )
                            })
                        {
                            agent_semantic_command_match::PrefixMatch::Matched
                        } else {
                            agent_semantic_command_match::PrefixMatch::NotMatched
                        }
                    }
                },
            ),
        };
        let token_match = self.command_any.is_empty()
            || self
                .command_any
                .iter()
                .any(|expected| matches_prefix(std::slice::from_ref(expected)).routes_protected());
        let contains_match =
            self.command_contains_any.is_empty() || self.command_contains_any.matches(command);
        let prefix_match = self.argv_prefix_any.is_empty()
            || self
                .argv_prefix_any
                .iter()
                .any(|prefix| matches_prefix(prefix).routes_protected());
        token_match && prefix_match && contains_match
    }

    fn matches_path(&self, paths: &[String]) -> bool {
        if self.path_any.is_empty() && self.path_glob_any.is_empty() {
            return true;
        }
        let exact_match = !self.path_any.is_empty()
            && paths.iter().any(|path| {
                self.path_any
                    .iter()
                    .any(|expected| path == expected || path.ends_with(expected))
            });
        let glob_match = paths.iter().any(|path| self.path_glob_any.matches(path));
        exact_match || glob_match
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
        if crate::match_policy_conformance::synthetic_match_environment_active()
            && matches!(path, "package.json" | "Cargo.toml")
        {
            return self.matches_configured_document_format(candidate);
        }
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

    fn matches_configured_document_format(&self, candidate: &std::path::Path) -> bool {
        if self.argv_structured_document_file && structured_document_format(candidate).is_none() {
            return false;
        }
        self.matches_structured_projection_format(candidate)
    }

    fn matches_structured_projection_format(&self, candidate: &std::path::Path) -> bool {
        let Some(projection) = self.structured_projection.as_ref() else {
            return true;
        };
        let format = structured_document_format(candidate);
        format.is_some_and(|format| format == projection.config.document_format)
    }
}

fn structured_document_format(
    candidate: &std::path::Path,
) -> Option<agent_semantic_config::HookClientStructuredFormat> {
    candidate
        .extension()
        .and_then(|extension| extension.to_str())
        .and_then(|extension| match extension.to_ascii_lowercase().as_str() {
            "json" => Some(agent_semantic_config::HookClientStructuredFormat::Json),
            "toml" => Some(agent_semantic_config::HookClientStructuredFormat::Toml),
            _ => None,
        })
}

fn fast_path_token(token: &str) -> Option<&str> {
    if token.starts_with('-') {
        return None;
    }
    let trimmed = token.trim_matches(|ch| matches!(ch, '"' | '\'' | ',' | ';'));
    let path = trimmed.strip_prefix("file://").unwrap_or(trimmed);
    path.contains('.').then_some(path)
}

fn path_without_line_range(path: &str) -> Option<&str> {
    let (base, suffix) = path.rsplit_once(':')?;
    if suffix.chars().all(|character| character.is_ascii_digit()) {
        let (base, start) = base.rsplit_once(':')?;
        return start
            .chars()
            .all(|character| character.is_ascii_digit())
            .then_some(base);
    }
    let (start, end) = suffix.split_once('-')?;
    (!start.is_empty()
        && !end.is_empty()
        && start.chars().all(|character| character.is_ascii_digit())
        && end.chars().all(|character| character.is_ascii_digit()))
    .then_some(base)
}

impl RuleRoute {
    fn decision_route(&self, runtime: &HookRuntime) -> DecisionRoute {
        let provider = runtime
            .providers
            .iter()
            .find(|provider| provider.provider_id == self.provider_id);
        DecisionRoute {
            language_id: self.language_id.clone(),
            provider_id: self.provider_id.clone(),
            binary: self
                .binary
                .clone()
                .or_else(|| provider.map(|provider| provider.binary.clone()))
                .unwrap_or_default(),
            kind: self.kind,
            argv: self.argv.clone(),
            stdin_mode: self.stdin_mode,
        }
    }
}

impl TryFrom<HookClientRuleConfig> for CompiledHookRule {
    type Error = String;

    fn try_from(config: HookClientRuleConfig) -> Result<Self, Self::Error> {
        let agents = agent_semantic_config::HookClientAgentsConfig::default();
        Self::try_from_with_agents(
            config,
            &agents,
            &[],
            &[],
            agent_semantic_config::WrapperMatchMode::default(),
        )
    }
}

impl CompiledHookRule {
    pub(in crate::hook_config) fn try_from_with_agents(
        config: HookClientRuleConfig,
        agents: &agent_semantic_config::HookClientAgentsConfig,
        command_profiles: &[agent_semantic_config::HookClientCommandProfileConfig],
        action_policies: &[agent_semantic_config::HookClientActionPolicyConfig],
        wrapper_match: agent_semantic_config::WrapperMatchMode,
    ) -> Result<Self, String> {
        Self::try_from_with_agents_and_matcher(
            config,
            agents,
            command_profiles,
            action_policies,
            wrapper_match,
            None,
            None,
        )
    }

    fn try_from_with_agents_and_matcher(
        config: HookClientRuleConfig,
        agents: &agent_semantic_config::HookClientAgentsConfig,
        command_profiles: &[agent_semantic_config::HookClientCommandProfileConfig],
        action_policies: &[agent_semantic_config::HookClientActionPolicyConfig],
        wrapper_match: agent_semantic_config::WrapperMatchMode,
        durable_matcher: Option<DurableRuleMatcherArtifact>,
        executable_capabilities: Option<&std::collections::BTreeSet<String>>,
    ) -> Result<Self, String> {
        let dispatch = config
            .dispatch
            .map(|dispatch| {
                let resident_name =
                    agents
                        .placeholders
                        .get(dispatch.role.as_str())
                        .ok_or_else(|| {
                            format!(
                                "rule `{}` dispatch references unavailable agent placeholder `{}`",
                                config.id,
                                dispatch.role.as_str()
                            )
                        })?;
                let resident = agents
                    .resident_agents
                    .iter()
                    .find(|resident| resident.enabled && resident.name == *resident_name)
                    .ok_or_else(|| {
                        format!(
                            "rule `{}` dispatch references unavailable resident `{}`",
                            config.id, resident_name
                        )
                    })?;
                Ok::<CompiledRuleDispatch, String>(CompiledRuleDispatch {
                    transport: dispatch.transport,
                    resident_name: resident_name.clone(),
                    resident_codex_agent_name: resident.codex_agent_name.clone(),
                    resident_role: resident.role.clone(),
                    resident_agent_kind: resident.agent_kind.clone(),
                    resident_display_role: if resident.display_role.is_empty() {
                        resident.role.clone()
                    } else {
                        resident.display_role.clone()
                    },
                    resident_description: resident.description.clone(),
                    receipt_kind: dispatch.receipt_kind.as_str().to_owned(),
                    lazy_provider: dispatch.lazy_provider,
                })
            })
            .transpose()?;
        let reason_kind = config
            .reason_kind
            .map(ReasonKind::from)
            .unwrap_or(ReasonKind::None);
        let typed_action_contract = !config.match_config.action_any.is_empty()
            || !config.match_config.effect_any.is_empty()
            || !config.match_config.subject_kind_any.is_empty()
            || !config.match_config.authority_any.is_empty()
            || !config.match_config.authority_exclude_any.is_empty()
            || !config.match_config.authority_rules.is_empty()
            || !config.match_config.effect_rules.is_empty()
            || !config.match_config.action_policy_all.is_empty()
            || !config.match_config.action_policy_any.is_empty()
            || !config.match_config.action_policy_none.is_empty();
        if typed_action_contract
            && matches!(
                reason_kind,
                ReasonKind::DirectSourceRead | ReasonKind::BulkSourceDump
            )
        {
            let referenced_effects = config
                .match_config
                .action_policy_all
                .iter()
                .filter_map(|reference| {
                    action_policies
                        .iter()
                        .find(|policy| policy.id == *reference)
                })
                .flat_map(|policy| policy.effect_any.iter().copied())
                .collect::<Vec<_>>();
            let effect_any = if config.match_config.effect_any.is_empty() {
                referenced_effects.as_slice()
            } else {
                config.match_config.effect_any.as_slice()
            };
            let includes_read =
                effect_any.contains(&agent_semantic_config::HookClientActionKind::Read);
            let effects_are_typed_read = effect_any
                .iter()
                .all(|effect| *effect == agent_semantic_config::HookClientActionKind::Read);
            if !includes_read || !effects_are_typed_read {
                return Err(format!(
                    "rule `{}` expands registered source without an exact typed read effect contract",
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
        let resolve_action_policies = |axis: &str, references: &mut Vec<String>| {
            std::mem::take(references)
                .into_iter()
                .map(|reference| {
                    action_policies
                        .iter()
                        .find(|policy| policy.id == reference)
                        .cloned()
                        .ok_or_else(|| {
                            format!(
                                "rule `{}` {axis} references unknown action policy `{reference}`",
                                config.id
                            )
                        })
                })
                .collect::<Result<Vec<_>, String>>()
        };
        let policy_all =
            resolve_action_policies("actionPolicyAll", &mut raw_match_config.action_policy_all)?;
        let policy_any =
            resolve_action_policies("actionPolicyAny", &mut raw_match_config.action_policy_any)?;
        let policy_none =
            resolve_action_policies("actionPolicyNone", &mut raw_match_config.action_policy_none)?;
        let mut match_config = RuleMatch::try_from_config(
            raw_match_config,
            durable_matcher,
            ResolvedActionPolicies {
                all: policy_all,
                any: policy_any,
                none: policy_none,
            },
            executable_capabilities,
        )?;
        match_config.wrapper_match = wrapper_match;
        Ok(Self {
            id: config.id,
            priority: config.priority,
            intent: config.intent,
            fields: config.fields,
            dispatch,
            decision: config.decision,
            decision_materializer: config.decision_materializer,
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
        Self::try_from_config(config, None, ResolvedActionPolicies::default(), None)
    }
}

impl TryFrom<HookClientRuleRouteConfig> for RuleRoute {
    type Error = String;

    fn try_from(config: HookClientRuleRouteConfig) -> Result<Self, Self::Error> {
        let provider_id = agent_semantic_config::ProviderId::try_new(config.provider_id)?;
        let manifest = crate::provider_manifest::provider_manifests()
            .into_iter()
            .find(|manifest| manifest.provider_id == provider_id)
            .ok_or_else(|| {
                format!("hook route references unregistered provider `{provider_id}`")
            })?;
        let language_id = config
            .language_id
            .map(agent_semantic_config::LanguageId::try_new)
            .transpose()?
            .unwrap_or_else(|| manifest.language_id.clone());
        if language_id != manifest.language_id {
            return Err(format!(
                "hook route provider `{provider_id}` belongs to language `{}`, not `{language_id}`",
                manifest.language_id
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

impl From<HookClientConfigReasonKind> for ReasonKind {
    fn from(kind: HookClientConfigReasonKind) -> Self {
        match kind {
            HookClientConfigReasonKind::None => Self::None,
            HookClientConfigReasonKind::DirectSourceRead => Self::DirectSourceRead,
            HookClientConfigReasonKind::StructuredSourceRead => Self::StructuredSourceRead,
            HookClientConfigReasonKind::BulkSourceDump => Self::BulkSourceDump,
            HookClientConfigReasonKind::RawBroadSearch => Self::RawBroadSearch,
            HookClientConfigReasonKind::AgentSearchJson => Self::AgentSearchJson,
            HookClientConfigReasonKind::SubagentReceiptRequired => Self::SubagentReceiptRequired,
        }
    }
}

impl From<HookClientConfigRouteKind> for DecisionRouteKind {
    fn from(kind: HookClientConfigRouteKind) -> Self {
        match kind {
            HookClientConfigRouteKind::Prime => Self::Prime,
            HookClientConfigRouteKind::Owner => Self::Owner,
            HookClientConfigRouteKind::Query => Self::Query,
            HookClientConfigRouteKind::Lexical => Self::Lexical,
            HookClientConfigRouteKind::Read => Self::Read,
            HookClientConfigRouteKind::Deps => Self::Deps,
            HookClientConfigRouteKind::Api => Self::Api,
            HookClientConfigRouteKind::Ingest => Self::Ingest,
            HookClientConfigRouteKind::Tests => Self::Tests,
            HookClientConfigRouteKind::CheckChanged => Self::CheckChanged,
        }
    }
}

impl From<HookClientConfigStdinMode> for StdinMode {
    fn from(mode: HookClientConfigStdinMode) -> Self {
        match mode {
            HookClientConfigStdinMode::None => Self::None,
            HookClientConfigStdinMode::PipeCandidates => Self::PipeCandidates,
            HookClientConfigStdinMode::PipeDiff => Self::PipeDiff,
            HookClientConfigStdinMode::Unknown => Self::Unknown,
        }
    }
}

#[path = "compiled_rule_client_config.rs"]
mod client_config;

#[path = "compiled_rule_durable_artifact.rs"]
mod durable_artifact;

pub(in crate::hook_config) use client_config::{
    compile_config, compile_config_with_executable_capabilities,
};
pub use durable_artifact::DurableHookConfigArtifact;
use durable_artifact::{
    DURABLE_HOOK_MATCHER_SCHEMA_ID, DURABLE_HOOK_MATCHER_SCHEMA_VERSION, DurableRuleMatcherArtifact,
};

use crate::hook_config::core::match_types::{CompiledCommandContains, CompiledPathGlobs};
use crate::hook_config::core::{compile_command_contains, compile_globs};

fn canonical_event(value: &str) -> String {
    value.to_ascii_lowercase().replace('_', "-")
}
#[path = "compiled_rule_message.rs"]
mod compiled_rule_message;
