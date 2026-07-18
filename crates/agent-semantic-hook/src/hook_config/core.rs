use agent_semantic_config::{
    HookClientConfigDecision, HookClientConfigFile, HookClientConfigReasonKind,
    HookClientConfigRouteKind, HookClientConfigStdinMode, HookClientRuleConfig,
    HookClientRuleMatchConfig, HookClientRuleRouteConfig, default_hook_client_config_template,
    default_hook_client_config_template_for_source_extensions, load_asp_project_config_file,
    load_hook_client_config_file,
};
use aho_corasick::{AhoCorasick, AhoCorasickBuilder, MatchKind};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use std::borrow::Cow;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::command::path_like_token_matches;
use crate::hook_config::AspSessionPolicy;
use crate::hook_config::agent_org_config::compile_agent_org_artifacts_config;
use crate::hook_config_agent_org::{
    AgentOrgArtifactsArchiveWarning, AgentOrgArtifactsRecovery, CompiledAgentOrgArtifactsConfig,
};
use crate::hook_config_global::default_global_client_config_path;
use crate::hook_recovery_prompt::CompiledRecoveryPromptConfig;
use crate::protocol::{
    DecisionKind, DecisionRoute, DecisionRouteKind, HOOK_DECISION_SCHEMA_ID,
    HOOK_DECISION_SCHEMA_VERSION, HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, HookDecision,
    ReasonKind, StdinMode,
};

use crate::protocol_activation::HookRuntime;
use crate::provider_manifest::project_agent_config_path;
use crate::source_selector::collect_source_selector_matches;
use crate::tool_action::{ToolAction, subject_for_action};

#[derive(Debug)]
/// Compiled hook rules loaded from the global ASP state root.
pub struct ClientHookConfig {
    rules: Vec<CompiledHookRule>,
    contract_fingerprint: Option<String>,
    asp_command_intent_policy: agent_semantic_config::HookClientAspCommandIntentPolicyConfig,
    semantic_ast_patch_disabled: bool,
    agent_org_artifacts: CompiledAgentOrgArtifactsConfig,
    recovery_prompt: CompiledRecoveryPromptConfig,
    asp_session_policy: AspSessionPolicy,
    agent_session_messages: agent_semantic_config::HookClientAgentSessionMessagesConfig,
}

impl Default for ClientHookConfig {
    fn default() -> Self {
        let config = agent_semantic_config::default_hook_client_config_file()
            .expect("embedded hook client config must remain valid");
        compile_config(config).expect("embedded hook client rules must compile")
    }
}

impl ClientHookConfig {
    /// Configured parser-owned taxonomy for public ASP language commands.
    pub fn asp_command_intent_policy(
        &self,
    ) -> &agent_semantic_config::HookClientAspCommandIntentPolicyConfig {
        &self.asp_command_intent_policy
    }

    /// Return the agent-facing session message templates.
    pub fn agent_session_messages(
        &self,
    ) -> &agent_semantic_config::HookClientAgentSessionMessagesConfig {
        &self.agent_session_messages
    }

    /// Return the resident child session name used for ASP exploration.
    pub fn resident_asp_explore_child_name(&self) -> &str {
        self.asp_session_policy.resident_child_name()
    }

    /// Return the configured Codex agent name used for ASP exploration.
    pub fn resident_asp_explore_codex_agent_name(&self) -> &str {
        self.asp_session_policy.resident_codex_agent_name()
    }
}

#[derive(Debug)]
struct CompiledHookRule {
    id: String,
    priority: i64,
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
struct RuleMatch {
    tool_any: Vec<String>,
    command_any: Vec<String>,
    argv_prefix_any: Vec<Vec<String>>,
    command_contains_any: CompiledCommandContains,
    path_any: Vec<String>,
    path_glob_any: CompiledPathGlobs,
    argv_source_any: Vec<String>,
    argv_source_glob_any: CompiledPathGlobs,
    argv_source_exclude_flag_any: Vec<String>,
}

#[derive(Debug, Default)]
struct CompiledCommandContains {
    matcher: Option<AhoCorasick>,
}

impl CompiledCommandContains {
    fn is_empty(&self) -> bool {
        self.matcher.is_none()
    }

