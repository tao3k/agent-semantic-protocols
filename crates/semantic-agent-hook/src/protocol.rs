//! Shared semantic agent hook protocol models and renderers.

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Schema identifier for semantic hook profile registries.
pub const PROFILE_REGISTRY_SCHEMA_ID: &str =
    "agent.semantic-protocols.semantic-agent-hook-profile-registry";
/// Schema version for semantic hook profile registries.
pub const PROFILE_REGISTRY_SCHEMA_VERSION: &str = "1";
/// Schema identifier for shared hook decision packets.
pub const HOOK_DECISION_SCHEMA_ID: &str = "agent.semantic-protocols.agent-hook-decision";
/// Schema version for shared hook decision packets.
pub const HOOK_DECISION_SCHEMA_VERSION: &str = "1";
/// Protocol identifier for the root semantic hook runtime.
pub const HOOK_PROTOCOL_ID: &str = "agent.semantic-protocols.agent-hooks";
/// Protocol version for the root semantic hook runtime.
pub const HOOK_PROTOCOL_VERSION: &str = "1";

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
/// Root registry of language hook profiles consumed by `semantic-agent-hook`.
pub struct ProfileRegistry {
    pub schema_id: String,
    pub schema_version: String,
    pub protocol_id: String,
    pub protocol_version: String,
    pub project_root: String,
    pub profiles: Vec<LanguageProfile>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
/// Language-owned policy and command descriptors used by the root hook runtime.
pub struct LanguageProfile {
    pub language_id: String,
    pub provider_id: String,
    pub binary: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provider_command_prefix: Vec<String>,
    pub namespace: String,
    #[serde(default)]
    pub source_extensions: Vec<String>,
    #[serde(default)]
    pub config_files: Vec<String>,
    #[serde(default)]
    pub source_roots: Vec<String>,
    #[serde(default)]
    pub ignored_path_prefixes: Vec<String>,
    #[serde(default)]
    pub policy: HookPolicy,
    pub commands: HookCommands,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SourceSelectorKind {
    ExactPath,
    Pattern,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ProfileSelectorMatch<'a> {
    pub profile: &'a LanguageProfile,
    pub kind: SourceSelectorKind,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
/// Policy switches that control how the root hook classifier handles a provider.
pub struct HookPolicy {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direct_source_read: Option<ActionPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bulk_source_dump: Option<ActionPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_source_search: Option<ActionPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_search_json: Option<ActionPolicy>,
    #[serde(default = "default_true")]
    pub block_direct_read: bool,
    #[serde(default = "default_true")]
    pub block_broad_raw_search: bool,
    #[serde(default = "default_true")]
    pub block_agent_search_json: bool,
    #[serde(default = "default_true")]
    pub require_prime_before_edit: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
/// Per-action enforcement mode for hook policy.
pub enum ActionPolicy {
    Block,
    Allow,
    Advisory,
}

impl Default for HookPolicy {
    fn default() -> Self {
        Self {
            direct_source_read: None,
            bulk_source_dump: None,
            raw_source_search: None,
            agent_search_json: None,
            block_direct_read: true,
            block_broad_raw_search: true,
            block_agent_search_json: true,
            require_prime_before_edit: true,
        }
    }
}

impl HookPolicy {
    pub(crate) fn blocks_direct_source_read(&self) -> bool {
        action_blocks(self.direct_source_read, self.block_direct_read)
    }

    pub(crate) fn blocks_bulk_source_dump(&self) -> bool {
        action_blocks(self.bulk_source_dump, self.block_direct_read)
    }

    pub(crate) fn blocks_raw_source_search(&self) -> bool {
        action_blocks(self.raw_source_search, self.block_broad_raw_search)
    }

    pub(crate) fn blocks_agent_search_json(&self) -> bool {
        action_blocks(self.agent_search_json, self.block_agent_search_json)
    }
}

fn action_blocks(action: Option<ActionPolicy>, legacy_block: bool) -> bool {
    match action {
        Some(ActionPolicy::Block) => true,
        Some(ActionPolicy::Allow | ActionPolicy::Advisory) => false,
        None => legacy_block,
    }
}

fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
/// Command templates that route denied tool use into semantic search.
pub struct HookCommands {
    pub prime: CommandTemplate,
    pub owner: CommandTemplate,
    pub text: CommandTemplate,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<CommandTemplate>,
    pub ingest: CommandTemplate,
    pub check_changed: CommandTemplate,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guide: Option<CommandTemplate>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
/// Argument template for a provider-owned semantic search command.
pub struct CommandTemplate {
    pub argv: Vec<String>,
    #[serde(default)]
    pub stdin_mode: Option<StdinMode>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Shared decision packet emitted before platform-specific hook rendering.
pub struct HookDecision {
    pub schema_id: &'static str,
    pub schema_version: &'static str,
    pub protocol_id: &'static str,
    pub protocol_version: &'static str,
    pub platform: String,
    pub event: String,
    pub decision: DecisionKind,
    pub reason_kind: ReasonKind,
    pub language_ids: Vec<String>,
    pub subject: DecisionSubject,
    pub routes: Vec<DecisionRoute>,
    pub message: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
/// Allow or deny result emitted by the hook classifier.
pub enum DecisionKind {
    Allow,
    Block,
    Deny,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
/// Reason category for a hook decision.
pub enum ReasonKind {
    None,
    DirectSourceRead,
    BulkSourceDump,
    RawBroadSearch,
    AgentSearchJson,
    SubagentReceiptRequired,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
/// Stdin handling contract for a semantic hook route.
pub enum StdinMode {
    None,
    PipeCandidates,
    PipeDiff,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
/// Semantic route kind suggested by a hook denial.
pub enum DecisionRouteKind {
    Prime,
    Owner,
    Query,
    Text,
    Read,
    Deps,
    Api,
    Ingest,
    Tests,
    CheckChanged,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
/// Tool name, command, and paths that triggered a hook decision.
pub struct DecisionSubject {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Provider command route that the agent should run instead of denied tool use.
pub struct DecisionRoute {
    pub language_id: String,
    pub provider_id: String,
    pub binary: String,
    pub kind: DecisionRouteKind,
    pub argv: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdin_mode: Option<StdinMode>,
}

#[derive(Debug)]
/// Errors produced while parsing profiles, payloads, or rendered hook output.
pub enum AgentHookError {
    InvalidProfiles(serde_json::Error),
    InvalidProfileRegistry(String),
    InvalidPayload(serde_json::Error),
    InvalidOutput(serde_json::Error),
}

/// Parse and validate a semantic hook profile registry.
pub fn parse_profiles(input: &str) -> Result<ProfileRegistry, AgentHookError> {
    let registry: ProfileRegistry =
        serde_json::from_str(input).map_err(AgentHookError::InvalidProfiles)?;
    registry.validate_protocol()?;
    Ok(registry)
}

/// Parse a platform hook payload as JSON.
pub fn parse_payload(input: &str) -> Result<Value, AgentHookError> {
    serde_json::from_str(input).map_err(AgentHookError::InvalidPayload)
}

/// Merge provider profile registries by replacing matching language/provider pairs.
pub fn merge_profile_registries(registries: Vec<ProfileRegistry>) -> ProfileRegistry {
    let project_root = registries
        .first()
        .map(|registry| registry.project_root.clone())
        .unwrap_or_else(|| ".".to_string());
    let mut profiles = Vec::<LanguageProfile>::new();
    for registry in registries {
        for profile in registry.profiles {
            if let Some(existing_index) = profiles.iter().position(|existing| {
                existing.language_id == profile.language_id
                    && existing.provider_id == profile.provider_id
            }) {
                profiles[existing_index] = profile;
            } else {
                profiles.push(profile);
            }
        }
    }
    ProfileRegistry {
        schema_id: PROFILE_REGISTRY_SCHEMA_ID.to_string(),
        schema_version: PROFILE_REGISTRY_SCHEMA_VERSION.to_string(),
        protocol_id: HOOK_PROTOCOL_ID.to_string(),
        protocol_version: HOOK_PROTOCOL_VERSION.to_string(),
        project_root,
        profiles,
    }
}

/// Render a shared hook decision into the selected platform response envelope.
pub fn render_platform_response(decision: &HookDecision) -> Result<Value, AgentHookError> {
    let decision_value = serde_json::to_value(decision).map_err(AgentHookError::InvalidOutput)?;
    let decision_context = format!(
        "[agent-hook-decision] {}",
        serde_json::to_string(&decision_value).map_err(AgentHookError::InvalidOutput)?
    );
    match decision.decision {
        DecisionKind::Deny => {
            return Ok(json!({
                "hookSpecificOutput": {
                    "hookEventName": platform_hook_event_name(&decision.event),
                    "permissionDecision": "deny",
                    "permissionDecisionReason": decision.message,
                    "additionalContext": decision_context,
                },
                "systemMessage": decision.message,
            }));
        }
        DecisionKind::Block => {
            return Ok(json!({
                "decision": "block",
                "reason": decision.message,
                "hookSpecificOutput": {
                    "hookEventName": platform_hook_event_name(&decision.event),
                    "additionalContext": decision_context,
                },
                "systemMessage": decision.message,
            }));
        }
        DecisionKind::Allow => {}
    }
    Ok(json!({}))
}

impl ProfileRegistry {
    fn validate_protocol(&self) -> Result<(), AgentHookError> {
        expect_field("schemaId", &self.schema_id, PROFILE_REGISTRY_SCHEMA_ID)?;
        expect_field(
            "schemaVersion",
            &self.schema_version,
            PROFILE_REGISTRY_SCHEMA_VERSION,
        )?;
        expect_field("protocolId", &self.protocol_id, HOOK_PROTOCOL_ID)?;
        expect_field(
            "protocolVersion",
            &self.protocol_version,
            HOOK_PROTOCOL_VERSION,
        )?;
        Ok(())
    }

    pub(crate) fn profiles_for_selector(&self, selector: &str) -> Vec<ProfileSelectorMatch<'_>> {
        let matcher = SourceSelectorMatcher::new(selector);
        self.profiles
            .iter()
            .filter_map(|profile| {
                profile
                    .match_source_selector_with(&matcher)
                    .map(|kind| ProfileSelectorMatch { profile, kind })
            })
            .collect()
    }
}

impl LanguageProfile {
    pub(crate) fn matches_source_selector(&self, selector: &str) -> bool {
        self.match_source_selector(selector).is_some()
    }

    pub(crate) fn match_source_selector(&self, selector: &str) -> Option<SourceSelectorKind> {
        let matcher = SourceSelectorMatcher::new(selector);
        self.match_source_selector_with(&matcher)
    }

    fn match_source_selector_with(
        &self,
        selector: &SourceSelectorMatcher<'_>,
    ) -> Option<SourceSelectorKind> {
        if self
            .ignored_path_prefixes
            .iter()
            .any(|prefix| selector.is_ignored_by(prefix))
        {
            return None;
        }
        if selector.has_glob {
            return self
                .glob_matches_source_selector(selector)
                .then_some(SourceSelectorKind::Pattern);
        }
        if self
            .config_files
            .iter()
            .any(|config| selector.normalized.ends_with(config))
        {
            return Some(SourceSelectorKind::ExactPath);
        }
        if self
            .source_extensions
            .iter()
            .any(|extension| selector.normalized.ends_with(extension))
        {
            return Some(SourceSelectorKind::ExactPath);
        }
        None
    }

    pub(crate) fn matches_search_token(&self, token: &str) -> bool {
        let normalized = token.trim_start_matches("./");
        self.matches_source_selector(normalized)
            || self
                .source_roots
                .iter()
                .any(|root| normalized == root || normalized.starts_with(&format!("{root}/")))
    }

    fn glob_matches_source_selector(&self, selector: &SourceSelectorMatcher<'_>) -> bool {
        self.source_extensions
            .iter()
            .any(|extension| selector.targets_extension(extension))
            || self
                .config_files
                .iter()
                .any(|config| selector.matches_config(config))
    }

    pub(crate) fn route_from_template(
        &self,
        kind: DecisionRouteKind,
        template: &CommandTemplate,
        path: Option<&str>,
        query: Option<&str>,
    ) -> DecisionRoute {
        let argv = template
            .argv
            .iter()
            .map(|arg| {
                arg.replace("{path}", path.unwrap_or(""))
                    .replace("{query}", query.unwrap_or(""))
                    .replace("{projectRoot}", ".")
            })
            .collect();
        DecisionRoute {
            language_id: self.language_id.clone(),
            provider_id: self.provider_id.clone(),
            binary: self.binary.clone(),
            kind,
            argv,
            stdin_mode: template.stdin_mode.clone(),
        }
    }
}

struct SourceSelectorMatcher<'a> {
    normalized: &'a str,
    has_glob: bool,
    extension_glob: Option<GlobSet>,
    basename_glob: Option<GlobSet>,
    literal_prefix: &'a str,
    has_extension_pattern: bool,
}

impl<'a> SourceSelectorMatcher<'a> {
    fn new(selector: &'a str) -> Self {
        let normalized = normalize_source_selector(selector);
        let has_glob = selector_has_glob(normalized);
        let basename = basename_pattern(normalized).to_ascii_lowercase();
        let extension_pattern = basename_extension_pattern(&basename);
        Self {
            normalized,
            has_glob,
            extension_glob: extension_pattern.and_then(build_glob_set),
            basename_glob: build_glob_set(&basename),
            literal_prefix: literal_prefix_before_glob(normalized),
            has_extension_pattern: extension_pattern.is_some(),
        }
    }

    fn is_ignored_by(&self, prefix: &str) -> bool {
        self.normalized == prefix || self.normalized.starts_with(&format!("{prefix}/"))
    }

    fn targets_extension(&self, extension: &str) -> bool {
        let extension = extension.trim_start_matches('.').to_ascii_lowercase();
        self.extension_glob
            .as_ref()
            .is_some_and(|glob_set| glob_set.is_match(extension))
    }

    fn matches_config(&self, config: &str) -> bool {
        if !self.has_extension_pattern {
            return false;
        }
        if self.normalized.contains('/') && !self.literal_prefix.trim_matches('/').is_empty() {
            return false;
        }
        let config_basename = basename_pattern(config).to_ascii_lowercase();
        self.basename_glob
            .as_ref()
            .is_some_and(|glob_set| glob_set.is_match(config_basename))
    }
}

fn build_glob_set(pattern: &str) -> Option<GlobSet> {
    let glob = match GlobBuilder::new(pattern)
        .literal_separator(false)
        .backslash_escape(false)
        .build()
    {
        Ok(glob) => glob,
        Err(_) => return None,
    };
    let mut builder = GlobSetBuilder::new();
    builder.add(glob);
    builder.build().ok()
}

pub(crate) fn normalize_source_selector(selector: &str) -> &str {
    strip_line_locator(selector.trim_start_matches("./"))
}

fn strip_line_locator(selector: &str) -> &str {
    let Some((prefix, suffix)) = selector.rsplit_once(':') else {
        return selector;
    };
    if is_line_range_locator(suffix) {
        return prefix;
    }
    if is_decimal_locator(suffix)
        && let Some((path, line)) = prefix.rsplit_once(':')
        && is_decimal_locator(line)
    {
        return path;
    }
    selector
}

fn is_line_range_locator(locator: &str) -> bool {
    if let Some((start, end)) = locator
        .split_once('-')
        .or_else(|| locator.split_once('\u{2013}'))
    {
        return is_decimal_locator(start) && is_decimal_locator(end);
    }
    is_decimal_locator(locator)
}

fn is_decimal_locator(locator: &str) -> bool {
    !locator.is_empty() && locator.chars().all(|character| character.is_ascii_digit())
}

fn basename_pattern(selector: &str) -> &str {
    selector
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(selector)
}

fn basename_extension_pattern(basename: &str) -> Option<&str> {
    let (_, _, last_literal_dot) = basename.char_indices().fold(
        (0usize, 0usize, None),
        |(bracket_depth, brace_depth, last_literal_dot), (index, character)| match character {
            '[' => (bracket_depth + 1, brace_depth, last_literal_dot),
            ']' if bracket_depth > 0 => (bracket_depth - 1, brace_depth, last_literal_dot),
            '{' if bracket_depth == 0 => (bracket_depth, brace_depth + 1, last_literal_dot),
            '}' if bracket_depth == 0 && brace_depth > 0 => {
                (bracket_depth, brace_depth - 1, last_literal_dot)
            }
            '.' if bracket_depth == 0 && brace_depth == 0 => {
                (bracket_depth, brace_depth, Some(index))
            }
            _ => (bracket_depth, brace_depth, last_literal_dot),
        },
    );
    let start = last_literal_dot? + 1;
    (start < basename.len()).then_some(&basename[start..])
}

fn literal_prefix_before_glob(selector: &str) -> &str {
    let glob_start = selector
        .char_indices()
        .find_map(|(index, character)| {
            matches!(character, '*' | '?' | '[' | ']' | '{' | '}').then_some(index)
        })
        .unwrap_or(selector.len());
    selector[..glob_start].trim_end_matches('/')
}

fn selector_has_glob(path: &str) -> bool {
    path.chars()
        .any(|character| matches!(character, '*' | '?' | '[' | ']' | '{' | '}'))
}

fn expect_field(name: &str, actual: &str, expected: &str) -> Result<(), AgentHookError> {
    if actual == expected {
        return Ok(());
    }
    Err(AgentHookError::InvalidProfileRegistry(format!(
        "invalid profile registry {name}: expected {expected}, got {actual}"
    )))
}

fn platform_hook_event_name(event: &str) -> &'static str {
    match event {
        "session-start" => "SessionStart",
        "user-prompt" => "UserPromptSubmit",
        "pre-tool" => "PreToolUse",
        "permission-request" => "PermissionRequest",
        "post-tool" => "PostToolUse",
        "subagent-start" => "SubagentStart",
        "subagent-stop" => "SubagentStop",
        "stop" => "Stop",
        _ => "Unknown",
    }
}
