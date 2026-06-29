//! Hook classifier orchestration for `agent-semantic-hook`.

use serde_json::Value;

use crate::command::{apply_patch_source_paths, infer_query_from_path, search_json_route};

use super::agent_org_artifacts::with_agent_org_artifact_recovery;
use super::decision::{allow, deny_for_action};
use super::prompt_search_flow::classify_prompt_search_flow_feedback;
use super::recovery::command_line;
use super::source_access_routes::{
    classify_direct_read_action, classify_raw_search_command, classify_source_read_command,
    direct_read_language_ids, direct_read_routes,
};
use crate::event_state::missing_search_pipe_after_prime;
use crate::{
    ActivatedProvider, ClientHookConfig, DecisionKind, DecisionRoute, DecisionRouteKind,
    DecisionSubject, HOOK_DECISION_SCHEMA_ID, HOOK_DECISION_SCHEMA_VERSION, HOOK_PROTOCOL_ID,
    HOOK_PROTOCOL_VERSION, HookDecision, HookRuntime, OperationIntent, ReasonKind, ToolAction,
    collect_source_selector_matches, collect_tool_actions, payload_string, subject_for_action,
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
    let decision = if let Some(decision) = classify_non_tool_event(&request) {
        decision
    } else {
        let actions = collect_payload_tool_actions(request.payload);
        if let Some(decision) = classify_tool_actions(&request, &actions) {
            decision
        } else {
            let subject = actions.first().map(subject_for_action).unwrap_or_default();
            allow(request.platform, request.event, subject)
        }
    };
    let decision = with_prompt_scope_fields(decision, request.payload);
    with_agent_org_artifact_recovery(decision, request.config, &request.registry.project_root)
}

fn with_prompt_scope_fields(mut decision: HookDecision, payload: &Value) -> HookDecision {
    if let Some(session_id) =
        payload_string(payload, "session_id").or_else(|| payload_string(payload, "sessionId"))
    {
        decision
            .fields
            .entry("sessionId".to_string())
            .or_insert_with(|| Value::String(session_id));
    }
    if let Some(transcript_path) = payload_string(payload, "transcript_path")
        .or_else(|| payload_string(payload, "transcriptPath"))
    {
        decision
            .fields
            .entry("transcriptPath".to_string())
            .or_insert_with(|| Value::String(transcript_path));
    }
    decision
}

fn collect_payload_tool_actions(payload: &Value) -> Vec<ToolAction> {
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
    collect_tool_actions(&tool_name, tool_input)
}

fn classify_non_tool_event(request: &HookClassificationRequest<'_>) -> Option<HookDecision> {
    classify_stop(
        request.registry,
        request.platform,
        request.event,
        request.payload,
    )
    .or_else(|| classify_subagent_stop(request.platform, request.event, request.payload))
    .or_else(|| classify_user_prompt(request.platform, request.event, request.payload))
}

fn classify_tool_actions(
    request: &HookClassificationRequest<'_>,
    actions: &[ToolAction],
) -> Option<HookDecision> {
    let HookClassificationRequest {
        registry,
        config,
        platform,
        event,
        payload,
    } = request;
    if let Some(decision) = actions
        .iter()
        .find_map(|action| config.classify(registry, platform, event, action))
    {
        return Some(decision);
    }
    if let Some(decision) = actions
        .iter()
        .find_map(|action| classify_invalid_asp_facade(registry, platform, event, action))
    {
        return Some(decision);
    }
    if let Some(decision) = actions.iter().find_map(|action| {
        classify_prompt_search_flow_feedback(registry, platform, event, payload, action)
    }) {
        return Some(decision);
    }

    if config.semantic_ast_patch_enabled()
        && let Some(decision) = actions.iter().find_map(|action| {
            classify_structured_apply_patch_action(registry, platform, event, action)
        })
    {
        return Some(decision);
    }

    if let Some(decision) = actions.iter().find_map(|action| {
        classify_direct_read_action(
            registry,
            platform,
            event,
            action,
            config.semantic_ast_patch_enabled(),
            config.recovery_prompt(),
        )
    }) {
        return Some(decision);
    }
    if let Some(decision) = actions.iter().find_map(|action| {
        classify_command_action(
            registry,
            platform,
            event,
            action,
            config.semantic_ast_patch_enabled(),
            config.recovery_prompt(),
        )
    }) {
        return Some(decision);
    }
    None
}

