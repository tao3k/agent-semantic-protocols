//! Shared semantic agent hook protocol models and renderers.

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

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
/// Policy switches that control how the root hook classifier handles a provider.
pub struct HookPolicy {
    pub block_direct_read: bool,
    pub block_broad_raw_search: bool,
    pub block_agent_search_json: bool,
    pub require_prime_before_edit: bool,
}

impl Default for HookPolicy {
    fn default() -> Self {
        Self {
            block_direct_read: true,
            block_broad_raw_search: true,
            block_agent_search_json: true,
            require_prime_before_edit: true,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
/// Command templates that route denied tool use into semantic search.
pub struct HookCommands {
    pub prime: CommandTemplate,
    pub owner: CommandTemplate,
    pub text: CommandTemplate,
    pub ingest: CommandTemplate,
    pub check_changed: CommandTemplate,
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
    Text,
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
    if decision.decision == DecisionKind::Deny {
        return Ok(json!({
            "agentHookDecision": decision_value,
            "hookSpecificOutput": {
                "hookEventName": platform_hook_event_name(&decision.event),
                "permissionDecision": "deny",
                "permissionDecisionReason": decision.message,
            },
            "systemMessage": decision.message,
        }));
    }
    Ok(json!({
        "agentHookDecision": decision_value,
    }))
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

    pub(crate) fn profile_for_path(&self, path: &str) -> Option<&LanguageProfile> {
        let normalized = path.trim_start_matches("./");
        self.profiles
            .iter()
            .find(|profile| profile.matches_path(normalized))
    }
}

impl LanguageProfile {
    fn matches_path(&self, path: &str) -> bool {
        if self
            .ignored_path_prefixes
            .iter()
            .any(|prefix| path == prefix || path.starts_with(&format!("{prefix}/")))
        {
            return false;
        }
        if self
            .config_files
            .iter()
            .any(|config| path.ends_with(config))
        {
            return true;
        }
        self.source_extensions
            .iter()
            .any(|extension| path.ends_with(extension))
    }

    pub(crate) fn matches_search_token(&self, token: &str) -> bool {
        let normalized = token.trim_start_matches("./");
        self.matches_path(normalized)
            || self
                .source_roots
                .iter()
                .any(|root| normalized == root || normalized.starts_with(&format!("{root}/")))
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
