//! Hook classifier orchestration for `agent-semantic-hook`.

use serde_json::Value;

use crate::command::{
    apply_patch_source_paths, infer_query_from_path, search_json_route, semantic_shell_tokens,
};

use super::decision::{allow, deny_for_action};
use super::recovery::command_line;
use super::source_access_routes::{
    classify_direct_read_action, classify_raw_search_command, classify_source_read_command,
    direct_read_language_ids, direct_read_routes,
};
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

    if config.semantic_ast_patch_enabled()
        && let Some(decision) = actions.iter().find_map(|action| {
            classify_structured_apply_patch_action(registry, platform, event, action)
        })
    {
        return decision;
    }

    if let Some(decision) = actions.iter().find_map(|action| {
        classify_direct_read_action(
            registry,
            platform,
            event,
            action,
            config.semantic_ast_patch_enabled(),
        )
    }) {
        return decision;
    }
    if let Some(decision) = actions.iter().find_map(|action| {
        classify_command_action(
            registry,
            platform,
            event,
            action,
            config.semantic_ast_patch_enabled(),
        )
    }) {
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

fn classify_command_action(
    registry: &HookRuntime,
    platform: &str,
    event: &str,
    action: &ToolAction,
    semantic_ast_patch_enabled: bool,
) -> Option<HookDecision> {
    let command = action.command.as_deref()?;
    let tokens = semantic_shell_tokens(command);
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
            )
        })
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
        "source apply_patch denied; handwritten source hunks are not a supported workflow for protected source. Locator route: {route_guide}. Treat path-only locator output as a frontier/read-plan, not patch preimage; only stdout from `asp {language} query --from-hook direct-source-read --selector <path:start:end> --workspace {project_root} --code` is byte-preserving exact source for patch context. Build semantic-ast-patch.json with `asp ast-patch template --language {language} --owner <owner-path> --read <path:start:end> --op <operation> --field <key=value> {project_root}`; verify with `asp {language} ast-patch dry-run --packet semantic-ast-patch.json {project_root}`; apply with provider-native `asp {language} ast-patch apply --packet semantic-ast-patch.json {project_root}` when the receipt reports mutationSource=provider-native. Codex text patching is only a codex-text-fallback or controlled maintenance policy path, not the normal AST patch route."
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
