//! Hook classifier orchestration for `agent-semantic-hook`.

use serde_json::Value;

use crate::command::{apply_patch_source_paths, infer_query_from_path, search_json_route};

use super::agent_org_artifacts::with_agent_org_artifact_recovery;
use super::decision::{allow, deny_for_action};
use super::recovery::command_line;
use super::source_access_routes::{
    classify_direct_read_action, direct_read_language_ids, direct_read_routes,
};
use crate::event_state::asp_command_tokens;
use crate::{
    ActivatedProvider, ClientHookConfig, DecisionRoute, DecisionRouteKind, DecisionSubject,
    HookDecision, HookRuntime, OperationIntent, ReasonKind, ToolAction,
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
    let actions = collect_payload_tool_actions(request.payload);
    let decision = if let Some(decision) = classify_non_tool_event(&request) {
        decision
    } else if let Some(decision) = classify_tool_actions(&request, &actions) {
        decision
    } else {
        let subject = actions.first().map(subject_for_action).unwrap_or_default();
        allow(request.platform, request.event, subject)
    };
    let decision = with_selector_only_subagent_message(decision);
    let decision = with_prompt_scope_fields(decision, request.payload);
    let decision =
        with_agent_org_artifact_recovery(decision, request.config, &request.registry.project_root);
    with_hook_match_receipt(decision, request.payload, &actions, request.registry)
}

