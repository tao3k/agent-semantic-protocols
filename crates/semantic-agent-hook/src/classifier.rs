//! Root semantic agent hook classifier over language profile descriptors.

use serde_json::Value;

use crate::command::{
    CommandIntent, command_intent, contains_ingest_pipe, path_like_tokens, profiles_for_raw_search,
    search_json_route, semantic_shell_tokens,
};
use crate::protocol::{
    DecisionKind, DecisionRoute, DecisionRouteKind, DecisionSubject, HOOK_DECISION_SCHEMA_ID,
    HOOK_DECISION_SCHEMA_VERSION, HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, HookDecision,
    LanguageProfile, ProfileRegistry, ProfileSelectorMatch, ReasonKind, SourceSelectorKind,
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
    let normalized = lower
        .chars()
        .map(|ch| match ch {
            '-' | '/' | ':' => '.',
            _ => ch,
        })
        .collect::<String>();
    is_direct_read_tool_alias(&normalized)
        || normalized
            .split('.')
            .next_back()
            .is_some_and(is_direct_read_tool_alias)
        || (lower.starts_with("mcp__") && lower.contains("__read"))
}

fn is_direct_read_tool_alias(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "read"
            | "readfile"
            | "read_file"
            | "readdirectory"
            | "read_directory"
            | "fsreadfile"
            | "fsreaddirectory"
            | "mcp_read"
    )
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
    path_like_tokens(&tokens)
        .into_iter()
        .map(str::to_string)
        .collect()
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
    if let Some(decision) = classify_subagent_stop(platform, event, payload) {
        return decision;
    }

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

fn classify_subagent_stop(platform: &str, event: &str, payload: &Value) -> Option<HookDecision> {
    if event != "subagent-stop" {
        return None;
    }
    let last_message = payload_string(payload, "last_assistant_message")
        .or_else(|| payload_string(payload, "lastAssistantMessage"))
        .unwrap_or_default();
    if last_message.contains("[search-subagent]") {
        return Some(allow(platform, event, DecisionSubject::default()));
    }
    Some(HookDecision {
        schema_id: HOOK_DECISION_SCHEMA_ID,
        schema_version: HOOK_DECISION_SCHEMA_VERSION,
        protocol_id: HOOK_PROTOCOL_ID,
        protocol_version: HOOK_PROTOCOL_VERSION,
        platform: platform.to_string(),
        event: event.to_string(),
        decision: DecisionKind::Block,
        reason_kind: ReasonKind::SubagentReceiptRequired,
        language_ids: Vec::new(),
        subject: DecisionSubject::default(),
        routes: Vec::new(),
        message: "SubagentStop requires compact [search-subagent] evidence before fan-in."
            .to_string(),
    })
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
    direct_read_decision(
        registry,
        platform,
        event,
        action,
        collect_direct_read_matches(registry, action.paths.iter().map(String::as_str)),
    )
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
    if !profile.policy.blocks_agent_search_json() {
        return None;
    }
    let routes = vec![DecisionRoute {
        language_id: profile.language_id.clone(),
        provider_id: profile.provider_id.clone(),
        binary: profile.binary.clone(),
        kind: DecisionRouteKind::Text,
        argv,
        stdin_mode: None,
    }];
    let message = provider_guide_message("agent-search-json denied", &[profile]);
    Some(deny(
        platform,
        event,
        ReasonKind::AgentSearchJson,
        vec![profile.language_id.clone()],
        subject_for_action(action),
        routes,
        message,
    ))
}

fn classify_source_read_command(
    registry: &ProfileRegistry,
    platform: &str,
    event: &str,
    action: &ToolAction,
    tokens: &[String],
) -> Option<HookDecision> {
    match command_intent(tokens) {
        CommandIntent::DirectRead => direct_read_decision(
            registry,
            platform,
            event,
            action,
            collect_direct_read_matches(registry, path_like_tokens(tokens)),
        ),
        CommandIntent::ContentDump => {
            let matches = collect_content_dump_matches(registry, path_like_tokens(tokens));
            if matches.is_empty() {
                return None;
            }
            let routes = direct_read_routes(&matches);
            let profiles = profiles_from_matches(&matches);
            let message = provider_guide_message("bulk-source-dump denied", &profiles);
            Some(deny(
                platform,
                event,
                ReasonKind::BulkSourceDump,
                direct_read_language_ids(&matches),
                subject_for_action(action),
                routes,
                message,
            ))
        }
        _ => None,
    }
}

