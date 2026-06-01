//! Root semantic agent hook classifier over language profile descriptors.

use serde_json::Value;

use crate::command::{
    CommandIntent, command_intent, contains_ingest_pipe, first_path, profiles_for_command,
    profiles_for_raw_search, search_json_route, semantic_shell_tokens,
};
use crate::protocol::{
    DecisionKind, DecisionRoute, DecisionRouteKind, DecisionSubject, HOOK_DECISION_SCHEMA_ID,
    HOOK_DECISION_SCHEMA_VERSION, HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, HookDecision,
    LanguageProfile, ProfileRegistry, ReasonKind,
};

#[derive(Clone, Debug)]
struct ToolAction {
    tool_name: String,
    command: Option<String>,
    paths: Vec<String>,
}

fn payload_string(payload: &Value, key: &str) -> Option<String> {
    payload.get(key).and_then(Value::as_str).map(str::to_string)
}

fn extract_command_direct(tool_name: &str, tool_input: &Value) -> Option<String> {
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

fn extract_paths_direct(tool_input: &Value) -> Vec<String> {
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
    let lower = tool_name.to_ascii_lowercase();
    matches!(
        tool_name,
        "Read"
            | "readFile"
            | "readDirectory"
            | "read_file"
            | "FsReadFile"
            | "FsReadDirectory"
            | "fs.read"
            | "fs.readDirectory"
            | "fs/readFile"
            | "fs/readDirectory"
            | "mcp_read"
    ) || (lower.starts_with("mcp__") && lower.contains("__read"))
}

fn collect_tool_actions(tool_name: &str, tool_input: &Value) -> Vec<ToolAction> {
    let command = extract_command_direct(tool_name, tool_input);
    let mut paths = extract_paths_direct(tool_input);
    if let Some(command) = command.as_deref() {
        for path in command_source_paths(command) {
            if !paths.iter().any(|existing| existing == &path) {
                paths.push(path);
            }
        }
    }
    let mut actions = vec![ToolAction {
        tool_name: tool_name.to_string(),
        command,
        paths,
    }];
    for nested in nested_tool_actions(tool_input) {
        actions.extend(collect_tool_actions(&nested.tool_name, nested.input));
    }
    actions
}

struct NestedToolAction<'a> {
    tool_name: String,
    input: &'a Value,
}

fn nested_tool_actions(tool_input: &Value) -> Vec<NestedToolAction<'_>> {
    let mut nested = Vec::new();
    for key in ["tool_uses", "toolUses"] {
        let Some(tool_uses) = tool_input.get(key).and_then(Value::as_array) else {
            continue;
        };
        for tool_use in tool_uses {
            let Some(tool_name) = payload_string(tool_use, "recipient_name")
                .or_else(|| payload_string(tool_use, "recipientName"))
                .or_else(|| payload_string(tool_use, "tool_name"))
                .or_else(|| payload_string(tool_use, "toolName"))
            else {
                continue;
            };
            let input = tool_use
                .get("parameters")
                .or_else(|| tool_use.get("tool_input"))
                .or_else(|| tool_use.get("toolInput"))
                .unwrap_or(&Value::Null);
            nested.push(NestedToolAction { tool_name, input });
        }
    }
    nested
}

fn command_source_paths(command: &str) -> Vec<String> {
    let tokens = semantic_shell_tokens(command);
    first_path(&tokens)
        .map(|path| vec![path.to_string()])
        .unwrap_or_default()
}

fn subject_for_action(action: &ToolAction) -> DecisionSubject {
    DecisionSubject {
        tool_name: if action.tool_name.is_empty() {
            None
        } else {
            Some(action.tool_name.clone())
        },
        command: action.command.clone(),
        paths: action.paths.clone(),
    }
}

/// Classify one platform hook payload against a semantic profile registry.
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
    let actions = collect_tool_actions(&tool_name, tool_input);

    if let Some(decision) = actions
        .iter()
        .find_map(|action| classify_direct_read_action(registry, platform, event, action))
    {
        return decision;
    }
    if let Some(decision) = actions
        .iter()
        .find_map(|action| classify_command_action(registry, platform, event, action))
    {
        return decision;
    }

    let subject = actions.first().map(subject_for_action).unwrap_or_default();
    allow(platform, event, subject)
}

fn classify_direct_read_action(
    registry: &ProfileRegistry,
    platform: &str,
    event: &str,
    action: &ToolAction,
) -> Option<HookDecision> {
    if !is_direct_read_tool(&action.tool_name) {
        return None;
    }
    let (profile, path) = action
        .paths
        .iter()
        .find_map(|path| {
            registry
                .profile_for_path(path)
                .map(|profile| (profile, path))
        })
        .filter(|(profile, _)| profile.policy.block_direct_read)?;
    Some(deny(
        platform,
        event,
        ReasonKind::DirectSourceRead,
        vec![profile.language_id.clone()],
        subject_for_action(action),
        vec![profile.route_from_template(
            DecisionRouteKind::Owner,
            &profile.commands.owner,
            Some(path),
            None,
        )],
        format!("Use {} search owner before reading source.", profile.binary),
    ))
}

