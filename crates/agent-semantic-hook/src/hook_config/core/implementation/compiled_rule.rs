use super::action_match;

use agent_semantic_config::{
    HookClientConfigDecision, HookClientConfigFile, HookClientConfigReasonKind,
    HookClientConfigRouteKind, HookClientConfigStdinMode, HookClientRuleConfig,
    HookClientRuleMatchConfig, HookClientRuleRouteConfig,
};

use crate::hook_config::AspSessionPolicy;
use crate::hook_config::agent_org_config::compile_agent_org_artifacts_config;
use crate::hook_config_agent_org::{
    AgentOrgArtifactsArchiveWarning, AgentOrgArtifactsRecovery, CompiledAgentOrgArtifactsConfig,
};
use crate::hook_recovery_prompt::CompiledRecoveryPromptConfig;
use crate::protocol::{
    DecisionKind, DecisionRoute, DecisionRouteKind, HOOK_DECISION_SCHEMA_ID,
    HOOK_DECISION_SCHEMA_VERSION, HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, HookDecision,
    ReasonKind, StdinMode,
};

use crate::protocol_activation::protocol_activation_manifest::HookRuntime;
use crate::source_selector::collect_source_selector_matches;
use crate::tool_action::{ToolAction, subject_for_action};

#[derive(Debug)]
/// Compiled hook rules loaded from the global ASP state root.
pub struct ClientHookConfig {
    pub(in crate::hook_config) rules: Vec<CompiledHookRule>,
    contract_fingerprint: Option<String>,
    semantic_ast_patch_disabled: bool,
    agent_org_artifacts: CompiledAgentOrgArtifactsConfig,
    recovery_prompt: CompiledRecoveryPromptConfig,
    asp_session_policy: AspSessionPolicy,
    agent_session_messages: agent_semantic_config::HookClientAgentSessionMessagesConfig,
}

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
    language_ids: Vec<String>,
    event: Option<String>,
    platform: Option<String>,
    match_config: RuleMatch,
    routes: Vec<RuleRoute>,
}

#[derive(Debug)]
pub(in crate::hook_config) struct CompiledRuleDispatch {
    transport: agent_semantic_config::HookClientRuleDispatchTransport,
    pub(in crate::hook_config) resident_name: String,
    pub(in crate::hook_config) resident_codex_agent_name: String,
    pub(in crate::hook_config) resident_role: String,
    receipt_kind: String,
    lazy_provider: Option<agent_semantic_config::HookClientLazyProviderPolicy>,
}

#[derive(Debug)]
pub(super) struct RuleMatch {
    agent_action: action_match::AgentActionMatch,
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
    pub(super) argv_registered_source_file: bool,
    structured_projection: Option<agent_semantic_config::HookClientStructuredProjectionMatchConfig>,
}

#[derive(Debug)]
struct RuleRoute {
    provider_id: String,
    language_id: Option<String>,
    binary: Option<String>,
    kind: DecisionRouteKind,
    argv: Vec<String>,
    stdin_mode: Option<StdinMode>,
}