fn direct_read_decision(
    _registry: &ProfileRegistry,
    platform: &str,
    event: &str,
    action: &ToolAction,
    matches: Vec<DirectReadMatch<'_, '_>>,
) -> Option<HookDecision> {
    if matches.is_empty() {
        return None;
    }
    let routes = direct_read_routes(&matches);
    let message = direct_read_decision_message(&matches, &routes);
    Some(deny(
        platform,
        event,
        ReasonKind::DirectSourceRead,
        direct_read_language_ids(&matches),
        subject_for_action(action),
        routes,
        message,
    ))
}

type DirectReadMatch<'path, 'profile> = (&'path str, ProfileSelectorMatch<'profile>);

fn collect_direct_read_matches<'path, 'profile, I>(
    registry: &'profile ProfileRegistry,
    paths: I,
) -> Vec<DirectReadMatch<'path, 'profile>>
where
    I: IntoIterator<Item = &'path str>,
{
    collect_source_selector_matches(registry, paths, |profile| {
        profile.policy.blocks_direct_source_read()
    })
}

fn collect_content_dump_matches<'path, 'profile, I>(
    registry: &'profile ProfileRegistry,
    paths: I,
) -> Vec<DirectReadMatch<'path, 'profile>>
where
    I: IntoIterator<Item = &'path str>,
{
    collect_source_selector_matches(registry, paths, |profile| {
        profile.policy.blocks_bulk_source_dump()
    })
}

fn collect_source_selector_matches<'path, 'profile, I, F>(
    registry: &'profile ProfileRegistry,
    paths: I,
    should_block: F,
) -> Vec<DirectReadMatch<'path, 'profile>>
where
    I: IntoIterator<Item = &'path str>,
    F: Fn(&LanguageProfile) -> bool,
{
    let mut matches: Vec<DirectReadMatch<'path, 'profile>> = Vec::new();
    for path in paths {
        for matched in registry.profiles_for_selector(path) {
            if !should_block(matched.profile) {
                continue;
            }
            if matches.iter().any(|(_, existing)| {
                existing.profile.language_id == matched.profile.language_id
                    && existing.profile.provider_id == matched.profile.provider_id
            }) {
                continue;
            }
            matches.push((path, matched));
        }
    }
    matches
}

fn direct_read_routes(matches: &[DirectReadMatch<'_, '_>]) -> Vec<DecisionRoute> {
    matches
        .iter()
        .map(|(path, matched)| direct_read_route(matched.profile, path, matched.kind))
        .collect()
}

fn direct_read_language_ids(matches: &[DirectReadMatch<'_, '_>]) -> Vec<String> {
    matches
        .iter()
        .map(|(_, matched)| matched.profile.language_id.clone())
        .collect()
}

fn direct_read_decision_message(
    matches: &[DirectReadMatch<'_, '_>],
    routes: &[DecisionRoute],
) -> String {
    let profiles = profiles_from_matches(matches);
    if !matches
        .iter()
        .any(|(_, matched)| matched.kind == SourceSelectorKind::ExactPath)
    {
        return provider_guide_message("direct-source-read denied", &profiles);
    }
    let rendered_routes = routes
        .iter()
        .map(|route| command_line(&route.argv))
        .collect::<Vec<_>>();
    if rendered_routes.is_empty() {
        return provider_guide_message("direct-source-read denied", &profiles);
    }
    format!(
        "direct-source-read denied; route: {}",
        rendered_routes.join("; ")
    )
}

fn profiles_from_matches<'a>(matches: &[DirectReadMatch<'_, 'a>]) -> Vec<&'a LanguageProfile> {
    matches.iter().map(|(_, matched)| matched.profile).collect()
}

