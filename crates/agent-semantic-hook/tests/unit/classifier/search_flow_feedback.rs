use agent_semantic_hook::{
    ActivatedProvider, DecisionKind, DecisionSubject, HOOK_DECISION_SCHEMA_ID,
    HOOK_DECISION_SCHEMA_VERSION, HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, HookDecision,
    HookRuntime, ReasonKind, append_hook_event_state, classify_hook, render_platform_response,
};
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

use super::{command, provider, provider_routes, registry};

mod after_pipe;
mod document;

#[test]
fn user_prompt_decision_records_prompt_scope_fields() {
    let runtime = runtime_for_project(&temp_project_root("asp-hook-user-prompt-scope"));

    let decision = classify_hook(
        &runtime,
        "claude",
        "user-prompt",
        &json!({
            "hook_event_name": "UserPromptSubmit",
            "session_id": "session-effect",
            "transcript_path": "transcript-effect.jsonl",
            "prompt": "How is search pipe scoped?"
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Allow);
    assert_eq!(decision.fields["sessionId"], "session-effect");
    assert_eq!(decision.fields["transcriptPath"], "transcript-effect.jsonl");
    let _ = fs::remove_dir_all(runtime.project_root);
}

#[test]
fn stop_hook_blocks_prime_only_flow_until_evidence_frontier_runs() {
    let project_root = temp_project_root("asp-hook-stop-prime-only");
    append_hook_event_state(
        &project_root,
        &allowed_command_decision(
            "claude",
            "post-tool",
            "session-effect",
            "transcript-effect.jsonl",
            "asp typescript search prime --workspace . --view seeds",
        ),
    )
    .expect("write hook event");
    let runtime = runtime_for_project(&project_root);

    let decision = classify_hook(
        &runtime,
        "claude",
        "stop",
        &json!({
            "hook_event_name": "Stop",
            "session_id": "session-effect",
            "transcript_path": "transcript-effect.jsonl"
        }),
    );
    let response = render_platform_response(&decision).unwrap();

    assert_eq!(decision.decision, DecisionKind::Block);
    assert_eq!(decision.language_ids, vec!["typescript"]);
    assert_eq!(decision.fields["hookFeedback"], "search-pipe-required");
    assert!(
        decision
            .message
            .contains("Choose the narrowest ASP route justified by the current evidence state"),
        "{}",
        decision.message
    );
    assert!(
        decision.message.contains("skip pipe and use the narrower"),
        "{}",
        decision.message
    );
    assert_eq!(response["decision"], "block");
    let context = response["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("additional context");
    assert!(
        context.contains("The prime packet is only a project/owner map"),
        "{context}"
    );
    assert!(
        context.contains("Do not repeat `search prime`"),
        "{context}"
    );

    let _ = fs::remove_dir_all(project_root);
}

#[test]
fn stop_hook_allows_after_search_pipe_runs() {
    let project_root = temp_project_root("asp-hook-stop-after-pipe");
    append_hook_event_state(
        &project_root,
        &allowed_command_decision(
            "claude",
            "post-tool",
            "session-effect",
            "transcript-effect.jsonl",
            "asp typescript search prime --workspace . --view seeds",
        ),
    )
    .expect("write prime event");
    append_hook_event_state(
        &project_root,
        &allowed_command_decision(
            "claude",
            "post-tool",
            "session-effect",
            "transcript-effect.jsonl",
            "asp typescript search pipe 'Effect concurrency Fiber Queue Stream Scope' --workspace . --view seeds",
        ),
    )
    .expect("write pipe event");
    let runtime = runtime_for_project(&project_root);

    let decision = classify_hook(
        &runtime,
        "claude",
        "stop",
        &json!({
            "hook_event_name": "Stop",
            "session_id": "session-effect",
            "transcript_path": "transcript-effect.jsonl"
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Allow);
    let _ = fs::remove_dir_all(project_root);
}

#[test]
fn pre_tool_denies_explicit_read_before_pipe() {
    let project_root = temp_project_root("asp-hook-read-before-pipe");
    append_hook_event_state(
        &project_root,
        &allowed_command_decision(
            "claude",
            "pre-tool",
            "session-effect",
            "transcript-effect.jsonl",
            "asp typescript search prime --workspace . --view seeds",
        ),
    )
    .expect("write prime event");
    let runtime = runtime_for_project(&project_root);

    let decision = classify_hook(
        &runtime,
        "claude",
        "pre-tool",
        &json!({
            "hook_event_name": "PreToolUse",
            "session_id": "session-effect",
            "transcript_path": "transcript-effect.jsonl",
            "tool_name": "Bash",
            "tool_input": {
                "command": "asp typescript query --from-hook direct-source-read --selector packages/effect/src/internal/concurrency.ts --workspace . --code"
            }
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.fields["hookFeedback"], "read-before-pipe");
    assert_eq!(decision.fields["aspCommandIntent"], "direct-read-fallback");
    assert!(decision.routes.is_empty());
    let _ = fs::remove_dir_all(project_root);
}

#[test]
fn pre_tool_allows_valid_root_language_search_form() {
    let project_root = temp_project_root("asp-hook-valid-root-search");
    let runtime = runtime_for_project(&project_root);

    let decision = classify_hook(
        &runtime,
        "claude",
        "pre-tool",
        &json!({
            "hook_event_name": "PreToolUse",
            "session_id": "session-effect",
            "transcript_path": "transcript-effect.jsonl",
            "tool_name": "Bash",
            "tool_input": {
                "command": "asp search --language typescript prime --workspace . --view seeds"
            }
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Allow);
    assert_eq!(decision.fields["aspCommandIntent"], "reasoning");
    assert_eq!(decision.fields["aspCommandRoute"], "search-prime");
    let _ = fs::remove_dir_all(project_root);
}

#[test]
fn pre_tool_classifies_language_reasoning_routes_precisely() {
    let project_root = temp_project_root("asp-hook-reasoning-route-taxonomy");
    let runtime = runtime_for_project(&project_root);

    for (command, route) in [
        ("asp typescript guide --workspace .", "guide"),
        (
            "asp typescript search prime --workspace . --view seeds",
            "search-prime",
        ),
        (
            "asp typescript search pipe 'effect fiber' --workspace . --view seeds",
            "search-pipe",
        ),
        (
            "asp typescript search owner src/Fiber.ts items --workspace . --view seeds",
            "search-owner",
        ),
        (
            "asp typescript search lexical --query Fiber --query Scope --workspace . --view seeds",
            "search-lexical",
        ),
        (
            "asp typescript search deps Effect --workspace . --view seeds",
            "search-deps",
        ),
        (
            "asp typescript search failure failing-test --workspace . --view seeds",
            "search-failure",
        ),
        (
            "asp typescript search reasoning Fiber --workspace . --view seeds",
            "search-reasoning",
        ),
        (
            "asp typescript query guide treesitter --workspace .",
            "query-reasoning",
        ),
        (
            "asp typescript query --term useEffect --workspace . --code",
            "query-reasoning",
        ),
    ] {
        let decision = classify_hook(
            &runtime,
            "claude",
            "pre-tool",
            &json!({
                "hook_event_name": "PreToolUse",
                "tool_name": "Bash",
                "tool_input": {"command": command}
            }),
        );

        assert_eq!(decision.decision, DecisionKind::Allow, "{command}");
        assert_eq!(
            decision.fields["aspCommandIntent"], "reasoning",
            "{command}"
        );
        assert_eq!(decision.fields["aspCommandRoute"], route, "{command}");
    }
    let _ = fs::remove_dir_all(project_root);
}

#[test]
fn pre_tool_allows_exact_selector_names_only_projection() {
    let project_root = temp_project_root("asp-hook-exact-names-only");
    let runtime = runtime_for_project(&project_root);
    let decision = classify_hook(
        &runtime,
        "claude",
        "pre-tool",
        &json!({
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_input": {
                "command": "asp typescript query --selector typescript://src/Fiber.ts#item/function/runFiber --workspace . --names-only"
            }
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Allow);
    assert_eq!(decision.fields["aspCommandIntent"], "exact-evidence");
    assert_eq!(decision.fields["aspCommandRoute"], "query-selector");
    let _ = fs::remove_dir_all(project_root);
}

#[test]
fn pre_tool_rejects_non_structural_or_cross_language_evidence_selectors() {
    let project_root = temp_project_root("asp-hook-invalid-evidence-selectors");
    let runtime = runtime_for_project(&project_root);

    for selector in [
        "src/Fiber.ts",
        "src/Fiber.ts:10:20",
        "typescript://#item/function/runFiber",
        "typescript://src/Fiber.ts#item/function",
        "rust://src/Fiber.ts#item/function/runFiber",
        "TypeScript://src/Fiber.ts#item/function/runFiber",
    ] {
        let command = format!("asp typescript query --selector {selector} --workspace . --code");
        let decision = classify_hook(
            &runtime,
            "claude",
            "pre-tool",
            &json!({
                "hook_event_name": "PreToolUse",
                "tool_name": "Bash",
                "tool_input": {"command": command}
            }),
        );

        assert_eq!(decision.decision, DecisionKind::Deny, "{selector}");
        assert_eq!(decision.fields["aspCommandIntent"], "invalid-evidence");
        assert_eq!(
            decision.fields["hookFeedback"],
            "invalid-evidence-query-denied"
        );
    }
    let _ = fs::remove_dir_all(project_root);
}

#[test]
fn pre_tool_allows_asp_binary_probe_without_treating_asp_as_facade() {
    let project_root = temp_project_root("asp-hook-which-asp");
    let runtime = runtime_for_project(&project_root);

    let decision = classify_hook(
        &runtime,
        "claude",
        "pre-tool",
        &json!({
            "hook_event_name": "PreToolUse",
            "session_id": "session-effect",
            "transcript_path": "transcript-effect.jsonl",
            "tool_name": "Bash",
            "tool_input": {
                "command": "which asp 2>/dev/null || echo \"asp not found\""
            }
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Allow);
    assert!(!decision.fields.contains_key("hookFeedback"));
    let _ = fs::remove_dir_all(project_root);
}

fn runtime_for_project(project_root: &std::path::Path) -> HookRuntime {
    HookRuntime {
        project_root: project_root.display().to_string(),
        providers: registry().providers,
    }
}

fn document_runtime_for_project(project_root: &std::path::Path) -> HookRuntime {
    HookRuntime {
        project_root: project_root.display().to_string(),
        providers: vec![markdown_provider()],
    }
}

fn markdown_provider() -> ActivatedProvider {
    provider(
        "md",
        "orgize",
        "asp",
        "agent.semantic-protocols.languages.md.orgize",
        &[".md", ".markdown"],
        &[],
        &["."],
        &[".git"],
        provider_routes(
            "asp",
            Some(command(&[
                "asp",
                "md",
                "query",
                "--selector",
                "{selector}",
                "--workspace",
                ".",
                "--content",
            ])),
        ),
    )
}

fn temp_project_root(prefix: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "{prefix}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos()
    ));
    fs::create_dir_all(&path).expect("temp project root");
    fs::create_dir_all(path.join(".git")).expect("temp git marker");
    path
}

fn allowed_command_decision(
    platform: &str,
    event: &str,
    session_id: &str,
    transcript_path: &str,
    command: &str,
) -> HookDecision {
    let mut fields = BTreeMap::new();
    fields.insert(
        "sessionId".to_string(),
        serde_json::Value::String(session_id.to_string()),
    );
    fields.insert(
        "transcriptPath".to_string(),
        serde_json::Value::String(transcript_path.to_string()),
    );
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
        subject: DecisionSubject {
            tool_name: Some("Bash".to_string()),
            command: Some(command.to_string()),
            paths: Vec::new(),
        },
        routes: Vec::new(),
        message: String::new(),
        fields,
    }
}

fn allowed_prompt_decision(
    platform: &str,
    session_id: &str,
    transcript_path: &str,
) -> HookDecision {
    let mut fields = BTreeMap::new();
    fields.insert(
        "sessionId".to_string(),
        serde_json::Value::String(session_id.to_string()),
    );
    fields.insert(
        "transcriptPath".to_string(),
        serde_json::Value::String(transcript_path.to_string()),
    );
    HookDecision {
        schema_id: HOOK_DECISION_SCHEMA_ID,
        schema_version: HOOK_DECISION_SCHEMA_VERSION,
        protocol_id: HOOK_PROTOCOL_ID,
        protocol_version: HOOK_PROTOCOL_VERSION,
        platform: platform.to_string(),
        event: "user-prompt".to_string(),
        decision: DecisionKind::Allow,
        reason_kind: ReasonKind::None,
        language_ids: Vec::new(),
        subject: DecisionSubject::default(),
        routes: Vec::new(),
        message: String::new(),
        fields,
    }
}