    fn matches(&self, command: &str) -> bool {
        self.matcher
            .as_ref()
            .is_some_and(|matcher| matcher.is_match(command))
    }
}

#[derive(Debug, Default)]
struct CompiledPathGlobs {
    suffix_ext_any: HashSet<String>,
    suffix_any: Vec<String>,
    globset: Option<GlobSet>,
}

impl CompiledPathGlobs {
    fn is_empty(&self) -> bool {
        self.suffix_ext_any.is_empty() && self.suffix_any.is_empty() && self.globset.is_none()
    }

    fn is_suffix_only(&self) -> bool {
        (!self.suffix_ext_any.is_empty() || !self.suffix_any.is_empty()) && self.globset.is_none()
    }

    fn matches(&self, path: &str) -> bool {
        self.matches_suffix_extension(path)
            || self.suffix_any.iter().any(|suffix| path.ends_with(suffix))
            || self
                .globset
                .as_ref()
                .is_some_and(|globset| globset.is_match(path))
    }

    fn matches_suffix_extension(&self, path: &str) -> bool {
        let Some(dot_index) = path.rfind('.') else {
            return false;
        };
        self.suffix_ext_any.contains(&path[dot_index..])
    }
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

/// Return the default global hook config path.
pub fn default_client_config_path(_project_root: &str) -> PathBuf {
    default_global_client_config_path()
        .unwrap_or_else(|| PathBuf::from(".agent-semantic-protocols/hooks/config.toml"))
}

/// Render the seed global hook config file.
pub fn default_client_config_template() -> String {
    default_hook_client_config_template()
}

/// Render the seed global hook config file for active provider source extensions.
pub fn default_client_config_template_for_source_extensions<I, S>(source_extensions: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    default_hook_client_config_template_for_source_extensions(source_extensions)
}

/// Load and compile hook config rules.
pub fn load_client_config(path: &Path) -> Result<ClientHookConfig, String> {
    let parsed = load_hook_client_config_file(path)?;
    compile_config(parsed)
}

/// Load optional user hook config and validate hook-owned project config fields.
pub fn load_client_config_for_project(
    path: &Path,
    project_root: &Path,
) -> Result<ClientHookConfig, String> {
    let parsed = if path.is_file() {
        load_hook_client_config_file(path)?
    } else {
        agent_semantic_config::default_hook_client_config_file()?
    };
    let agent_config_path = project_agent_config_path(project_root);
    load_asp_project_config_file(&agent_config_path)?;
    compile_config(parsed)
}

impl ClientHookConfig {
    pub fn contract_fingerprint(&self) -> Option<&str> {
        self.contract_fingerprint.as_deref()
    }

    pub(crate) fn semantic_ast_patch_enabled(&self) -> bool {
        !self.semantic_ast_patch_disabled
    }

    /// Return ASP session routing policy compiled from hook config.
    pub fn asp_session_policy(&self) -> &AspSessionPolicy {
        &self.asp_session_policy
    }

    pub(crate) fn recovery_prompt(&self) -> &CompiledRecoveryPromptConfig {
        &self.recovery_prompt
    }

    pub(crate) fn agent_org_artifacts_recovery(
        &self,
        project_root: impl AsRef<Path>,
        session_id: Option<&str>,
    ) -> Option<AgentOrgArtifactsRecovery> {
        self.agent_org_artifacts
            .recovery(project_root.as_ref(), session_id)
    }

    pub(crate) fn agent_org_artifacts_archive_warning(
        &self,
        project_root: impl AsRef<Path>,
    ) -> Option<AgentOrgArtifactsArchiveWarning> {
        self.agent_org_artifacts
            .archive_warning(project_root.as_ref())
    }

