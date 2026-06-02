//! Root semantic agent hook classifier over language profile descriptors.

use serde_json::Value;

use crate::command::looks_like_command_transcript;
use crate::command::{
    CommandIntent, command_intent, contains_ingest_pipe, infer_query_from_path, path_like_tokens,
    raw_search_plan, search_json_route, search_query_route, semantic_shell_tokens,
};
use crate::protocol::{
    DecisionKind, DecisionRoute, DecisionRouteKind, DecisionSubject, HOOK_DECISION_SCHEMA_ID,
    HOOK_DECISION_SCHEMA_VERSION, HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, HookDecision,
    LanguageProfile, ProfileRegistry, ProfileSelectorMatch, ReasonKind, SourceSelectorKind,
    normalize_source_selector,
};
use crate::tool_action::{ToolAction, collect_tool_actions, payload_string, subject_for_action};

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
    if !is_direct_read_tool(&action.tool_name)
        && !action
            .command
            .as_deref()
            .is_some_and(looks_like_command_transcript)
    {
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
    let route = search_json_decision_route(profile, argv);
    let message = format!(
        "agent-search-json denied; route: {}",
        command_line(&route.argv)
    );
    let routes = vec![route];
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

fn search_json_decision_route(profile: &LanguageProfile, argv: Vec<String>) -> DecisionRoute {
    if let Some(path) = search_json_owner_path(&argv).map(str::to_string) {
        let query = infer_query_from_path(&path);
        return profile.route_from_template(
            DecisionRouteKind::Owner,
            &profile.commands.owner,
            Some(&path),
            query.as_deref(),
        );
    }
    DecisionRoute {
        language_id: profile.language_id.clone(),
        provider_id: profile.provider_id.clone(),
        binary: profile.binary.clone(),
        kind: DecisionRouteKind::Text,
        argv,
        stdin_mode: None,
    }
}

fn search_json_owner_path(argv: &[String]) -> Option<&str> {
    if argv.get(1).map(String::as_str) != Some("search") {
        return None;
    }
    if argv.get(2).map(String::as_str) != Some("owner") {
        return None;
    }
    let path = argv.get(3)?.as_str();
    if path == "." || path.starts_with('-') {
        return None;
    }
    Some(path)
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
        let normalized_path = normalize_source_selector(path);
        for matched in registry.profiles_for_selector(normalized_path) {
            if !should_block(matched.profile) {
                continue;
            }
            if matches.iter().any(|(_, existing)| {
                existing.profile.language_id == matched.profile.language_id
                    && existing.profile.provider_id == matched.profile.provider_id
            }) {
                continue;
            }
            matches.push((normalized_path, matched));
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
        SourceSelectorKind::ExactPath => direct_source_query_route(profile, path),
        SourceSelectorKind::Pattern => profile.route_from_template(
            DecisionRouteKind::Prime,
            &profile.commands.prime,
            None,
            None,
        ),
    }
}

fn direct_source_query_route(profile: &LanguageProfile, path: &str) -> DecisionRoute {
    let mut argv = provider_command_argv(profile);
    argv.extend([
        "query".to_string(),
        "--from-hook".to_string(),
        "direct-source-read".to_string(),
        "--selector".to_string(),
        path.to_string(),
    ]);
    argv.push(".".to_string());
    DecisionRoute {
        language_id: profile.language_id.clone(),
        provider_id: profile.provider_id.clone(),
        binary: profile.binary.clone(),
        kind: DecisionRouteKind::Query,
        argv,
        stdin_mode: None,
    }
}

fn provider_command_argv(profile: &LanguageProfile) -> Vec<String> {
    if profile.provider_command_prefix.is_empty() {
        return vec![profile.binary.clone()];
    }
    profile.provider_command_prefix.clone()
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
    let plan = raw_search_plan(registry, tokens)?;
    let raw_search_profiles = plan
        .profiles
        .into_iter()
        .filter(|profile| profile.policy.blocks_raw_source_search())
        .collect::<Vec<_>>();
    if raw_search_profiles.is_empty() || contains_ingest_pipe(tokens, &raw_search_profiles) {
        return None;
    }
    let terms = plan.terms;
    let routes: Vec<DecisionRoute> = raw_search_profiles
        .iter()
        .map(|profile| {
            if terms.is_empty() {
                return raw_search_ingest_route(profile);
            }
            search_query_route(profile, &terms).unwrap_or_else(|| raw_search_ingest_route(profile))
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

fn raw_search_ingest_route(profile: &LanguageProfile) -> DecisionRoute {
    profile.route_from_template(
        DecisionRouteKind::Ingest,
        &profile.commands.ingest,
        None,
        None,
    )
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
