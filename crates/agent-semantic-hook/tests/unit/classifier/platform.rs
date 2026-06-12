use agent_semantic_hook::{
    ActivatedProvider, HookClassificationRequest, HookRuntime, classify_hook,
    classify_hook_with_config, load_client_config, render_platform_response,
};
use serde_json::json;
use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

use super::{command, provider, provider_routes, registry};

#[test]
fn platform_response_wraps_denied_decision_for_codex_hooks() {
    let decision = classify_hook(
        &registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "Read",
            "tool_input": {"path": "src/cli/agent-hooks.ts"}
        }),
    );

    let response = render_platform_response(&decision).unwrap();

    assert_eq!(
        response["hookSpecificOutput"]["hookEventName"],
        "PreToolUse"
    );
    assert_eq!(response["hookSpecificOutput"]["permissionDecision"], "deny");
    assert!(response.get("agentHookDecision").is_none());
    let context = response["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("decision context");
    assert!(context.starts_with("[agent-hook-decision] "));
    assert!(context.contains("\"decision\":\"deny\""));
    let reason = response["hookSpecificOutput"]["permissionDecisionReason"]
        .as_str()
        .expect("permission decision reason");
    assert!(
        reason.starts_with("ASP hook blocked `direct-source-read`"),
        "{reason}"
    );
    assert!(reason.contains("call `send_input`"), "{reason}");
    assert!(reason.contains("call `spawn_agent`"), "{reason}");
    assert!(reason.contains("`fork_context=true`"), "{reason}");
    assert!(reason.contains("`asp-search-subagent(role,action,evidence,missing,next,risk)`"));
    assert!(reason.contains("keep model and reasoning settings in Codex config"));
    assert!(
        reason.contains("If subagents are unavailable, run the safe route directly."),
        "{reason}"
    );
    let system_message = response["systemMessage"].as_str().expect("system message");
    assert_eq!(system_message, reason);
    assert!(
        system_message.contains("do not retry raw read/search commands on the same source"),
        "{system_message}"
    );
    assert!(
        system_message.contains(
            "asp typescript query --selector src/cli/agent-hooks.ts --workspace . --code"
        ),
        "{system_message}"
    );
    assert!(!system_message.contains("|run-next"), "{system_message}");
    assert!(
        !system_message.contains("protocol=asp-hook-recovery.v1"),
        "{system_message}"
    );
}

#[test]
fn subagent_platform_response_does_not_prompt_nested_spawn() {
    let mut decision = classify_hook(
        &registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "Read",
            "tool_input": {"path": "src/cli/agent-hooks.ts"}
        }),
    );
    decision
        .fields
        .insert("subagentContext".to_string(), json!(true));

    let response = render_platform_response(&decision).unwrap();

    let reason = response["hookSpecificOutput"]["permissionDecisionReason"]
        .as_str()
        .expect("permission decision reason");
    assert!(!reason.contains("send_input"), "{reason}");
    assert!(!reason.contains("spawn_agent"), "{reason}");
    assert!(!reason.contains("agent_type"), "{reason}");
    assert!(
        reason.contains("already running inside a subagent"),
        "{reason}"
    );
    assert!(reason.contains("`[asp-search-subagent]`"), "{reason}");
    assert!(
        reason.contains(
            "asp typescript query --selector src/cli/agent-hooks.ts --workspace . --code"
        ),
        "{reason}"
    );
    let context = response["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("decision context");
    assert!(!context.contains("send_input"), "{context}");
    assert!(!context.contains("spawn_agent"), "{context}");
    assert!(!context.contains("agent_type"), "{context}");
    assert!(context.contains("\"subagentContext\":true"), "{context}");
}

