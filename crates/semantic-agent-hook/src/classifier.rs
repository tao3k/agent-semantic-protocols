//! Root semantic agent hook classifier over language profile descriptors.

use serde_json::Value;

use crate::command::{
    CommandIntent, command_intent, contains_ingest_pipe, first_path, profiles_for_command,
    profiles_for_raw_search, search_json_route, semantic_shell_tokens,
};
use crate::protocol::{
    DecisionKind, DecisionRoute, DecisionSubject, HOOK_DECISION_SCHEMA_ID,
    HOOK_DECISION_SCHEMA_VERSION, HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, HookDecision,
    LanguageProfile, ProfileRegistry, ReasonKind,
};

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
        if let Some((profile, path)) = paths
            .iter()
            .find_map(|path| {
                registry
                    .profile_for_path(path)
                    .map(|profile| (profile, path))
            })
            .filter(|(profile, _)| profile.policy.block_direct_read)
        {
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
        let tokens = semantic_shell_tokens(&command);
        if tokens.iter().any(|token| token == "--json") {
            if let Some((profile, argv)) = search_json_route(registry, &tokens) {
                if profile.policy.block_agent_search_json {
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
        }

        let routed_profiles = profiles_for_command(registry, &tokens);
        let direct_read_profiles = routed_profiles
            .iter()
            .copied()
            .filter(|profile| profile.policy.block_direct_read)
            .collect::<Vec<_>>();
        if command_intent(&tokens) == CommandIntent::ContentDump && !direct_read_profiles.is_empty()
        {
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
                    let profile = direct_read_profiles[0];
                    profile.route_from_template("prime", &profile.commands.prime, None, None)
                });
            return deny(
                platform,
                event,
                ReasonKind::BulkSourceDump,
                language_ids(&direct_read_profiles),
                subject,
                vec![route],
                "Use semantic search before dumping source content.".to_string(),
            );
        }

        let raw_search_profiles = profiles_for_raw_search(registry, &tokens)
            .into_iter()
            .filter(|profile| profile.policy.block_broad_raw_search)
            .collect::<Vec<_>>();
        if command_intent(&tokens) == CommandIntent::RawSearch && !raw_search_profiles.is_empty() {
            if !contains_ingest_pipe(&tokens, &raw_search_profiles) {
                let routes = raw_search_profiles
                    .iter()
                    .map(|profile| {
                        profile.route_from_template("ingest", &profile.commands.ingest, None, None)
                    })
                    .collect();
                return deny(
                    platform,
                    event,
                    ReasonKind::RawBroadSearch,
                    language_ids(&raw_search_profiles),
                    subject,
                    routes,
                    "Pipe broad raw search candidates into semantic search ingest.".to_string(),
                );
            }
        }
    }

    allow(platform, event, subject)
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