fn classify_command_action(
    registry: &ProfileRegistry,
    platform: &str,
    event: &str,
    action: &ToolAction,
) -> Option<HookDecision> {
    let command = action.command.as_deref()?;
    let tokens = semantic_shell_tokens(command);
    classify_search_json_command(registry, platform, event, action, &tokens)
        .or_else(|| classify_source_read_command(registry, platform, event, action, &tokens))
        .or_else(|| classify_raw_search_command(registry, platform, event, action, &tokens))
}

fn classify_search_json_command(
    registry: &ProfileRegistry,
    platform: &str,
    event: &str,
    action: &ToolAction,
    tokens: &[String],
) -> Option<HookDecision> {
    if !tokens.iter().any(|token| token == "--json") {
        return None;
    }
    let (profile, argv) = search_json_route(registry, tokens)?;
    if !profile.policy.block_agent_search_json {
        return None;
    }
    Some(deny(
        platform,
        event,
        ReasonKind::AgentSearchJson,
        vec![profile.language_id.clone()],
        subject_for_action(action),
        vec![DecisionRoute {
            language_id: profile.language_id.clone(),
            provider_id: profile.provider_id.clone(),
            binary: profile.binary.clone(),
            kind: DecisionRouteKind::Text,
            argv,
            stdin_mode: None,
        }],
        "Use compact search output for agent exploration; reserve --json for schema tests and machine consumers.".to_string(),
    ))
}

fn classify_source_read_command(
    registry: &ProfileRegistry,
    platform: &str,
    event: &str,
    action: &ToolAction,
    tokens: &[String],
) -> Option<HookDecision> {
    let direct_read_profiles = profiles_for_command(registry, tokens)
        .into_iter()
        .filter(|profile| profile.policy.block_direct_read)
        .collect::<Vec<_>>();
    match command_intent(tokens) {
        CommandIntent::DirectRead => {
            let (path, profile) = first_path(tokens).and_then(|path| {
                registry
                    .profile_for_path(path)
                    .map(|profile| (path, profile))
            })?;
            Some(deny(
                platform,
                event,
                ReasonKind::DirectSourceRead,
                vec![profile.language_id.clone()],
                subject_for_action(action),
                vec![profile.route_from_template(
                    DecisionRouteKind::Owner,
                    &profile.commands.owner,
                    Some(path),
                    None,
                )],
                format!("Use {} search owner before reading source.", profile.binary),
            ))
        }
        CommandIntent::ContentDump if !direct_read_profiles.is_empty() => {
            let route = first_path(tokens)
                .and_then(|path| {
                    registry.profile_for_path(path).map(|profile| {
                        profile.route_from_template(
                            DecisionRouteKind::Owner,
                            &profile.commands.owner,
                            Some(path),
                            None,
                        )
                    })
                })
                .unwrap_or_else(|| {
                    let profile = direct_read_profiles[0];
                    profile.route_from_template(
                        DecisionRouteKind::Prime,
                        &profile.commands.prime,
                        None,
                        None,
                    )
                });
            Some(deny(
                platform,
                event,
                ReasonKind::BulkSourceDump,
                language_ids(&direct_read_profiles),
                subject_for_action(action),
                vec![route],
                "Use semantic search before dumping source content.".to_string(),
            ))
        }
        _ => None,
    }
}

fn classify_raw_search_command(
    registry: &ProfileRegistry,
    platform: &str,
    event: &str,
    action: &ToolAction,
    tokens: &[String],
) -> Option<HookDecision> {
    if command_intent(tokens) != CommandIntent::RawSearch {
        return None;
    }
    let raw_search_profiles = profiles_for_raw_search(registry, tokens)
        .into_iter()
        .filter(|profile| profile.policy.block_broad_raw_search)
        .collect::<Vec<_>>();
    if raw_search_profiles.is_empty() || contains_ingest_pipe(tokens, &raw_search_profiles) {
        return None;
    }
    let routes = raw_search_profiles
        .iter()
        .map(|profile| {
            profile.route_from_template(
                DecisionRouteKind::Ingest,
                &profile.commands.ingest,
                None,
                None,
            )
        })
        .collect();
    Some(deny(
        platform,
        event,
        ReasonKind::RawBroadSearch,
        language_ids(&raw_search_profiles),
        subject_for_action(action),
        routes,
        "Pipe broad raw search candidates into semantic search ingest.".to_string(),
    ))
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
