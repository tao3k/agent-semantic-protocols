//! Root semantic agent hook classifier over activated providers.

use serde_json::Value;

use crate::command::looks_like_command_transcript;
use crate::command::{
    CommandIntent, command_intent, contains_ingest_pipe, infer_query_from_path, path_like_tokens,
    raw_search_plan, search_json_route, search_query_route, semantic_shell_tokens,
};
use crate::protocol::{
    DecisionKind, DecisionRoute, DecisionRouteKind, DecisionSubject, HOOK_DECISION_SCHEMA_ID,
    HOOK_DECISION_SCHEMA_VERSION, HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, HookDecision,
    ReasonKind, normalize_source_route_selector,
};
use crate::protocol_activation::{ActivatedProvider, HookRuntime, SourceSelectorKind};
use crate::source_selector::{SourceSelectorMatch, collect_source_selector_matches};
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

/// Classify one platform hook payload against an activated provider runtime.
pub fn classify_hook(
    registry: &HookRuntime,
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
        .or_else(|| payload.get("parameters"))
        .or_else(|| payload.get("input"))
        .or_else(|| payload.get("arguments"))
        .unwrap_or(payload);
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
    registry: &HookRuntime,
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
    registry: &HookRuntime,
    platform: &str,
    event: &str,
    action: &ToolAction,
) -> Option<HookDecision> {
    let command = action.command.as_deref()?;
    let tokens = semantic_shell_tokens(command);
    classify_search_json_command(registry, platform, event, action, &tokens)
        .or_else(|| {
            classify_source_read_command(registry, platform, event, action, command, &tokens)
        })
        .or_else(|| classify_raw_search_command(registry, platform, event, action, &tokens))
}

fn classify_search_json_command(
    registry: &HookRuntime,
    platform: &str,
    event: &str,
    action: &ToolAction,
    tokens: &[String],
) -> Option<HookDecision> {
    if !tokens.iter().any(|token| token == "--json") {
        return None;
    }
    let (provider, argv) = search_json_route(registry, tokens)?;
    if !provider.policy.blocks_agent_search_json() {
        return None;
    }
    let route = search_json_decision_route(provider, argv);
    let message = format!(
        "agent-search-json denied; route: {}",
        command_line(&route.argv)
    );
    let routes = vec![route];
    Some(deny(
        platform,
        event,
        ReasonKind::AgentSearchJson,
        vec![provider.language_id.clone()],
        subject_for_action(action),
        routes,
        message,
    ))
}

fn search_json_decision_route(provider: &ActivatedProvider, argv: Vec<String>) -> DecisionRoute {
    if let Some(path) = search_json_owner_path(&argv).map(str::to_string) {
        let query = infer_query_from_path(&path);
        return provider.route_from_template(
            DecisionRouteKind::Owner,
            &provider.routes.owner,
            Some(&path),
            query.as_deref(),
        );
    }
    DecisionRoute {
        language_id: provider.language_id.clone(),
        provider_id: provider.provider_id.clone(),
        binary: provider.binary.clone(),
        kind: DecisionRouteKind::Fzf,
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
    registry: &HookRuntime,
    platform: &str,
    event: &str,
    action: &ToolAction,
    command: &str,
    tokens: &[String],
) -> Option<HookDecision> {
    let python_source_read_paths = python_source_read_paths(command, tokens);
    if !python_source_read_paths.is_empty() {
        let matches = collect_content_dump_matches(
            registry,
            python_source_read_paths.iter().map(String::as_str),
        );
        if !matches.is_empty() {
            return Some(content_dump_decision(platform, event, action, matches));
        }
    }

    match command_intent(tokens) {
        CommandIntent::DirectRead => direct_read_decision(
            registry,
            platform,
            event,
            action,
            collect_direct_read_matches(registry, action_path_selectors(action, tokens)),
        ),
        CommandIntent::ContentDump => {
            let matches =
                collect_content_dump_matches(registry, action_path_selectors(action, tokens));
            if matches.is_empty() {
                return None;
            }
            Some(content_dump_decision(platform, event, action, matches))
        }
        _ => None,
    }
}

fn action_path_selectors<'a>(action: &'a ToolAction, tokens: &'a [String]) -> Vec<&'a str> {
    let mut selectors = Vec::new();
    for path in &action.paths {
        push_unique_selector(&mut selectors, path);
    }
    for path in path_like_tokens(tokens) {
        push_unique_selector(&mut selectors, path);
    }
    selectors
}

fn push_unique_selector<'a>(selectors: &mut Vec<&'a str>, selector: &'a str) {
    if !selectors.iter().any(|existing| existing == &selector) {
        selectors.push(selector);
    }
}