fn classify_invalid_asp_facade(
    registry: &HookRuntime,
    platform: &str,
    event: &str,
    action: &ToolAction,
) -> Option<HookDecision> {
    if event != "pre-tool" {
        return None;
    }
    if !action_supports_asp_command_feedback(action) {
        return None;
    }
    action.command.as_deref()?;
    let command_tokens = action.command_tokens()?;
    let invalid_facade = invalid_asp_facade_from_tokens(&command_tokens, registry)?;
    let preferred_language = preferred_language_for_invalid_facade(&invalid_facade, registry);
    let mut fields = std::collections::BTreeMap::new();
    fields.insert(
        "hookFeedback".to_string(),
        Value::String("invalid-asp-facade".to_string()),
    );
    fields.insert(
        "invalidFacade".to_string(),
        Value::String(invalid_facade.clone()),
    );
    if let Some(language_id) = preferred_language.as_deref() {
        fields.insert(
            "languageId".to_string(),
            Value::String(language_id.to_string()),
        );
    }
    Some(HookDecision {
        schema_id: HOOK_DECISION_SCHEMA_ID,
        schema_version: HOOK_DECISION_SCHEMA_VERSION,
        protocol_id: HOOK_PROTOCOL_ID,
        protocol_version: HOOK_PROTOCOL_VERSION,
        platform: platform.to_string(),
        event: event.to_string(),
        decision: DecisionKind::Deny,
        reason_kind: ReasonKind::None,
        language_ids: preferred_language.iter().cloned().collect(),
        subject: subject_for_action(action),
        routes: Vec::new(),
        message: invalid_asp_facade_message(
            &invalid_facade,
            preferred_language.as_deref(),
            registry,
        ),
        fields,
    })
}

fn action_supports_asp_command_feedback(action: &ToolAction) -> bool {
    matches!(
        action.operation,
        OperationIntent::ShellCommand | OperationIntent::StdinContinuation
    )
}

fn invalid_asp_facade_from_tokens(tokens: &[String], registry: &HookRuntime) -> Option<String> {
    let asp_index = tokens.iter().enumerate().find_map(|(index, token)| {
        is_asp_binary_token(token)
            .then_some(index)
            .filter(|index| is_asp_invocation_position(tokens, *index))
    })?;
    let facade = tokens.get(asp_index + 1)?;
    if facade.starts_with('-')
        || is_root_asp_command(facade)
        || registry
            .providers
            .iter()
            .any(|provider| provider.language_id == *facade)
    {
        return None;
    }
    Some(facade.clone())
}

fn is_asp_binary_token(token: &str) -> bool {
    token == "asp" || token.ends_with("/asp") || token.ends_with(".bin/asp")
}

fn is_asp_invocation_position(tokens: &[String], index: usize) -> bool {
    if index == 0 {
        return true;
    }
    let previous = tokens[index - 1].as_str();
    if matches!(previous, "&&" | ";" | "|" | "||" | "rtk") {
        return true;
    }
    if tokens[..index].iter().all(|token| is_env_assignment(token)) {
        return true;
    }
    if is_env_assignment(previous) {
        return true;
    }
    index >= 3 && tokens[index - 3] == "direnv" && tokens[index - 2] == "exec"
}

fn is_env_assignment(token: &str) -> bool {
    let Some((name, _value)) = token.split_once('=') else {
        return false;
    };
    !name.is_empty()
        && !name.starts_with('-')
        && name
            .chars()
            .all(|character| character == '_' || character.is_ascii_alphanumeric())
}

fn is_root_asp_command(value: &str) -> bool {
    matches!(
        value,
        "agent"
            | "guide"
            | "providers"
            | "tools"
            | "wrap"
            | "cache"
            | "cloud"
            | "hook"
            | "state"
            | "plugin"
            | "install"
            | "sync"
            | "paths"
            | "healthcheck"
            | "source-access"
            | "ast-patch"
            | "graph"
            | "fd"
            | "rg"
            | "search"
            | "query"
            | "check"
    )
}

fn preferred_language_for_invalid_facade(
    invalid_facade: &str,
    registry: &HookRuntime,
) -> Option<String> {
    if invalid_facade.eq_ignore_ascii_case("effect")
        && registry
            .providers
            .iter()
            .any(|provider| provider.language_id == "typescript")
    {
        return Some("typescript".to_string());
    }
    None
}