    pub(crate) fn classify(
        &self,
        runtime: &HookRuntime,
        platform: &str,
        event: &str,
        payload: &serde_json::Value,
        action: &ToolAction,
    ) -> Option<HookDecision> {
        let mut command_tokens: Option<Option<Cow<'_, [String]>>> = None;
        let mut effective_paths: Option<Vec<String>> = None;
        for rule in &self.rules {
            let needs_command_tokens = rule.match_config.needs_command_tokens()
                || rule.needs_match_paths()
                || matches!(
                    rule.decision_materializer,
                    Some(
                        agent_semantic_config::HookClientDecisionMaterializer::AgentSearchJson
                            | agent_semantic_config::HookClientDecisionMaterializer::PromptSearchStrategy
                            | agent_semantic_config::HookClientDecisionMaterializer::SourceAccess
                    )
                );
            let command_token_slice = if needs_command_tokens {
                command_tokens
                    .get_or_insert_with(|| action.command_tokens())
                    .as_deref()
            } else {
                None
            };
            if !rule.matches_before_paths(platform, event, action, command_token_slice) {
                continue;
            }
            let match_paths = if rule.needs_match_paths() {
                effective_paths
                    .get_or_insert_with(|| derive_effective_paths(action, command_token_slice))
                    .as_slice()
            } else {
                action.paths.as_slice()
            };
            if !rule.matches_after_paths(runtime, match_paths) {
                continue;
            }
            let argv_source_paths = if rule.match_config.needs_argv_source_match() {
                let Some(paths) = rule
                    .match_config
                    .matching_argv_source_paths(command_token_slice)
                else {
                    continue;
                };
                Some(paths)
            } else {
                None
            };
            let decision_paths = if rule.needs_decision_paths() {
                if let Some(paths) = argv_source_paths.as_deref()
                    && !rule.match_config.needs_path_match()
                {
                    paths
                } else {
                    effective_paths
                        .get_or_insert_with(|| derive_effective_paths(action, command_token_slice))
                        .as_slice()
                }
            } else {
                action.paths.as_slice()
            };
            if let Some(materializer) = rule.decision_materializer {
                let decision = match materializer {
                    agent_semantic_config::HookClientDecisionMaterializer::AgentSearchJson => {
                        command_token_slice.and_then(|tokens| {
                            crate::classifier::materialize_agent_search_json_decision(
                                runtime, platform, event, action, tokens,
                            )
                        })
                    }
                    agent_semantic_config::HookClientDecisionMaterializer::PromptSearchStrategy => {
                        crate::classifier::materialize_prompt_search_strategy_decision(
                            runtime,
                            platform,
                            event,
                            payload,
                            action,
                            self.asp_command_intent_policy(),
                        )
                    }
                    agent_semantic_config::HookClientDecisionMaterializer::ApplyPatch => {
                        crate::classifier::materialize_apply_patch_decision(
                            runtime,
                            platform,
                            event,
                            action,
                            self.semantic_ast_patch_enabled(),
                        )
                    }
                    agent_semantic_config::HookClientDecisionMaterializer::SourceAccess => {
                        crate::classifier::materialize_source_access_decision(
                            runtime,
                            platform,
                            event,
                            action,
                            command_token_slice,
                            self.semantic_ast_patch_enabled(),
                            self.recovery_prompt(),
                        )
                    }
                };
                if let Some(mut decision) = decision {
                    decision.fields.insert(
                        "configRuleId".to_string(),
                        serde_json::Value::String(rule.id.clone()),
                    );
                    return Some(decision);
                }
                continue;
            }
            return Some(rule.decision(runtime, platform, event, action, decision_paths));
        }
        None
    }
}

impl CompiledHookRule {
    fn needs_match_paths(&self) -> bool {
        self.match_config.needs_path_match()
            || (!self.language_ids.is_empty() && self.match_config.needs_source_paths())
    }

    fn needs_decision_paths(&self) -> bool {
        self.match_config.needs_source_paths()
    }