#[test]
fn permission_request_allow_renders_explicit_allow_for_claude() {
    let decision = classify_hook(
        &registry(),
        "claude",
        "permission-request",
        &json!({
            "hook_event_name": "PermissionRequest",
            "tool_name": "Bash",
            "tool_input": {
                "command": "asp typescript search prime --workspace . --view seeds"
            }
        }),
    );

    let response = render_platform_response(&decision).unwrap();

    assert_eq!(decision.decision, agent_semantic_hook::DecisionKind::Allow);
    assert_eq!(
        response["hookSpecificOutput"]["hookEventName"],
        "PermissionRequest"
    );
    assert_eq!(
        response["hookSpecificOutput"]["permissionDecision"],
        "allow"
    );
    let context = response["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("decision context");
    assert!(context.contains("\"decision\":\"allow\""), "{context}");
}

#[test]
fn user_prompt_submit_allow_adds_search_first_context_for_claude() {
    let decision = classify_hook(
        &registry(),
        "claude",
        "user-prompt",
        &json!({
            "hook_event_name": "UserPromptSubmit",
            "prompt": "How is AsyncRead implemented?"
        }),
    );

    let response = render_platform_response(&decision).unwrap();

    assert_eq!(
        response["hookSpecificOutput"]["hookEventName"],
        "UserPromptSubmit"
    );
    assert!(response["hookSpecificOutput"]["permissionDecision"].is_null());
    let context = response["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("user prompt additional context");
    assert!(context.contains("ASP search-first workflow"), "{context}");
    assert!(
        context.contains("asp <language> search prime --workspace <workspace-root> --view seeds"),
        "{context}"
    );
    assert!(context.contains("at most once"), "{context}");
    assert!(context.contains("immediate next ASP command"), "{context}");
    assert!(
        context.contains("asp <language> search pipe '<question-or-feature-term>' --workspace <workspace-root> --view seeds"),
        "{context}"
    );
    assert!(
        context.contains("Do not answer from prime alone"),
        "{context}"
    );
    assert!(context.contains("prime is only a project map"), "{context}");
    assert!(
        context.contains("ASP facades are language IDs"),
        "{context}"
    );
    assert!(
        context.contains("Do not repeat an exact ASP command"),
        "{context}"
    );
    assert!(
        context.contains("query --selector <path:start-end> --workspace . --code"),
        "{context}"
    );
    assert!(
        context.contains("Do not use direct source reads as the first step"),
        "{context}"
    );
}

#[test]
fn user_prompt_submit_locator_questions_do_not_push_code_reads() {
    let decision = classify_hook(
        &registry(),
        "claude",
        "user-prompt",
        &json!({
            "hook_event_name": "UserPromptSubmit",
            "prompt": "Where is AsyncRead implemented before selecting files to edit?"
        }),
    );

    let response = render_platform_response(&decision).unwrap();
    let context = response["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("user prompt additional context");

    assert!(context.contains("locator/frontier question"), "{context}");
    assert!(
        context.contains("answer where to look before editing"),
        "{context}"
    );
    assert!(
        context.contains("Do not answer from prime alone"),
        "{context}"
    );
    assert!(
        context.contains("ASP facades are language IDs"),
        "{context}"
    );
    assert!(context.contains("Do not run `query --code`"), "{context}");
}

#[test]
fn platform_response_keeps_multi_language_agent_flows_separate() {
    let runtime = HookRuntime {
        project_root: ".".to_string(),
        providers: vec![
            provider_for_language(
                "rust",
                "rs-harness",
                "rs-harness",
                &[".rs"],
                &["Cargo.toml"],
                &["target"],
            ),
            provider_for_language(
                "typescript",
                "ts-harness",
                "ts-harness",
                &[".ts", ".tsx", ".js", ".jsx"],
                &["package.json", "tsconfig.json"],
                &["node_modules", "dist"],
            ),
            provider_for_language(
                "python",
                "py-harness",
                "py-harness",
                &[".py"],
                &["pyproject.toml"],
                &[".venv", "__pycache__"],
            ),
        ],
    };
    let decision = classify_hook(
        &runtime,
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "Bash",
            "tool_input": {
                "command": "cat crates/agent-semantic-hook/src/classifier.rs languages/typescript-lang-project-harness/src/config.ts languages/python-lang-project-harness/src/python_lang_project_harness/_project_config.py"
            }
        }),
    );

    let response = render_platform_response(&decision).unwrap();
    let reason = response["hookSpecificOutput"]["permissionDecisionReason"]
        .as_str()
        .expect("permission decision reason");
    assert!(
        reason.contains(
            "asp rust query --selector crates/agent-semantic-hook/src/classifier.rs --workspace . --code"
        ),
        "{reason}"
    );
    assert!(
        reason.contains(
            "asp typescript query --selector languages/typescript-lang-project-harness/src/config.ts --workspace . --code"
        ),
        "{reason}"
    );
    assert!(
        reason.contains(
            "asp python query --selector languages/python-lang-project-harness/src/python_lang_project_harness/_project_config.py --workspace . --code"
        ),
        "{reason}"
    );
    assert!(!reason.contains("## Detected Binaries"), "{reason}");
    assert!(!reason.contains("### TypeScript"), "{reason}");
}

#[test]
fn platform_response_reflects_disabled_semantic_ast_patch_config() {
    let config_dir = std::env::temp_dir().join(format!(
        "asp-hook-test-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos()
    ));
    fs::create_dir_all(&config_dir).expect("config dir");
    let config_path = config_dir.join("config.toml");
    fs::write(
        &config_path,
        r#"
schemaId = "agent.semantic-protocols.hook.client-config"
schemaVersion = "1"
protocolId = "agent.semantic-protocols.hook"
protocolVersion = "1"

[experimental.semanticAstPatch]
enabled = false
"#,
    )
    .expect("write hook config");
    let config = load_client_config(&config_path).expect("load hook config");
    let runtime = registry();
    let decision = classify_hook_with_config(HookClassificationRequest {
        registry: &runtime,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &json!({
            "tool_name": "Read",
            "tool_input": {"path": "src/cli/agent-hooks.ts"}
        }),
    });
    let response = render_platform_response(&decision).unwrap();
    let reason = response["hookSpecificOutput"]["permissionDecisionReason"]
        .as_str()
        .expect("permission decision reason");

    assert!(reason.contains("call `send_input`"), "{reason}");
    assert!(reason.contains("call `spawn_agent`"), "{reason}");
    assert!(reason.contains("`fork_context=true`"), "{reason}");
    assert!(reason.contains("keep model and reasoning settings in Codex config"));
    assert!(
        !reason.contains(
            "`ast-patch` is available for structural/mechanical edits after a provider dry-run receipt"
        ),
        "{reason}"
    );

    let _ = fs::remove_dir_all(config_dir);
}

fn provider_for_language(
    language_id: &str,
    provider_id: &str,
    binary: &str,
    source_extensions: &[&str],
    config_files: &[&str],
    ignored_path_prefixes: &[&str],
) -> ActivatedProvider {
    let namespace = format!("agent.semantic-protocols.languages.{language_id}.{provider_id}");
    provider(
        language_id,
        provider_id,
        binary,
        &namespace,
        source_extensions,
        config_files,
        &["src", "tests"],
        ignored_path_prefixes,
        provider_routes(
            binary,
            Some(command(&[
                "asp",
                language_id,
                "query",
                "--from-hook",
                "direct-source-read",
                "--selector",
                "{selector}",
                "{termArgs}",
                "--surface",
                "owners,tests",
                "--view",
                "seeds",
                ".",
            ])),
        ),
    )
}