fn invalid_asp_facade_message(
    invalid_facade: &str,
    preferred_language: Option<&str>,
    registry: &HookRuntime,
) -> String {
    let active_languages = registry
        .providers
        .iter()
        .map(|provider| provider.language_id.as_str())
        .collect::<Vec<_>>()
        .join(",");
    let mut lines = vec![
        format!("ASP hook denied unknown ASP facade `{invalid_facade}`."),
        "ASP facades are language IDs, not package or library names.".to_string(),
        format!("Active language facades: {active_languages}."),
    ];
    if let Some(language_id) = preferred_language {
        lines.push(format!("Suggested matching facade: {language_id}."));
    }
    lines.extend([String::new(), "## Run Next".to_string()]);
    if let Some(language_id) = preferred_language {
        lines.push(format!(
            "Choose the narrowest `asp {language_id}` route from the current evidence state: owner/reasoning/query for known anchors, `search prime --workspace . --view seeds` only when the owner map is unknown, and `search pipe '<question-or-feature-term>' --workspace . --view seeds` only for ambiguous query refinement."
        ));
    } else {
        lines.extend([
            "asp providers".to_string(),
            "asp fd -query '<path-or-language-term>' '.'".to_string(),
            "asp rg -query '<feature-term>' '<bounded-scope>'".to_string(),
        ]);
    }
    lines.extend([
        String::new(),
        "## Rules".to_string(),
        "Only run `asp <language> search|query` when the facade is listed and matches the target language.".to_string(),
        "Do not switch to an unrelated active facade just because it is the only provider in this repository.".to_string(),
        "For unsupported target-language files, use provider-neutral finder commands or install/activate a matching provider.".to_string(),
        "For the Effect package, use the TypeScript facade: `asp typescript ...`."
            .to_string(),
    ]);
    lines.join("\n")
}

fn classify_user_prompt(platform: &str, event: &str, payload: &Value) -> Option<HookDecision> {
    if event != "user-prompt" {
        return None;
    }
    let prompt = payload_string(payload, "prompt").unwrap_or_default();
    let mut decision = allow(platform, event, DecisionSubject::default());
    if prompt_is_locator_only(&prompt) {
        decision.fields.insert(
            "promptWorkflow".to_string(),
            Value::String("locator-only".to_string()),
        );
    }
    Some(decision)
}

fn prompt_is_locator_only(prompt: &str) -> bool {
    let prompt = prompt.to_ascii_lowercase();
    (prompt.contains("where ")
        || prompt.contains("locate")
        || prompt.contains("located")
        || prompt.contains("selecting files")
        || prompt.contains("before selecting"))
        && !prompt.contains("show code")
        && !prompt.contains("read code")
        && !prompt.contains("extract code")
}

fn classify_stop(
    registry: &HookRuntime,
    platform: &str,
    event: &str,
    payload: &Value,
) -> Option<HookDecision> {
    if event != "stop" {
        return None;
    }
    let session_id =
        payload_string(payload, "session_id").or_else(|| payload_string(payload, "sessionId"));
    let transcript_path = payload_string(payload, "transcript_path")
        .or_else(|| payload_string(payload, "transcriptPath"));
    let feedback = missing_search_pipe_after_prime(
        std::path::Path::new(&registry.project_root),
        session_id.as_deref(),
        transcript_path.as_deref(),
    )
    .ok()
    .flatten()?;
    let mut fields = std::collections::BTreeMap::new();
    fields.insert(
        "hookFeedback".to_string(),
        Value::String("search-pipe-required".to_string()),
    );
    fields.insert(
        "languageId".to_string(),
        Value::String(feedback.language_id.clone()),
    );
    Some(HookDecision {
        schema_id: HOOK_DECISION_SCHEMA_ID,
        schema_version: HOOK_DECISION_SCHEMA_VERSION,
        protocol_id: HOOK_PROTOCOL_ID,
        protocol_version: HOOK_PROTOCOL_VERSION,
        platform: platform.to_string(),
        event: event.to_string(),
        decision: DecisionKind::Block,
        reason_kind: ReasonKind::None,
        language_ids: vec![feedback.language_id.clone()],
        subject: DecisionSubject::default(),
        routes: Vec::new(),
        message: search_pipe_required_stop_message(&feedback.language_id),
        fields,
    })
}