fn content_dump_decision(
    platform: &str,
    event: &str,
    action: &ToolAction,
    matches: Vec<DirectReadMatch<'_>>,
) -> HookDecision {
    let routes = direct_read_routes(&matches);
    let providers = providers_from_matches(&matches);
    let message = provider_guide_message("bulk-source-dump denied", &providers);
    deny(
        platform,
        event,
        ReasonKind::BulkSourceDump,
        direct_read_language_ids(&matches),
        subject_for_action(action),
        routes,
        message,
    )
}

fn python_source_read_paths(command: &str, tokens: &[String]) -> Vec<String> {
    if !tokens
        .first()
        .is_some_and(|token| is_python_interpreter_command(token))
        || !python_source_read_api(command)
    {
        return Vec::new();
    }
    quoted_path_literals(command)
}

fn is_python_interpreter_command(token: &str) -> bool {
    let name = token.rsplit('/').next().unwrap_or(token);
    name == "python"
        || name == "python3"
        || name == "py"
        || name
            .strip_prefix("python3.")
            .is_some_and(|suffix| suffix.chars().all(|ch| ch.is_ascii_digit() || ch == '.'))
}

fn python_source_read_api(command: &str) -> bool {
    command.contains(".read_text(")
        || command.contains(".read_bytes(")
        || (command.contains("open(") && command.contains(".read("))
}

fn quoted_path_literals(command: &str) -> Vec<String> {
    let mut paths = Vec::new();
    let mut chars = command.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\'' && ch != '"' {
            continue;
        }
        let quote = ch;
        let mut literal = String::new();
        let mut escaped = false;
        for current in chars.by_ref() {
            if escaped {
                literal.push(current);
                escaped = false;
                continue;
            }
            if current == '\\' {
                escaped = true;
                continue;
            }
            if current == quote {
                break;
            }
            literal.push(current);
        }
        let normalized = normalize_source_route_selector(&literal);
        if is_path_like_literal(normalized) {
            paths.push(normalized.to_string());
        }
    }
    paths
}

fn is_path_like_literal(literal: &str) -> bool {
    !literal.starts_with('-')
        && (literal.contains('/') || literal.contains('*'))
        && literal.contains('.')
}

fn direct_read_decision(
    _registry: &HookRuntime,
    platform: &str,
    event: &str,
    action: &ToolAction,
    matches: Vec<DirectReadMatch<'_>>,
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

type DirectReadMatch<'provider> = SourceSelectorMatch<'provider>;

fn collect_direct_read_matches<'provider, I, S>(
    registry: &'provider HookRuntime,
    paths: I,
) -> Vec<DirectReadMatch<'provider>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    collect_source_selector_matches(registry, paths, |provider| {
        provider.policy.blocks_direct_source_read()
    })
}

fn collect_content_dump_matches<'provider, I, S>(
    registry: &'provider HookRuntime,
    paths: I,
) -> Vec<DirectReadMatch<'provider>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    collect_source_selector_matches(registry, paths, |provider| {
        provider.policy.blocks_bulk_source_dump()
    })
}

fn direct_read_routes(matches: &[DirectReadMatch<'_>]) -> Vec<DecisionRoute> {
    matches
        .iter()
        .map(|matched| direct_read_route(matched.provider, &matched.route_selector, matched.kind))
        .collect()
}

fn direct_read_language_ids(matches: &[DirectReadMatch<'_>]) -> Vec<String> {
    matches
        .iter()
        .map(|matched| matched.provider.language_id.clone())
        .collect()
}

fn direct_read_decision_message(
    matches: &[DirectReadMatch<'_>],
    routes: &[DecisionRoute],
) -> String {
    let providers = providers_from_matches(matches);
    if !matches
        .iter()
        .any(|matched| matched.kind == SourceSelectorKind::ExactPath)
    {
        return provider_guide_message("direct-source-read denied", &providers);
    }
    let rendered_routes = routes
        .iter()
        .map(|route| command_line(&route.argv))
        .collect::<Vec<_>>();
    if rendered_routes.is_empty() {
        return provider_guide_message("direct-source-read denied", &providers);
    }
    format!(
        "direct-source-read denied; route: {}",
        rendered_routes.join("; ")
    )
}

fn providers_from_matches<'a>(matches: &[DirectReadMatch<'a>]) -> Vec<&'a ActivatedProvider> {
    matches.iter().map(|matched| matched.provider).collect()
}