    fn matches_before_paths(
        &self,
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
            && self
                .match_config
                .matches_before_paths(action, command_tokens)
    }

    fn matches_after_paths(&self, runtime: &HookRuntime, paths: &[String]) -> bool {
        self.matches_language(runtime, paths) && self.match_config.matches_paths(paths)
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
        HookDecision {
            schema_id: HOOK_DECISION_SCHEMA_ID,
            schema_version: HOOK_DECISION_SCHEMA_VERSION,
            protocol_id: HOOK_PROTOCOL_ID,
            protocol_version: HOOK_PROTOCOL_VERSION,
            platform: platform.to_string(),
            event: event.to_string(),
            decision,
            reason_kind: self.reason_kind,
            language_ids: self.language_ids.clone(),
            subject,
            routes,
            message,
            fields: std::collections::BTreeMap::from([(
                "configRuleId".to_string(),
                serde_json::Value::String(self.id.clone()),
            )]),
        }
    }
}

impl RuleMatch {
    fn matches_before_paths(&self, action: &ToolAction, command_tokens: Option<&[String]>) -> bool {
        self.matches_tool(action) && self.matches_command(action, command_tokens)
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
    }

    fn needs_path_match(&self) -> bool {
        !self.path_any.is_empty() || !self.path_glob_any.is_empty()
    }

    fn needs_source_paths(&self) -> bool {
        self.needs_path_match()
            || !self.argv_source_any.is_empty()
            || !self.argv_source_glob_any.is_empty()
    }