fn search_pipe_required_stop_message(language_id: &str) -> String {
    [
        "ASP hook blocked Stop because this prompt ran `search prime` but has not shown final evidence beyond the prime map."
            .to_string(),
        "The prime packet is only a project/owner map; answer from a justified route frontier, not from prime alone."
            .to_string(),
        String::new(),
        "## Run Next".to_string(),
        "Choose the narrowest ASP route justified by the current evidence state.".to_string(),
        String::new(),
        "## Rules".to_string(),
        "Follow `recommendedNext` or `nextCommand` when the prime packet supplied one."
            .to_string(),
        format!(
            "Run `asp {language_id} search pipe '<question-or-feature-term>' --workspace . --view seeds` only when the evidence is still ambiguous and needs query refinement."
        ),
        "If an owner, symbol, dependency, test/failure, or exact selector is already known, skip pipe and use the narrower owner/reasoning/query route."
            .to_string(),
        "Do not repeat `search prime`. Do not answer from prime alone.".to_string(),
    ]
    .join("\n")
}

fn classify_subagent_stop(platform: &str, event: &str, payload: &Value) -> Option<HookDecision> {
    if event != "subagent-stop" {
        return None;
    }
    let last_message = payload_string(payload, "last_assistant_message")
        .or_else(|| payload_string(payload, "lastAssistantMessage"))
        .unwrap_or_default();
    if last_message.contains("[asp-search-subagent]") {
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
        message: "SubagentStop requires compact [asp-search-subagent] evidence before fan-in."
            .to_string(),
        fields: std::collections::BTreeMap::new(),
    })
}

fn classify_command_action(
    registry: &HookRuntime,
    platform: &str,
    event: &str,
    action: &ToolAction,
    semantic_ast_patch_enabled: bool,
    recovery_prompt: &crate::hook_recovery_prompt::CompiledRecoveryPromptConfig,
) -> Option<HookDecision> {
    let command = action.command.as_deref()?;
    let tokens = action.command_tokens()?;
    if action_is_known_asp_command(registry, action, &tokens) {
        return None;
    }
    let apply_patch_decision = semantic_ast_patch_enabled
        .then(|| classify_apply_patch_command(registry, platform, event, action, command))
        .flatten();
    apply_patch_decision
        .or_else(|| classify_search_json_command(registry, platform, event, action, &tokens))
        .or_else(|| {
            classify_source_read_command(
                registry,
                platform,
                event,
                action,
                command,
                &tokens,
                semantic_ast_patch_enabled,
                recovery_prompt,
            )
        })
        .or_else(|| {
            classify_raw_search_command(
                registry,
                platform,
                event,
                action,
                &tokens,
                semantic_ast_patch_enabled,
                recovery_prompt,
            )
        })
}

fn action_is_known_asp_command(
    registry: &HookRuntime,
    action: &ToolAction,
    tokens: &[String],
) -> bool {
    if !action_supports_asp_command_feedback(action) {
        return false;
    }
    let Some(asp_index) = tokens.iter().enumerate().find_map(|(index, token)| {
        is_asp_binary_token(token)
            .then_some(index)
            .filter(|index| is_asp_invocation_position(tokens, *index))
    }) else {
        return false;
    };
    let Some(facade) = tokens.get(asp_index + 1) else {
        return true;
    };
    is_root_asp_command(facade)
        || registry
            .providers
            .iter()
            .any(|provider| provider.language_id == *facade)
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
        "source apply_patch denied; handwritten source hunks are not a supported workflow for protected source. Locator route: {route_guide}. Treat path-only locator output as a frontier/read-plan, not patch preimage; exact patch context must come from normal selector/code stdout: `asp {language} query --selector <path:start:end> --workspace {project_root} --code`. Build semantic-ast-patch.json with `asp ast-patch template --language {language} --owner <owner-path> --read <path:start:end> --op <operation> --field <key=value> {project_root}`; verify with `asp {language} ast-patch dry-run --packet semantic-ast-patch.json {project_root}`; apply with provider-native `asp {language} ast-patch apply --packet semantic-ast-patch.json {project_root}` when the receipt reports mutationSource=provider-native. Codex text patching is only a codex-text-fallback or controlled maintenance policy path, not the normal AST patch route."
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
        kind: DecisionRouteKind::Lexical,
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