impl CompiledHookRule {
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
    ) -> Option<serde_json::Value> {
        self.match_config
            .agent_action
            .derive_agent_action_for_rule(runtime, action, Some(paths))
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
                .any(|language_id| language_id.eq_ignore_ascii_case(&provider.language_id))
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
    ) -> HookDecision {
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
        let message = self.message.clone().unwrap_or_else(|| {
            format!(
                "client hook config rule `{}` matched this tool use",
                self.id
            )
        });
        let mut subject = subject_for_action(action);
        subject.paths = paths.to_vec();
        let mut decision_fields = self
            .fields
            .iter()
            .map(|(key, value)| (key.clone(), serde_json::Value::String(value.clone())))
            .collect::<std::collections::BTreeMap<_, _>>();
        if let Some(agent_action) = self.agent_action_receipt(runtime, action, paths) {
            decision_fields.insert("agentAction".to_string(), agent_action);
        }
        if let Some(dispatch) = self.dispatch.as_ref() {
            decision_fields.insert(
                "transport".to_string(),
                serde_json::Value::String(dispatch.transport.as_str().to_string()),
            );
            decision_fields.insert(
                "residentName".to_string(),
                serde_json::Value::String(dispatch.resident_name.clone()),
            );
            decision_fields.insert(
                "residentChildName".to_string(),
                serde_json::Value::String(dispatch.resident_name.clone()),
            );
            decision_fields.insert(
                "targetAgentName".to_string(),
                serde_json::Value::String(dispatch.resident_codex_agent_name.clone()),
            );
            decision_fields.insert(
                "targetAgentRole".to_string(),
                serde_json::Value::String(dispatch.resident_role.clone()),
            );
            decision_fields.insert(
                "agentSessionAction".to_string(),
                serde_json::Value::String("dispatch-configured-resident".to_string()),
            );
            decision_fields.insert(
                "receiptKind".to_string(),
                serde_json::Value::String(dispatch.receipt_kind.clone()),
            );
            if let Some(command) = action.command.as_deref() {
                use sha2::{Digest, Sha256};
                decision_fields.insert(
                    "commandDigest".to_string(),
                    serde_json::Value::String(format!(
                        "sha256:{:x}",
                        Sha256::digest(command.as_bytes())
                    )),
                );
            }
            decision_fields.insert(
                "canonicalTarget".to_string(),
                serde_json::Value::String(format!("/root/{}", dispatch.resident_codex_agent_name)),
            );
            decision_fields.insert(
                "requiredAction".to_string(),
                serde_json::Value::String(
                    "route-exact-command-to-hook-selected-resident".to_string(),
                ),
            );
        }
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
                runtime,
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
            && self.matches_structured_projection(action)
            && (self.argv_pattern_any.is_empty()
                || crate::hook_config::core::registered_asp::match_registered_asp_command(
                    &self.argv_pattern_any,
                    runtime,
                    action,
                )
                .is_some())
    }

    pub(in crate::hook_config) fn matches_structured_projection(
        &self,
        action: &ToolAction,
    ) -> bool {
        crate::hook_config::core::structured_projection::matches(
            self.structured_projection.as_ref(),
            action,
        )
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
            || self.argv_registered_source_file
    }

    fn needs_argv_source_match(&self) -> bool {
        !self.argv_source_any.is_empty()
            || !self.argv_source_glob_any.is_empty()
            || self.argv_workspace_regular_file
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
        let command_stages =
            match crate::command_match::bash::parse_bash_command_candidates(command) {
                Ok(stages) => stages,
                Err(_) => return true,
            };
        let _ = command_tokens;
        let token_match = self.command_any.is_empty()
            || self.command_any.iter().any(|expected| {
                crate::command_match::command_stages_match_prefix(
                    &command_stages,
                    std::slice::from_ref(expected),
                )
                .routes_protected()
            });
        let contains_match =
            self.command_contains_any.is_empty() || self.command_contains_any.matches(command);
        let prefix_match = self.argv_prefix_any.is_empty()
            || self.argv_prefix_any.iter().any(|prefix| {
                crate::command_match::command_stages_match_prefix(&command_stages, prefix)
                    .routes_protected()
            });
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
        if !self.argv_workspace_regular_file {
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
        let Some(projection) = self.structured_projection.as_ref() else {
            return true;
        };
        let format = candidate
            .extension()
            .and_then(|extension| extension.to_str())
            .and_then(|extension| match extension.to_ascii_lowercase().as_str() {
                "json" => Some(agent_semantic_config::HookClientStructuredFormat::Json),
                "toml" => Some(agent_semantic_config::HookClientStructuredFormat::Toml),
                _ => None,
            });
        format.is_some_and(|format| format == projection.document_format)
    }
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
            language_id: self
                .language_id
                .clone()
                .or_else(|| provider.map(|provider| provider.language_id.clone()))
                .unwrap_or_default(),
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

fn merge_agent_session_messages(
    mut config: agent_semantic_config::HookClientAgentSessionMessagesConfig,
    defaults: agent_semantic_config::HookClientAgentSessionMessagesConfig,
) -> agent_semantic_config::HookClientAgentSessionMessagesConfig {
    if config.session_start_reuse.is_none() {
        config.session_start_reuse = defaults.session_start_reuse;
    }
    if config.session_start_bootstrap.is_none() {
        config.session_start_bootstrap = defaults.session_start_bootstrap;
    }
    if config.missing_resident_explore.is_none() {
        config.missing_resident_explore = defaults.missing_resident_explore;
    }
    if config.main_restricted_with_child.is_none() {
        config.main_restricted_with_child = defaults.main_restricted_with_child;
    }
    if config.main_restricted_without_child.is_none() {
        config.main_restricted_without_child = defaults.main_restricted_without_child;
    }
    if config.binary_gate_with_child.is_none() {
        config.binary_gate_with_child = defaults.binary_gate_with_child;
    }
    if config.binary_gate_without_child.is_none() {
        config.binary_gate_without_child = defaults.binary_gate_without_child;
    }
    if config.binary_gate_invalid_child.is_none() {
        config.binary_gate_invalid_child = defaults.binary_gate_invalid_child;
    }
    if config.binary_gate_registry_blocked.is_none() {
        config.binary_gate_registry_blocked = defaults.binary_gate_registry_blocked;
    }
    if config.source_access_compact.is_none() {
        config.source_access_compact = defaults.source_access_compact;
    }
    if config.source_access_compact_repeated.is_none() {
        config.source_access_compact_repeated = defaults.source_access_compact_repeated;
    }
    if config.source_access_compact_subagent.is_none() {
        config.source_access_compact_subagent = defaults.source_access_compact_subagent;
    }
    config
}

impl TryFrom<HookClientRuleConfig> for CompiledHookRule {
    type Error = String;

    fn try_from(config: HookClientRuleConfig) -> Result<Self, Self::Error> {
        Self::try_from_with_agents(config, &[])
    }
}

impl CompiledHookRule {
    pub(in crate::hook_config) fn try_from_with_agents(
        config: HookClientRuleConfig,
        resident_agents: &[agent_semantic_config::HookClientResidentAgentConfig],
    ) -> Result<Self, String> {
        let dispatch = config
            .dispatch
            .map(|dispatch| {
                let resident = resident_agents
                    .iter()
                    .find(|resident| resident.enabled && resident.name == dispatch.resident_name)
                    .ok_or_else(|| {
                        format!(
                            "rule `{}` dispatch references unavailable resident `{}`",
                            config.id, dispatch.resident_name
                        )
                    })?;
                Ok::<CompiledRuleDispatch, String>(CompiledRuleDispatch {
                    transport: dispatch.transport,
                    resident_name: dispatch.resident_name,
                    resident_codex_agent_name: resident.codex_agent_name.clone(),
                    resident_role: resident.role.clone(),
                    receipt_kind: dispatch.receipt_kind,
                    lazy_provider: dispatch.lazy_provider,
                })
            })
            .transpose()?;
        let reason_kind = config
            .reason_kind
            .map(ReasonKind::from)
            .unwrap_or(ReasonKind::None);
        let typed_action_contract = !config.match_config.command_wrappers.is_empty()
            || !config.match_config.invocation_shape_any.is_empty()
            || !config.match_config.wrapper_match_any.is_empty()
            || !config.match_config.flag_presence_any.is_empty()
            || !config.match_config.action_any.is_empty()
            || !config.match_config.effect_any.is_empty()
            || !config.match_config.subject_kind_any.is_empty()
            || !config.match_config.authority_any.is_empty()
            || !config.match_config.authority_exclude_any.is_empty()
            || !config.match_config.authority_rules.is_empty()
            || !config.match_config.effect_rules.is_empty();
        if typed_action_contract
            && matches!(
                reason_kind,
                ReasonKind::DirectSourceRead | ReasonKind::BulkSourceDump
            )
        {
            let effect_any = &config.match_config.effect_any;
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
            language_ids: config.language_ids,
            event: config.event,
            platform: config.platform,
            match_config: RuleMatch::try_from(config.match_config)?,
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
        let mut tool_any = config.tool_any;
        if let Some(tool) = config.tool {
            tool_any.push(tool);
        }
        Ok(Self {
            agent_action: action_match::AgentActionMatch::new(
                config.command_wrappers,
                config.invocation_shape_any,
                config.wrapper_match_any,
                config.flag_presence_any,
                config.action_any,
                config.effect_any,
                config.subject_kind_any,
                config.authority_any,
                config.authority_exclude_any,
                config.authority_rules,
                config.effect_rules,
            ),
            tool_any,
            command_any: config.command_any,
            argv_pattern_any: config.argv_pattern_any,
            argv_prefix_any: config.argv_prefix_any,
            command_contains_any: compile_command_contains(config.command_contains_any)?,
            path_any: config.path_any,
            path_glob_any: compile_globs("pathGlobAny", config.path_glob_any)?,
            argv_source_any: config.argv_source_any,
            argv_source_glob_any: compile_globs("argvSourceGlobAny", config.argv_source_glob_any)?,
            argv_source_exclude_flag_any: config.argv_source_exclude_flag_any,
            argv_workspace_regular_file: config.argv_workspace_regular_file,
            argv_registered_source_file: config.argv_registered_source_file,
            structured_projection: config.structured_projection,
        })
    }
}

impl TryFrom<HookClientRuleRouteConfig> for RuleRoute {
    type Error = String;

    fn try_from(config: HookClientRuleRouteConfig) -> Result<Self, Self::Error> {
        Ok(Self {
            provider_id: config.provider_id,
            language_id: config.language_id,
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

pub(in crate::hook_config) use client_config::compile_config;

use crate::hook_config::core::compile::{compile_command_contains, compile_globs};
use crate::hook_config::core::match_types::{CompiledCommandContains, CompiledPathGlobs};

fn canonical_event(value: &str) -> String {
    value.to_ascii_lowercase().replace('_', "-")
}
