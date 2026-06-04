//! Root semantic agent hook classifier over activated providers.
#[path = "classifier_inline_source_read.rs"]
mod inline_source_read;

use serde_json::Value;

use crate::command::looks_like_command_transcript;
use crate::command::{
    CommandIntent, apply_patch_source_paths, command_intent, contains_ingest_pipe,
    direct_source_query_route, infer_query_from_path, path_like_tokens, raw_search_plan,
    search_json_route, search_query_route, search_query_route_for_selector, semantic_shell_tokens,
};
use crate::{
    ActivatedProvider, ClientHookConfig, DecisionKind, DecisionRoute, DecisionRouteKind,
    DecisionSubject, HOOK_DECISION_SCHEMA_ID, HOOK_DECISION_SCHEMA_VERSION, HOOK_PROTOCOL_ID,
    HOOK_PROTOCOL_VERSION, HookDecision, HookRuntime, OperationIntent, ReasonKind,
    SourceSelectorKind, SourceSelectorMatch, ToolAction, collect_source_selector_matches,
    collect_tool_actions, payload_string, subject_for_action,
};

/// Named input for hook classification with optional client policy config.
pub struct HookClassificationRequest<'a> {
    /// Activated provider runtime for the current project.
    pub registry: &'a HookRuntime,
    /// Project-local client rules layered over the built-in classifier.
    pub config: &'a ClientHookConfig,
    /// Hook client identifier such as `codex`.
    pub platform: &'a str,
    /// Canonical hook event name such as `pre-tool`.
    pub event: &'a str,
    /// Raw platform hook payload.
    pub payload: &'a Value,
}

/// Classify one platform hook payload against an activated provider runtime.
pub fn classify_hook(
    registry: &HookRuntime,
    platform: &str,
    event: &str,
    payload: &Value,
) -> HookDecision {
    classify_hook_with_config(HookClassificationRequest {
        registry,
        config: &ClientHookConfig::default(),
        platform,
        event,
        payload,
    })
}

/// Classify one hook payload using a named `HookClassificationRequest`.
pub fn classify_hook_with_config(request: HookClassificationRequest<'_>) -> HookDecision {
    let HookClassificationRequest {
        registry,
        config,
        platform,
        event,
        payload,
    } = request;
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
        .find_map(|action| config.classify(registry, platform, event, action))
    {
        return decision;
    }

    if let Some(decision) = actions.iter().find_map(|action| {
        classify_structured_apply_patch_action(registry, platform, event, action)
    }) {
        return decision;
    }

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
        fields: std::collections::BTreeMap::new(),
    })
}

fn classify_direct_read_action(
    registry: &HookRuntime,
    platform: &str,
    event: &str,
    action: &ToolAction,
) -> Option<HookDecision> {
    fn directory_path_matches_provider(
        project_root: &str,
        provider: &ActivatedProvider,
        path: &str,
    ) -> bool {
        let Some(normalized) = normalize_directory_path(path) else {
            return false;
        };

        if let Some(project_relative) = project_relative_directory(project_root, &normalized) {
            return directory_path_candidate_matches_provider(provider, &project_relative);
        }

        if project_root.trim().is_empty() || project_root.trim() == "." {
            if let Ok(current_dir) = std::env::current_dir() {
                if let Some(current_dir) = current_dir.to_str() {
                    if let Some(project_relative) =
                        project_relative_directory(current_dir, &normalized)
                    {
                        return directory_path_candidate_matches_provider(
                            provider,
                            &project_relative,
                        );
                    }
                }
            }
        }

        if !std::path::Path::new(&normalized).is_absolute() {
            return directory_path_candidate_matches_provider(provider, &normalized);
        }

        provider.matches_search_token(&normalized)
    }

    fn normalize_directory_path(path: &str) -> Option<String> {
        let path = path.trim().trim_start_matches("./").trim_end_matches('/');
        if path.is_empty() {
            return None;
        }

        let absolute = path.starts_with('/');
        let mut segments = Vec::new();
        for segment in path.split('/') {
            match segment {
                "" | "." => {}
                ".." => {
                    segments.pop()?;
                }
                segment => segments.push(segment),
            }
        }

        let joined = segments.join("/");
        if absolute {
            Some(format!("/{joined}"))
        } else if joined.is_empty() {
            Some(".".to_string())
        } else {
            Some(joined)
        }
    }

    fn project_relative_directory(project_root: &str, normalized: &str) -> Option<String> {
        let project_root = normalize_directory_path(project_root)?;
        if project_root == "." {
            return None;
        }
        if normalized == project_root {
            return Some(".".to_string());
        }
        normalized
            .strip_prefix(&project_root)
            .and_then(|path| path.strip_prefix('/'))
            .filter(|path| !path.is_empty())
            .map(str::to_string)
    }

    fn directory_path_candidate_matches_provider(
        provider: &ActivatedProvider,
        candidate: &str,
    ) -> bool {
        if provider.matches_search_token(candidate) {
            return true;
        }

        provider.source_roots.iter().any(|root| {
            let Some(root) = normalize_directory_path(root) else {
                return false;
            };
            !root.is_empty()
                && (candidate == root
                    || candidate.starts_with(&format!("{root}/"))
                    || candidate.ends_with(&format!("/{root}"))
                    || candidate.contains(&format!("/{root}/")))
        })
    }

    if action.operation == OperationIntent::DirectoryRead {
        let providers = registry
            .providers
            .iter()
            .filter(|provider| provider.policy.blocks_raw_source_search())
            .filter(|provider| {
                action.paths.iter().any(|path| {
                    directory_path_matches_provider(&registry.project_root, provider, path)
                })
            })
            .collect::<Vec<_>>();

        if providers.is_empty() {
            return None;
        }

        let routes = providers
            .iter()
            .map(|provider| raw_search_ingest_route(provider))
            .collect();
        let message = provider_guide_message("source-directory-enumeration denied", &providers);
        return Some(deny_for_action(
            platform,
            event,
            ReasonKind::SourceDirectoryEnumeration,
            action,
            language_ids(&providers),
            subject_for_action(action),
            routes,
            message,
        ));
    }

    if !action.operation.is_direct_read_candidate()
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
    classify_apply_patch_command(registry, platform, event, action, command)
        .or_else(|| classify_search_json_command(registry, platform, event, action, &tokens))
        .or_else(|| {
            classify_source_read_command(registry, platform, event, action, command, &tokens)
        })
        .or_else(|| classify_raw_search_command(registry, platform, event, action, &tokens))
}