    fn needs_argv_source_match(&self) -> bool {
        !self.argv_source_any.is_empty() || !self.argv_source_glob_any.is_empty()
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
        let token_match = self.command_any.is_empty()
            || self.command_any.iter().any(|expected| {
                command_tokens.is_some_and(|tokens| {
                    command_name_tokens(tokens).any(|token| {
                        token.eq_ignore_ascii_case(expected)
                            || command_token_basename(token).eq_ignore_ascii_case(expected)
                    })
                })
            });
        let contains_match =
            self.command_contains_any.is_empty() || self.command_contains_any.matches(command);
        let prefix_match = self.argv_prefix_any.is_empty()
            || command_tokens.is_some_and(|tokens| {
                self.argv_prefix_any
                    .iter()
                    .any(|prefix| command_stage_matches_argv_prefix(tokens, prefix))
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

    fn matching_argv_source_paths(&self, command_tokens: Option<&[String]>) -> Option<Vec<String>> {
        if self.argv_source_any.is_empty() && self.argv_source_glob_any.is_empty() {
            return Some(Vec::new());
        }
        let tokens = command_tokens?;

        let mut paths = Vec::new();
        let mut skip_next = false;
        let mut positional_only = false;
        for token in tokens {
            if skip_next {
                skip_next = false;
                continue;
            }
            if token == "--" {
                positional_only = true;
                continue;
            }
            if !positional_only
                && self
                    .argv_source_exclude_flag_any
                    .iter()
                    .any(|flag| token.as_str() == flag.as_str())
            {
                skip_next = true;
                continue;
            }
            if !positional_only
                && self.argv_source_exclude_flag_any.iter().any(|flag| {
                    token
                        .strip_prefix(flag.as_str())
                        .is_some_and(|suffix| suffix.starts_with('='))
                })
            {
                continue;
            }
            if self.argv_source_glob_any.is_suffix_only() {
                if let Some(path) = self.fast_argv_source_path(token)
                    && !paths.iter().any(|existing| existing == &path)
                {
                    paths.push(path);
                }
            } else {
                path_like_token_matches(token, |path| {
                    if self.matches_argv_source_path(path)
                        && !paths.iter().any(|existing| existing == path)
                    {
                        paths.push(path.to_string());
                    }
                    false
                });
            }
        }
        (!paths.is_empty()).then_some(paths)
    }

    fn fast_argv_source_path(&self, token: &str) -> Option<String> {
        let path = fast_path_token(token)?;
        if self.matches_argv_source_path(path) {
            return Some(path.to_string());
        }
        let base = path_without_line_range(path)?;
        self.matches_argv_source_path(base)
            .then(|| base.to_string())
    }

    fn matches_argv_source_path(&self, path: &str) -> bool {
        let exact_match = !self.argv_source_any.is_empty()
            && self
                .argv_source_any
                .iter()
                .any(|expected| path == expected || path.ends_with(expected));
        let glob_match = self.argv_source_glob_any.matches(path);
        exact_match || glob_match
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

fn derive_effective_paths(action: &ToolAction, command_tokens: Option<&[String]>) -> Vec<String> {
    let mut paths = action.paths.clone();
    if let Some(tokens) = command_tokens {
        for token in tokens {
            path_like_token_matches(token, |path| {
                if !paths.iter().any(|existing| existing == path) {
                    paths.push(path.to_string());
                }
                false
            });
        }
    }
    paths
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

fn compile_config(config: HookClientConfigFile) -> Result<ClientHookConfig, String> {
    let contract_fingerprint = config.contract_fingerprint.clone();
    let default_config = agent_semantic_config::default_hook_client_config_file()?;
    let default_agent_session_messages = default_config.agent_session_messages;
    let agent_session_messages = merge_agent_session_messages(
        config.agent_session_messages,
        default_agent_session_messages,
    );
    let semantic_ast_patch_enabled = config
        .experimental
        .get("semanticAstPatch")
        .and_then(|feature| feature.get("enabled"))
        .copied()
        .unwrap_or(true);
    let configured_rule_ids = config
        .rules
        .iter()
        .map(|rule| rule.id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let mut rule_configs = default_config
        .rules
        .into_iter()
        .filter(|rule| !configured_rule_ids.contains(rule.id.as_str()))
        .collect::<Vec<_>>();
    rule_configs.extend(config.rules);
    let mut rules = rule_configs
        .into_iter()
        .filter(|rule| rule.enabled)
        .map(CompiledHookRule::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    // `sort_by_key` is stable, so equal-priority rules keep config file order.
    rules.sort_by_key(|rule| std::cmp::Reverse(rule.priority));
    let asp_command_intent_policy = config.asp_command_intent_policy;
    Ok(ClientHookConfig {
        rules,
        contract_fingerprint,
        asp_command_intent_policy: asp_command_intent_policy.clone(),
        semantic_ast_patch_disabled: !semantic_ast_patch_enabled,
        agent_org_artifacts: compile_agent_org_artifacts_config(config.agent_org_artifacts)?,
        recovery_prompt: config.recovery_prompt.into(),
        agent_session_messages,
        asp_session_policy: AspSessionPolicy::try_from(config.agents)?
            .with_command_intent_policy(asp_command_intent_policy),
    })
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
        Ok(Self {
            id: config.id,
            priority: config.priority,
            decision: config.decision,
            decision_materializer: config.decision_materializer,
            reason_kind: config
                .reason_kind
                .map(ReasonKind::from)
                .unwrap_or(ReasonKind::None),
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
            tool_any,
            command_any: config.command_any,
            argv_prefix_any: config.argv_prefix_any,
            command_contains_any: compile_command_contains(config.command_contains_any)?,
            path_any: config.path_any,
            path_glob_any: compile_globs("pathGlobAny", config.path_glob_any)?,
            argv_source_any: config.argv_source_any,
            argv_source_glob_any: compile_globs("argvSourceGlobAny", config.argv_source_glob_any)?,
            argv_source_exclude_flag_any: config.argv_source_exclude_flag_any,
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

fn compile_globs(label: &str, patterns: Vec<String>) -> Result<CompiledPathGlobs, String> {
    if patterns.is_empty() {
        return Ok(CompiledPathGlobs::default());
    }
    let paired_suffixes = paired_suffix_globs(&patterns);
    let mut suffix_ext_any = HashSet::new();
    let mut suffix_any = Vec::new();
    for suffix in &paired_suffixes {
        if simple_extension_suffix(suffix) {
            suffix_ext_any.insert(suffix.clone());
        } else {
            suffix_any.push(suffix.clone());
        }
    }
    let mut builder = GlobSetBuilder::new();
    let mut glob_count = 0usize;
    for pattern in patterns {
        if simple_glob_suffix(&pattern)
            .is_some_and(|suffix| paired_suffixes.iter().any(|existing| existing == suffix))
        {
            continue;
        }
        let glob = GlobBuilder::new(&pattern)
            .literal_separator(true)
            .build()
            .map_err(|error| format!("invalid {label} pattern `{pattern}`: {error}"))?;
        builder.add(glob);
        glob_count += 1;
    }
    let globset = if glob_count == 0 {
        None
    } else {
        Some(
            builder
                .build()
                .map_err(|error| format!("failed to compile {label} patterns: {error}"))?,
        )
    };
    Ok(CompiledPathGlobs {
        suffix_ext_any,
        suffix_any,
        globset,
    })
}

fn paired_suffix_globs(patterns: &[String]) -> Vec<String> {
    let mut suffixes = Vec::new();
    for pattern in patterns {
        let Some(suffix) = simple_glob_suffix(pattern) else {
            continue;
        };
        let has_pair = patterns.iter().any(|candidate| {
            candidate != pattern
                && simple_glob_suffix(candidate).is_some_and(|other| other == suffix)
        });
        if has_pair && !suffixes.iter().any(|existing| existing == suffix) {
            suffixes.push(suffix.to_string());
        }
    }
    suffixes
}

fn simple_glob_suffix(pattern: &str) -> Option<&str> {
    let suffix = pattern
        .strip_prefix("**/*")
        .or_else(|| pattern.strip_prefix('*'))?;
    (!suffix.is_empty()
        && !suffix
            .chars()
            .any(|character| matches!(character, '*' | '?' | '[' | ']' | '{' | '}')))
    .then_some(suffix)
}

fn simple_extension_suffix(suffix: &str) -> bool {
    suffix
        .strip_prefix('.')
        .is_some_and(|extension| !extension.is_empty() && !extension.contains('.'))
}

fn compile_command_contains(patterns: Vec<String>) -> Result<CompiledCommandContains, String> {
    if patterns.is_empty() {
        return Ok(CompiledCommandContains::default());
    }
    AhoCorasickBuilder::new()
        .ascii_case_insensitive(true)
        .match_kind(MatchKind::LeftmostFirst)
        .build(patterns)
        .map(|matcher| CompiledCommandContains {
            matcher: Some(matcher),
        })
        .map_err(|error| format!("failed to compile commandContainsAny patterns: {error}"))
}

fn canonical_event(value: &str) -> String {
    value.to_ascii_lowercase().replace('_', "-")
}

fn command_name_tokens(tokens: &[String]) -> impl Iterator<Item = &str> {
    let mut stage_start = true;
    tokens.iter().filter_map(move |token| {
        if is_shell_stage_separator(token) {
            stage_start = true;
            return None;
        }
        if stage_start {
            stage_start = false;
            Some(token.as_str())
        } else {
            None
        }
    })
}

fn command_stage_matches_argv_prefix(tokens: &[String], prefix: &[String]) -> bool {
    tokens
        .split(|token| is_shell_stage_separator(token))
        .any(|stage| {
            stage.len() >= prefix.len()
                && stage
                    .iter()
                    .zip(prefix)
                    .enumerate()
                    .all(|(index, (actual, expected))| {
                        actual.eq_ignore_ascii_case(expected)
                            || (index == 0
                                && command_token_basename(actual).eq_ignore_ascii_case(expected))
                    })
        })
}

fn command_token_basename(token: &str) -> &str {
    token.rsplit('/').next().unwrap_or(token)
}

fn is_shell_stage_separator(token: &str) -> bool {
    matches!(token, "|" | ";" | "&&" | "||" | "&")
}