fn provider_guide_message(reason: &str, profiles: &[&LanguageProfile]) -> String {
    let mut rendered_guides = Vec::<String>::new();
    let mut seen_providers = Vec::<(&str, &str)>::new();
    for profile in profiles {
        if seen_providers.iter().any(|(language_id, provider_id)| {
            *language_id == profile.language_id && *provider_id == profile.provider_id
        }) {
            continue;
        }
        seen_providers.push((profile.language_id.as_str(), profile.provider_id.as_str()));
        rendered_guides.push(format!(
            "{} => {}",
            profile.provider_id,
            command_line(&provider_guide_argv(profile))
        ));
    }
    if rendered_guides.is_empty() {
        return format!("{reason}; provider guide unavailable");
    }
    format!("{reason}; provider guide: {}", rendered_guides.join("; "))
}

fn provider_guide_argv(profile: &LanguageProfile) -> Vec<String> {
    profile
        .commands
        .guide
        .as_ref()
        .map(|template| {
            template
                .argv
                .iter()
                .map(|arg| arg.replace("{projectRoot}", "."))
                .collect()
        })
        .unwrap_or_else(|| {
            vec![
                profile.binary.clone(),
                "agent".to_string(),
                "guide".to_string(),
                ".".to_string(),
            ]
        })
}

fn direct_read_route(
    profile: &LanguageProfile,
    path: &str,
    selector_kind: SourceSelectorKind,
) -> DecisionRoute {
    match selector_kind {
        SourceSelectorKind::ExactPath => {
            let query = direct_read_query(profile, path);
            profile.route_from_template(
                DecisionRouteKind::Owner,
                &profile.commands.owner,
                Some(path),
                query.as_deref(),
            )
        }
        SourceSelectorKind::Pattern => profile.route_from_template(
            DecisionRouteKind::Prime,
            &profile.commands.prime,
            None,
            None,
        ),
    }
}

fn direct_read_query(profile: &LanguageProfile, path: &str) -> Option<String> {
    if !profile
        .commands
        .owner
        .argv
        .iter()
        .any(|arg| arg.contains("{query}"))
    {
        return None;
    }
    infer_query_from_path(path)
}

fn infer_query_from_path(path: &str) -> Option<String> {
    let normalized = path.trim().trim_end_matches('/');
    let file_name = normalized.rsplit('/').next()?;
    let stem = file_name
        .rsplit_once('.')
        .map_or(file_name, |(stem, _)| stem);
    let base = if matches!(stem, "index" | "mod" | "__init__") {
        normalized.rsplit('/').nth(1).unwrap_or(stem)
    } else {
        stem
    };
    query_variants(base)
}

fn query_variants(base: &str) -> Option<String> {
    let raw = base.trim_matches(|ch: char| !ch.is_ascii_alphanumeric());
    if raw.is_empty() {
        return None;
    }
    let pascal = title_case_identifier(raw);
    let camel = lower_first_ascii(&pascal);
    let mut variants = Vec::new();
    push_unique(&mut variants, raw.to_string());
    if !pascal.is_empty() {
        push_unique(&mut variants, pascal);
    }
    if !camel.is_empty() {
        push_unique(&mut variants, camel);
    }
    Some(variants.join("|"))
}

fn title_case_identifier(value: &str) -> String {
    value
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(uppercase_first_ascii)
        .collect::<String>()
}

fn uppercase_first_ascii(value: &str) -> String {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut output = String::new();
    output.push(first.to_ascii_uppercase());
    output.extend(chars);
    output
}

fn lower_first_ascii(value: &str) -> String {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut output = String::new();
    output.push(first.to_ascii_lowercase());
    output.extend(chars);
    output
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

fn command_line(argv: &[String]) -> String {
    argv.iter()
        .map(|arg| shell_quote_arg(arg))
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_quote_arg(arg: &str) -> String {
    if arg.chars().all(|character| {
        character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | '/' | ':')
    }) {
        return arg.to_string();
    }
    format!("'{}'", arg.replace('\'', "'\\''"))
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
        .filter(|profile| profile.policy.blocks_raw_source_search())
        .collect::<Vec<_>>();
    if raw_search_profiles.is_empty() || contains_ingest_pipe(tokens, &raw_search_profiles) {
        return None;
    }
    let routes: Vec<DecisionRoute> = raw_search_profiles
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
    let message = provider_guide_message("raw-broad-search denied", &raw_search_profiles);
    Some(deny(
        platform,
        event,
        ReasonKind::RawBroadSearch,
        language_ids(&raw_search_profiles),
        subject_for_action(action),
        routes,
        message,
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