fn classify_apply_patch_command(
    registry: &HookRuntime,
    platform: &str,
    event: &str,
    action: &ToolAction,
    command: &str,
) -> Option<HookDecision> {
    let patch_paths = apply_patch_source_paths(&action.tool_name, command);
    if patch_paths.is_empty() {
        return None;
    }
    classify_apply_patch_paths(registry, platform, event, action, patch_paths)
}

fn classify_structured_apply_patch_action(
    registry: &HookRuntime,
    platform: &str,
    event: &str,
    action: &ToolAction,
) -> Option<HookDecision> {
    if action.operation != OperationIntent::ApplyPatch || action.paths.is_empty() {
        return None;
    }
    classify_apply_patch_paths(registry, platform, event, action, action.paths.clone())
}

fn classify_apply_patch_paths(
    registry: &HookRuntime,
    platform: &str,
    event: &str,
    action: &ToolAction,
    patch_paths: Vec<String>,
) -> Option<HookDecision> {
    let matches =
        collect_source_selector_matches(registry, patch_paths.iter().map(String::as_str), |_| true);
    if matches.is_empty() {
        return None;
    }

    let routes = direct_read_routes(&matches);
    let mut subject = subject_for_action(action);
    subject.paths = patch_paths;
    let languages = direct_read_language_ids(&matches);

    if let Some(command) = action.command.as_deref() {
        let patch_digest = source_apply_patch_digest(command);
        let authorization_path = source_apply_patch_authorization_path(registry, &patch_digest);
        if authorization_path.is_file() {
            let mut decision = allow(platform, event, subject);
            decision.language_ids = languages;
            decision.message = format!(
                "source apply_patch allowed by controlled maintenance authorization {}",
                authorization_path.display()
            );
            decision.fields.insert(
                "toolSurface".to_string(),
                Value::String(action.surface.as_str().to_string()),
            );
            decision.fields.insert(
                "operationIntent".to_string(),
                Value::String(action.operation.as_str().to_string()),
            );
            decision.fields.insert(
                "maintenancePolicy".to_string(),
                Value::String("source-apply-patch-authorization".to_string()),
            );
            decision
                .fields
                .insert("patchDigest".to_string(), Value::String(patch_digest));
            decision.fields.insert(
                "authorizationPath".to_string(),
                Value::String(authorization_path.display().to_string()),
            );
            return Some(decision);
        }
    }

    let language = languages
        .first()
        .map(String::as_str)
        .unwrap_or("<language>");
    let project_root = routes
        .first()
        .and_then(|route| route.argv.last())
        .filter(|arg| !arg.starts_with('-'))
        .map(String::as_str)
        .unwrap_or(".");
    let route_guide = routes
        .iter()
        .map(|route| command_line(&route.argv))
        .collect::<Vec<_>>()
        .join("; ");
    let message = format!(
        "source apply_patch denied; compact output is not patch context. Exact-read route: {route_guide}. Build semantic-ast-patch.json with `asp ast-patch template --language {language} --owner <owner-path> --read <path:start:end> --op <operation> --snippet '<exact source or inserted snippet>' {project_root}` and validate it with `asp {language} ast-patch dry-run --packet semantic-ast-patch.json {project_root}`. Receipt verification records edit intent; this hook does not auto-unlock source apply_patch. If provider dry-run cannot verify this operation, use a provider-native mutation route or a controlled maintenance policy."
    );
    Some(deny_for_action(
        platform,
        event,
        ReasonKind::SemanticAstPatchRequired,
        action,
        languages,
        subject,
        routes,
        message,
    ))
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
    Some(deny_for_action(
        platform,
        event,
        ReasonKind::AgentSearchJson,
        action,
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
        binary: "asp".to_string(),
        kind: DecisionRouteKind::Fzf,
        argv: provider.agent_facade_argv_from_provider_argv(argv),
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
    let inline_source_read_paths = inline_source_read::source_read_paths(command, tokens);
    if !inline_source_read_paths.is_empty() {
        let matches = collect_content_dump_matches(
            registry,
            inline_source_read_paths.iter().map(String::as_str),
        );
        if !matches.is_empty() {
            return Some(content_dump_decision(
                platform,
                event,
                action,
                matches,
                Some(inline_source_read_paths),
            ));
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
            Some(content_dump_decision(
                platform, event, action, matches, None,
            ))
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
    subject_paths: Option<Vec<String>>,
) -> HookDecision {
    let routes = direct_read_routes(&matches);
    let providers = providers_from_matches(&matches);
    let message = provider_guide_message("bulk-source-dump denied", &providers);
    let mut subject = subject_for_action(action);
    if let Some(paths) = subject_paths {
        subject.paths = paths;
    }
    deny_for_action(
        platform,
        event,
        ReasonKind::BulkSourceDump,
        action,
        direct_read_language_ids(&matches),
        subject,
        routes,
        message,
    )
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
    Some(deny_for_action(
        platform,
        event,
        ReasonKind::DirectSourceRead,
        action,
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
    provider.agent_facade_argv_from_provider_argv(argv)
}

fn direct_read_route(
    provider: &ActivatedProvider,
    path: &str,
    selector_kind: SourceSelectorKind,
) -> DecisionRoute {
    match selector_kind {
        SourceSelectorKind::ExactPath => direct_source_query_route(provider, path),
        SourceSelectorKind::Pattern => {
            let route_context = provider.route_path_context(path);
            search_query_route_for_selector(
                provider,
                &route_context.selector,
                &route_context.project_root,
                &[],
            )
            .unwrap_or_else(|| {
                provider.route_from_template(
                    DecisionRouteKind::Prime,
                    &provider.routes.prime,
                    None,
                    None,
                )
            })
        }
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
    Some(deny_for_action(
        platform,
        event,
        ReasonKind::RawBroadSearch,
        action,
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
        fields: std::collections::BTreeMap::new(),
    }
}

fn deny_for_action(
    platform: &str,
    event: &str,
    reason_kind: ReasonKind,
    action: &ToolAction,
    language_ids: Vec<String>,
    subject: DecisionSubject,
    routes: Vec<DecisionRoute>,
    message: String,
) -> HookDecision {
    let mut decision = deny(
        platform,
        event,
        reason_kind,
        language_ids,
        subject,
        routes,
        message,
    );
    decision.fields.insert(
        "toolSurface".to_string(),
        Value::String(action.surface.as_str().to_string()),
    );
    decision.fields.insert(
        "operationIntent".to_string(),
        Value::String(action.operation.as_str().to_string()),
    );
    decision
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
        fields: std::collections::BTreeMap::new(),
    }
}

fn source_apply_patch_digest(command: &str) -> String {
    let digest = <sha2::Sha256 as sha2::Digest>::digest(command.as_bytes());
    format!("{digest:x}")
}

fn source_apply_patch_authorization_path(
    registry: &HookRuntime,
    patch_digest: &str,
) -> std::path::PathBuf {
    std::path::Path::new(&registry.project_root)
        .join(".cache")
        .join("agent-semantic-protocol")
        .join("hooks")
        .join("source-apply-patch")
        .join(format!("{patch_digest}.json"))
}