fn with_hook_match_receipt(
    mut decision: HookDecision,
    payload: &Value,
    actions: &[ToolAction],
    registry: &HookRuntime,
) -> HookDecision {
    let mut payload_keys = payload
        .as_object()
        .map(|object| object.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    payload_keys.sort();
    decision.fields.insert(
        "hookPayloadKeys".to_string(),
        Value::Array(payload_keys.into_iter().map(Value::String).collect()),
    );

    let normalized_actions = actions
        .iter()
        .filter(|action| {
            !action.tool_name.is_empty()
                || action.command.is_some()
                || !action.paths.is_empty()
                || action.operation != OperationIntent::Unknown
        })
        .map(|action| {
            serde_json::json!({
                "toolName": action.tool_name,
                "toolSurface": action.surface.as_str(),
                "operationIntent": action.operation.as_str(),
                "paths": action.paths,
            })
        })
        .collect();
    decision.fields.insert(
        "normalizedActions".to_string(),
        Value::Array(normalized_actions),
    );
    if let Some(snapshot) =
        crate::hook_policy_kernel::active_policy_snapshot(&registry.project_root)
    {
        decision.fields.insert(
            "hookPolicySnapshotDigest".to_string(),
            Value::String(snapshot.generation_digest.clone()),
        );
        decision.fields.insert(
            "hookPolicyKernelVersion".to_string(),
            Value::String(snapshot.kernel_version.to_string()),
        );
        decision.fields.insert(
            "hookPolicySynchronousDependencies".to_string(),
            Value::Array(Vec::new()),
        );
    }
    decision
}

fn with_selector_only_subagent_message(mut decision: HookDecision) -> HookDecision {
    if decision
        .message
        .contains("Return one compact `[asp-search-subagent]` graph-route receipt")
        && !decision
            .message
            .contains("Return selector-only `[asp-search-subagent]` evidence")
    {
        decision.message = decision.message.replace(
            "Return one compact `[asp-search-subagent]` graph-route receipt",
            "Return selector-only `[asp-search-subagent]` evidence. Return one compact `[asp-search-subagent]` graph-route receipt",
        );
    }
    decision
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
    classify_user_prompt(request.platform, request.event, request.payload)
}

fn direct_executable_token_v1(command: &str) -> Option<&str> {
    let mut tokens = command.split_whitespace();
    let mut executable = tokens.next()?;
    while shell_environment_assignment_v1(executable) {
        executable = tokens.next()?;
    }
    if executable.is_empty()
        || executable.bytes().any(|byte| {
            matches!(
                byte,
                b'\'' | b'"' | b'`' | b'$' | b'\\' | b';' | b'|' | b'&' | b'<' | b'>' | b'(' | b')'
            )
        })
    {
        return None;
    }
    Some(executable)
}

fn shell_environment_assignment_v1(token: &str) -> bool {
    let Some((name, _value)) = token.split_once('=') else {
        return false;
    };
    let mut bytes = name.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    (first == b'_' || first.is_ascii_alphabetic())
        && bytes.all(|byte| byte == b'_' || byte.is_ascii_alphanumeric())
}

fn root_session_is_known_v1(payload: &Value) -> bool {
    payload_string(payload, "session_id")
        .or_else(|| payload_string(payload, "sessionId"))
        .is_some_and(|session_id| !session_id.is_empty())
}

fn classify_runtime_binary_action_v1(
    request: &HookClassificationRequest<'_>,
    action: &ToolAction,
) -> Option<HookDecision> {
    if !root_session_is_known_v1(request.payload) {
        return None;
    }
    let executable = direct_executable_token_v1(action.command.as_deref()?)?;
    let crate::provider_registry::RuntimeBinaryClassificationV1::RegisteredProviderInternal(
        registrations,
    ) = crate::provider_registry::classify_runtime_executable_v1(executable)
    else {
        return None;
    };

    let language_ids = registrations
        .iter()
        .map(|registration| registration.language_id().clone())
        .collect::<Vec<_>>();
    let routes = registrations
        .iter()
        .map(|registration| crate::protocol::DecisionRoute {
            language_id: registration.language_id().clone(),
            provider_id: registration.provider_id().clone(),
            binary: "asp".to_string(),
            kind: crate::protocol::DecisionRouteKind::Query,
            argv: vec![
                "asp".to_string(),
                registration.language_id().as_str().to_string(),
            ],
            stdin_mode: None,
        })
        .collect::<Vec<_>>();
    let route_guide = registrations
        .iter()
        .map(|registration| format!("asp {}", registration.language_id().as_str()))
        .collect::<Vec<_>>()
        .join("; ");
    let registered_binary = registrations
        .first()
        .map(|registration| registration.binary().to_string())
        .unwrap_or_else(|| executable.to_string());
    let denial =
        crate::provider_registry::RuntimeBinaryAdmissionDenialV1::MissingDispatchCapability;
    let mut decision = deny_for_action(
        request.platform,
        request.event,
        super::decision::DenyForActionRequest {
            reason_kind: ReasonKind::ProviderBinaryDirectExecution,
            action,
            language_ids,
            subject: subject_for_action(action),
            routes,
            message: format!(
                "registered provider-internal binary `{executable}` denied: root session is known but no server-verified ASP runtime dispatch capability is present. Use a registry-derived facade route: {route_guide}."
            ),
        },
    );
    decision.fields.insert(
        "runtimeBinaryProfile".to_string(),
        Value::String("provider-internal".to_string()),
    );
    decision.fields.insert(
        "runtimeBinaryAdmissionDenial".to_string(),
        Value::String(match denial {
            crate::provider_registry::RuntimeBinaryAdmissionDenialV1::MissingDispatchCapability => {
                "missing-dispatch-capability"
            }
            crate::provider_registry::RuntimeBinaryAdmissionDenialV1::DirectProviderInternal => {
                "direct-provider-internal"
            }
            crate::provider_registry::RuntimeBinaryAdmissionDenialV1::UnregisteredRuntimeBinary => {
                "unregistered-runtime-binary"
            }
        }
        .to_string()),
    );
    decision.fields.insert(
        "runtimeBinary".to_string(),
        Value::String(registered_binary),
    );
    Some(decision)
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
        payload: _,
    } = request;
    let mut first_allow = None;
    for action in actions {
        if let Some(decision) = classify_runtime_binary_action_v1(request, action) {
            return Some(decision);
        }
        let Some(decision) = config.classify(registry, platform, event, action) else {
            continue;
        };
        match decision.decision {
            crate::DecisionKind::Allow => {
                first_allow.get_or_insert(decision);
            }
            crate::DecisionKind::Block | crate::DecisionKind::Deny => return Some(decision),
        }
    }
    first_allow
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

pub(crate) fn materialize_apply_patch_decision(
    registry: &HookRuntime,
    platform: &str,
    event: &str,
    action: &ToolAction,
    semantic_ast_patch_enabled: bool,
) -> Option<HookDecision> {
    if !semantic_ast_patch_enabled {
        return None;
    }
    if let Some(decision) =
        classify_structured_apply_patch_action(registry, platform, event, action)
    {
        return Some(decision);
    }
    let command = action.command.as_deref()?;
    classify_apply_patch_command(registry, platform, event, action, command)
}

pub(crate) fn materialize_source_access_decision(
    registry: &HookRuntime,
    platform: &str,
    event: &str,
    action: &ToolAction,
    agent_action: Option<&crate::tool_action::AgentAction>,
    semantic_ast_patch_enabled: bool,
    recovery_prompt: &crate::hook_recovery_prompt::CompiledRecoveryPromptConfig,
) -> Option<HookDecision> {
    classify_direct_read_action(
        registry,
        platform,
        event,
        action,
        agent_action,
        semantic_ast_patch_enabled,
        recovery_prompt,
    )
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
        .map(agent_semantic_config::LanguageId::as_str)
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
        "source apply_patch denied; handwritten source hunks are not a supported workflow for protected source. Locator route: {route_guide}. Treat path-only locator output as a frontier, not patch preimage; exact patch context must come from `asp {language} query --selector <exact-structural-selector> --workspace {project_root} --projection source`. Build semantic-ast-patch.json with `asp ast-patch template --language {language} --owner <owner-path> --read <exact-structural-selector> --op <operation> --field <key=value> {project_root}`; verify with `asp {language} ast-patch dry-run --packet semantic-ast-patch.json {project_root}`; apply with provider-native `asp {language} ast-patch apply --packet semantic-ast-patch.json {project_root}` when the receipt reports mutationSource=provider-native. Codex text patching is only a codex-text-fallback or controlled maintenance policy path, not the normal AST patch route."
    );
    Some(deny_for_action(
        platform,
        event,
        super::decision::DenyForActionRequest {
            reason_kind: ReasonKind::SemanticAstPatchRequired,
            action,
            language_ids: languages,
            subject,
            routes,
            message,
        },
    ))
}

pub(crate) fn materialize_agent_search_json_decision(
    registry: &HookRuntime,
    platform: &str,
    event: &str,
    action: &ToolAction,
    tokens: &[String],
) -> Option<HookDecision> {
    if asp_command_tokens(tokens) || crate::command::is_asp_facade_command(tokens) {
        return None;
    }
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
        super::decision::DenyForActionRequest {
            reason_kind: ReasonKind::AgentSearchJson,
            action,
            language_ids: vec![provider.language_id.clone()],
            subject: subject_for_action(action),
            routes,
            message,
        },
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
