//! Root semantic agent hook classifier over language profile descriptors.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const PROFILE_REGISTRY_SCHEMA_ID: &str =
    "agent.semantic-protocols.semantic-agent-hook-profile-registry";
pub const PROFILE_REGISTRY_SCHEMA_VERSION: &str = "1";
pub const HOOK_DECISION_SCHEMA_ID: &str = "agent.semantic-protocols.agent-hook-decision";
pub const HOOK_DECISION_SCHEMA_VERSION: &str = "1";
pub const HOOK_PROTOCOL_ID: &str = "agent.semantic-protocols.agent-hooks";
pub const HOOK_PROTOCOL_VERSION: &str = "1";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileRegistry {
    pub schema_id: String,
    pub schema_version: String,
    pub protocol_id: String,
    pub protocol_version: String,
    pub project_root: String,
    pub profiles: Vec<LanguageProfile>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
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
    pub commands: HookCommands,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookCommands {
    pub prime: CommandTemplate,
    pub owner: CommandTemplate,
    pub text: CommandTemplate,
    pub ingest: CommandTemplate,
    pub check_changed: CommandTemplate,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandTemplate {
    pub argv: Vec<String>,
    #[serde(default)]
    pub stdin_mode: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
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
pub enum DecisionKind {
    Allow,
    Deny,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReasonKind {
    None,
    DirectSourceRead,
    BulkSourceDump,
    RawBroadSearch,
    AgentSearchJson,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
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
pub struct DecisionRoute {
    pub language_id: String,
    pub provider_id: String,
    pub binary: String,
    pub kind: String,
    pub argv: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdin_mode: Option<String>,
}

#[derive(Debug)]
pub enum AgentHookError {
    InvalidProfiles(serde_json::Error),
    InvalidProfileRegistry(String),
    InvalidPayload(serde_json::Error),
    InvalidOutput(serde_json::Error),
}

pub fn parse_profiles(input: &str) -> Result<ProfileRegistry, AgentHookError> {
    let registry: ProfileRegistry =
        serde_json::from_str(input).map_err(AgentHookError::InvalidProfiles)?;
    registry.validate_protocol()?;
    Ok(registry)
}

pub fn parse_payload(input: &str) -> Result<Value, AgentHookError> {
    serde_json::from_str(input).map_err(AgentHookError::InvalidPayload)
}

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

pub fn classify_hook(
    registry: &ProfileRegistry,
    platform: &str,
    event: &str,
    payload: &Value,
) -> HookDecision {
    let tool_name = payload_string(payload, "tool_name")
        .or_else(|| payload_string(payload, "toolName"))
        .unwrap_or_default();
    let tool_input = payload
        .get("tool_input")
        .or_else(|| payload.get("toolInput"))
        .unwrap_or(&Value::Null);
    let command = extract_command(&tool_name, tool_input);
    let paths = extract_paths(tool_input);
    let subject = DecisionSubject {
        tool_name: if tool_name.is_empty() {
            None
        } else {
            Some(tool_name.clone())
        },
        command: command.clone(),
        paths: paths.clone(),
    };

    if is_direct_read_tool(&tool_name) {
        if let Some((profile, path)) = paths.iter().find_map(|path| {
            registry
                .profile_for_path(path)
                .map(|profile| (profile, path))
        }) {
            return deny(
                platform,
                event,
                ReasonKind::DirectSourceRead,
                vec![profile.language_id.clone()],
                subject,
                vec![profile.route_from_template(
                    "owner",
                    &profile.commands.owner,
                    Some(path),
                    None,
                )],
                format!("Use {} search owner before reading source.", profile.binary),
            );
        }
    }

    if let Some(command) = command {
        let tokens = shell_tokens(&command);
        if tokens.iter().any(|token| token == "--json") {
            if let Some((profile, argv)) = search_json_route(registry, &tokens) {
                return deny(
                    platform,
                    event,
                    ReasonKind::AgentSearchJson,
                    vec![profile.language_id.clone()],
                    subject,
                    vec![DecisionRoute {
                        language_id: profile.language_id.clone(),
                        provider_id: profile.provider_id.clone(),
                        binary: profile.binary.clone(),
                        kind: "text".to_string(),
                        argv,
                        stdin_mode: None,
                    }],
                    "Use compact search output for agent exploration; reserve --json for schema tests and machine consumers.".to_string(),
                );
            }
        }

        let routed_profiles = profiles_for_command(registry, &tokens);
        if command_intent(&tokens) == CommandIntent::ContentDump && !routed_profiles.is_empty() {
            let route = first_path(&tokens)
                .and_then(|path| {
                    registry.profile_for_path(path).map(|profile| {
                        profile.route_from_template(
                            "owner",
                            &profile.commands.owner,
                            Some(path),
                            None,
                        )
                    })
                })
                .unwrap_or_else(|| {
                    let profile = routed_profiles[0];
                    profile.route_from_template("prime", &profile.commands.prime, None, None)
                });
            return deny(
                platform,
                event,
                ReasonKind::BulkSourceDump,
                language_ids(&routed_profiles),
                subject,
                vec![route],
                "Use semantic search before dumping source content.".to_string(),
            );
        }

        if command_intent(&tokens) == CommandIntent::RawSearch && !routed_profiles.is_empty() {
            if !contains_ingest_pipe(&tokens, &routed_profiles) {
                let routes = routed_profiles
                    .iter()
                    .map(|profile| {
                        profile.route_from_template("ingest", &profile.commands.ingest, None, None)
                    })
                    .collect();
                return deny(
                    platform,
                    event,
                    ReasonKind::RawBroadSearch,
                    language_ids(&routed_profiles),
                    subject,
                    routes,
                    "Pipe broad raw search candidates into semantic search ingest.".to_string(),
                );
            }
        }
    }

    allow(platform, event, subject)
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

    fn profile_for_path(&self, path: &str) -> Option<&LanguageProfile> {
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

    fn matches_search_token(&self, token: &str) -> bool {
        let normalized = token.trim_start_matches("./");
        self.matches_path(normalized)
            || self
                .source_roots
                .iter()
                .any(|root| normalized == root || normalized.starts_with(&format!("{root}/")))
    }

    fn route_from_template(
        &self,
        kind: &str,
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
            kind: kind.to_string(),
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

#[derive(Clone, Copy, Eq, PartialEq)]
enum CommandIntent {
    Other,
    ContentDump,
    RawSearch,
}

fn payload_string(payload: &Value, key: &str) -> Option<String> {
    payload.get(key).and_then(Value::as_str).map(str::to_string)
}

fn extract_command(tool_name: &str, tool_input: &Value) -> Option<String> {
    for key in ["cmd", "command"] {
        if let Some(command) = tool_input.get(key).and_then(Value::as_str) {
            return Some(command.to_string());
        }
    }
    if tool_name == "command_execution" {
        return tool_input
            .get("tool_input")
            .and_then(|value| value.get("command"))
            .and_then(Value::as_str)
            .map(str::to_string);
    }
    None
}

fn extract_paths(tool_input: &Value) -> Vec<String> {
    let mut paths = Vec::new();
    for key in ["path", "file_path", "filePath"] {
        if let Some(path) = tool_input.get(key).and_then(Value::as_str) {
            paths.push(path.to_string());
        }
    }
    if let Some(array) = tool_input.get("paths").and_then(Value::as_array) {
        for value in array {
            if let Some(path) = value.as_str() {
                paths.push(path.to_string());
            }
        }
    }
    paths
}

fn is_direct_read_tool(tool_name: &str) -> bool {
    matches!(tool_name, "Read" | "read_file" | "mcp_read")
}

fn shell_tokens(command: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut chars = command.chars().peekable();
    while let Some(ch) = chars.next() {
        match (quote, ch) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => current.push(c),
            (None, '\'' | '"') => quote = Some(ch),
            (None, '|' | ';' | '&') => {
                push_token(&mut tokens, &mut current);
                if ch == '&' && chars.peek() == Some(&'&') {
                    chars.next();
                    tokens.push("&&".to_string());
                } else {
                    tokens.push(ch.to_string());
                }
            }
            (None, c) if c.is_whitespace() => push_token(&mut tokens, &mut current),
            (None, c) => current.push(c),
        }
    }
    push_token(&mut tokens, &mut current);
    tokens
}

fn push_token(tokens: &mut Vec<String>, current: &mut String) {
    if !current.is_empty() {
        tokens.push(std::mem::take(current));
    }
}

fn command_intent(tokens: &[String]) -> CommandIntent {
    let command = first_stage_command(tokens);
    if matches!(
        command.as_deref(),
        Some("cat" | "sed" | "nl" | "bat" | "head" | "tail" | "awk" | "less")
    ) {
        return CommandIntent::ContentDump;
    }
    if matches!(
        command.as_deref(),
        Some("rg" | "grep" | "ag" | "fd" | "find" | "git")
    ) {
        return CommandIntent::RawSearch;
    }
    CommandIntent::Other
}

fn first_stage_command(tokens: &[String]) -> Option<String> {
    tokens
        .iter()
        .find(|token| !token.starts_with('-') && !is_separator(token))
        .cloned()
}

fn first_path(tokens: &[String]) -> Option<&str> {
    tokens.iter().find_map(|token| {
        if token.starts_with('-') || is_separator(token) {
            return None;
        }
        if token.contains('/') || token.contains('.') {
            Some(token.as_str())
        } else {
            None
        }
    })
}

fn profiles_for_command<'a>(
    registry: &'a ProfileRegistry,
    tokens: &[String],
) -> Vec<&'a LanguageProfile> {
    if tokens.iter().any(|token| token == ".") {
        return registry.profiles.iter().collect();
    }
    let mut profiles = Vec::new();
    for token in tokens {
        if let Some(profile) = registry
            .profiles
            .iter()
            .find(|profile| profile.matches_search_token(token))
        {
            if !profiles
                .iter()
                .any(|existing: &&LanguageProfile| existing.language_id == profile.language_id)
            {
                profiles.push(profile);
            }
        }
    }
    profiles
}

fn contains_ingest_pipe(tokens: &[String], profiles: &[&LanguageProfile]) -> bool {
    profiles.iter().any(|profile| {
        tokens.windows(3).any(|window| {
            window[0] == profile.binary && window[1] == "search" && window[2] == "ingest"
        })
    })
}

fn search_json_route<'a>(
    registry: &'a ProfileRegistry,
    tokens: &[String],
) -> Option<(&'a LanguageProfile, Vec<String>)> {
    for profile in &registry.profiles {
        let Some(binary_index) = tokens.iter().position(|token| token == &profile.binary) else {
            continue;
        };
        if tokens.get(binary_index + 1).map(String::as_str) != Some("search") {
            continue;
        }
        let mut argv = tokens[binary_index..]
            .iter()
            .take_while(|token| !is_separator(token))
            .filter(|token| token.as_str() != "--json")
            .cloned()
            .collect::<Vec<_>>();
        if !argv.iter().any(|arg| arg == "--view") {
            let insert_at = argv
                .iter()
                .rposition(|arg| arg == ".")
                .unwrap_or(argv.len());
            argv.splice(
                insert_at..insert_at,
                ["--view".to_string(), "seeds".to_string()],
            );
        }
        return Some((profile, argv));
    }
    None
}

fn is_separator(token: &str) -> bool {
    matches!(token, "|" | ";" | "&&" | "&")
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

fn language_ids(profiles: &[&LanguageProfile]) -> Vec<String> {
    profiles
        .iter()
        .map(|profile| profile.language_id.clone())
        .collect()
}

fn allow(platform: &str, event: &str, subject: DecisionSubject) -> HookDecision {
    HookDecision {
        schema_id: HOOK_DECISION_SCHEMA_ID,
        schema_version: HOOK_DECISION_SCHEMA_VERSION,
        protocol_id: HOOK_PROTOCOL_ID,
        protocol_version: HOOK_PROTOCOL_VERSION,
        platform: platform.to_string(),
        event: event.to_string(),
        decision: DecisionKind::Allow,
        reason_kind: ReasonKind::None,
        language_ids: Vec::new(),
        subject,
        routes: Vec::new(),
        message: "Allowed by semantic agent hook runtime.".to_string(),
    }
}

fn deny(
    platform: &str,
    event: &str,
    reason_kind: ReasonKind,
    language_ids: Vec<String>,
    subject: DecisionSubject,
    routes: Vec<DecisionRoute>,
    message: String,
) -> HookDecision {
    HookDecision {
        schema_id: HOOK_DECISION_SCHEMA_ID,
        schema_version: HOOK_DECISION_SCHEMA_VERSION,
        protocol_id: HOOK_PROTOCOL_ID,
        protocol_version: HOOK_PROTOCOL_VERSION,
        platform: platform.to_string(),
        event: event.to_string(),
        decision: DecisionKind::Deny,
        reason_kind,
        language_ids,
        subject,
        routes,
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn registry_value() -> Value {
        json!({
            "schemaId": PROFILE_REGISTRY_SCHEMA_ID,
            "schemaVersion": PROFILE_REGISTRY_SCHEMA_VERSION,
            "protocolId": HOOK_PROTOCOL_ID,
            "protocolVersion": HOOK_PROTOCOL_VERSION,
            "projectRoot": ".",
            "profiles": [{
                "languageId": "typescript",
                "providerId": "ts-harness",
                "binary": "ts-harness",
                "namespace": "agent.semantic-protocols.languages.typescript.ts-harness",
                "sourceExtensions": [".ts", ".tsx"],
                "configFiles": ["package.json", "tsconfig.json"],
                "sourceRoots": ["src", "tests"],
                "ignoredPathPrefixes": ["node_modules", "dist"],
                "commands": {
                    "prime": {"argv": ["ts-harness", "search", "prime", "."]},
                    "owner": {"argv": ["ts-harness", "search", "owner", "{path}", "."]},
                    "text": {"argv": ["ts-harness", "search", "text", "{query}", "owner", "tests", "--view", "seeds", "."]},
                    "ingest": {"argv": ["ts-harness", "search", "ingest", "owner", "tests", "--view", "seeds", "."], "stdinMode": "pipe-candidates"},
                    "checkChanged": {"argv": ["ts-harness", "check", "--changed", "."]}
                }
            }]
        })
    }

    fn registry() -> ProfileRegistry {
        parse_profiles(&registry_value().to_string()).unwrap()
    }

    #[test]
    fn profile_registry_protocol_identity_is_validated() {
        let mut value = registry_value();
        value["schemaId"] = json!("agent.semantic-protocols.wrong-profile-registry");

        let error = parse_profiles(&value.to_string()).unwrap_err();

        assert!(format!("{error:?}").contains("schemaId"));
    }

    #[test]
    fn platform_response_wraps_denied_decision_for_codex_hooks() {
        let decision = classify_hook(
            &registry(),
            "codex",
            "pre-tool",
            &json!({
                "tool_name": "Read",
                "tool_input": {"path": "src/cli/agent-hooks.ts"}
            }),
        );

        let response = render_platform_response(&decision).unwrap();

        assert_eq!(
            response["hookSpecificOutput"]["hookEventName"],
            "PreToolUse"
        );
        assert_eq!(response["hookSpecificOutput"]["permissionDecision"], "deny");
        assert_eq!(response["agentHookDecision"]["decision"], "deny");
        assert_eq!(
            response["agentHookDecision"]["reasonKind"],
            "direct-source-read"
        );
    }

    #[test]
    fn direct_read_routes_to_owner_search() {
        let decision = classify_hook(
            &registry(),
            "codex",
            "pre-tool",
            &json!({
                "tool_name": "Read",
                "tool_input": {"path": "src/cli/agent-hooks.ts"}
            }),
        );

        assert_eq!(decision.decision, DecisionKind::Deny);
        assert_eq!(decision.reason_kind, ReasonKind::DirectSourceRead);
        assert_eq!(
            decision.routes[0].argv,
            [
                "ts-harness",
                "search",
                "owner",
                "src/cli/agent-hooks.ts",
                "."
            ]
        );
    }

    #[test]
    fn search_json_routes_to_compact_search() {
        let decision = classify_hook(
            &registry(),
            "codex",
            "pre-tool",
            &json!({
                "tool_name": "functions.exec_command",
                "tool_input": {"cmd": "ts-harness search text projectRoot owner tests --json ."}
            }),
        );

        assert_eq!(decision.decision, DecisionKind::Deny);
        assert_eq!(decision.reason_kind, ReasonKind::AgentSearchJson);
        assert_eq!(
            decision.routes[0].argv,
            [
                "ts-harness",
                "search",
                "text",
                "projectRoot",
                "owner",
                "tests",
                "--view",
                "seeds",
                "."
            ]
        );
    }

    #[test]
    fn broad_raw_search_routes_to_ingest() {
        let decision = classify_hook(
            &registry(),
            "codex",
            "pre-tool",
            &json!({
                "tool_name": "functions.exec_command",
                "tool_input": {"cmd": "rg -n WorkflowExecution src tests"}
            }),
        );

        assert_eq!(decision.decision, DecisionKind::Deny);
        assert_eq!(decision.reason_kind, ReasonKind::RawBroadSearch);
        assert_eq!(decision.routes[0].kind, "ingest");
    }

    #[test]
    fn raw_search_piped_to_ingest_is_allowed() {
        let decision = classify_hook(
            &registry(),
            "codex",
            "pre-tool",
            &json!({
                "tool_name": "functions.exec_command",
                "tool_input": {"cmd": "rg -n WorkflowExecution src | ts-harness search ingest owner tests --view seeds ."}
            }),
        );

        assert_eq!(decision.decision, DecisionKind::Allow);
    }
}