fn provider_guide_message(reason: &str, providers: &[&ActivatedProvider]) -> String {
    let mut rendered_guides = Vec::<String>::new();
    let mut seen_providers = Vec::<(&str, &str)>::new();
    for provider in providers {
        if seen_providers.iter().any(|(language_id, provider_id)| {
            *language_id == provider.language_id && *provider_id == provider.provider_id
        }) {
            continue;
        }
        seen_providers.push((provider.language_id.as_str(), provider.provider_id.as_str()));
        rendered_guides.push(format!(
            "{} => {}",
            provider.provider_id,
            command_line(&provider_guide_argv(provider))
        ));
    }
    if rendered_guides.is_empty() {
        return format!("{reason}; provider guide unavailable");
    }
    format!("{reason}; provider guide: {}", rendered_guides.join("; "))
}

fn provider_guide_argv(provider: &ActivatedProvider) -> Vec<String> {
    let argv = provider
        .routes
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
                provider.binary.clone(),
                "agent".to_string(),
                "guide".to_string(),
                ".".to_string(),
            ]
        });
    if !provider.provider_command_prefix.is_empty()
        && argv
            .first()
            .is_some_and(|command| command == &provider.binary)
    {
        return provider
            .provider_command_prefix
            .iter()
            .cloned()
            .chain(argv.into_iter().skip(1))
            .collect();
    }
    argv
}

fn direct_read_route(
    provider: &ActivatedProvider,
    path: &str,
    selector_kind: SourceSelectorKind,
) -> DecisionRoute {
    match selector_kind {
        SourceSelectorKind::ExactPath => direct_source_query_route(provider, path),
        SourceSelectorKind::Pattern => provider.route_from_template(
            DecisionRouteKind::Prime,
            &provider.routes.prime,
            None,
            None,
        ),
    }
}

fn direct_source_query_route(provider: &ActivatedProvider, path: &str) -> DecisionRoute {
    let route_context = provider.route_path_context(path);
    let mut argv = provider_command_argv(provider);
    argv.extend([
        "query".to_string(),
        "--from-hook".to_string(),
        "direct-source-read".to_string(),
        "--selector".to_string(),
        route_context.selector,
        "--code".to_string(),
    ]);
    argv.push(route_context.project_root);
    DecisionRoute {
        language_id: provider.language_id.clone(),
        provider_id: provider.provider_id.clone(),
        binary: provider.binary.clone(),
        kind: DecisionRouteKind::Query,
        argv,
        stdin_mode: None,
    }
}

fn provider_command_argv(provider: &ActivatedProvider) -> Vec<String> {
    if provider.provider_command_prefix.is_empty() {
        return vec![provider.binary.clone()];
    }
    provider.provider_command_prefix.clone()
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
    registry: &HookRuntime,
    platform: &str,
    event: &str,
    action: &ToolAction,
    tokens: &[String],
) -> Option<HookDecision> {
    if command_intent(tokens) != CommandIntent::RawSearch {
        return None;
    }
    let plan = raw_search_plan(registry, tokens)?;
    let raw_search_providers = plan
        .providers
        .into_iter()
        .filter(|provider| provider.policy.blocks_raw_source_search())
        .collect::<Vec<_>>();
    if raw_search_providers.is_empty() || contains_ingest_pipe(tokens, &raw_search_providers) {
        return None;
    }
    let terms = plan.terms;
    let routes: Vec<DecisionRoute> = raw_search_providers
        .iter()
        .map(|provider| {
            if terms.is_empty() {
                return raw_search_ingest_route(provider);
            }
            search_query_route(provider, &terms)
                .unwrap_or_else(|| raw_search_ingest_route(provider))
        })
        .collect();
    let message = provider_guide_message("raw-broad-search denied", &raw_search_providers);
    Some(deny(
        platform,
        event,
        ReasonKind::RawBroadSearch,
        language_ids(&raw_search_providers),
        subject_for_action(action),
        routes,
        message,
    ))
}

fn raw_search_ingest_route(provider: &ActivatedProvider) -> DecisionRoute {
    provider.route_from_template(
        DecisionRouteKind::Ingest,
        &provider.routes.ingest,
        None,
        None,
    )
}

fn language_ids(providers: &[&ActivatedProvider]) -> Vec<String> {
    providers
        .iter()
        .map(|provider| provider.language_id.clone())
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
