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
        reason.starts_with("ASP denied `direct-source-read`"),
        "{reason}"
    );
    assert!(
        reason.contains("configured resident ASP session"),
        "{reason}"
    );
    assert!(
        reason.contains("Return one compact `[asp-search-subagent]` graph-route receipt"),
        "{reason}"
    );
    assert!(
        reason.contains("Do not return source bodies, snippets, or line-range selectors"),
        "{reason}"
    );
    assert!(!reason.contains("call `send_input`"), "{reason}");
    assert!(
        reason.contains("start the configured resident ASP subagent"),
        "{reason}"
    );
    assert!(
        reason.contains("register it from this root session"),
        "{reason}"
    );
    assert!(
        reason.contains("Forward ASP search/query, owner/frontier ranking"),
        "{reason}"
    );
    assert!(
        !reason.contains("fall back to `agent_type=\"explorer\"`"),
        "{reason}"
    );
    assert!(!reason.contains("`fork_context=false`"), "{reason}");
    assert!(!reason.contains("fork_turns"), "{reason}");
    assert!(
        !reason.contains("Record the returned `agent-...` id"),
        "{reason}"
    );
    assert!(!reason.contains("Keep model and reasoning settings in Codex config"));
    assert!(!reason.contains("If subagents are unavailable"), "{reason}");
    let system_message = response["systemMessage"].as_str().expect("system message");
    assert_eq!(system_message, reason);
    assert!(
        system_message.contains("Do not retry raw source tools."),
        "{system_message}"
    );
    assert!(
        system_message.contains(
            "asp typescript search owner src/cli/agent-hooks.ts items --workspace . --view seeds"
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
        reason.contains("configured resident ASP session"),
        "{reason}"
    );
    assert!(
        reason.contains("Return one compact `[asp-search-subagent]` graph-route receipt"),
        "{reason}"
    );
    assert!(
        reason.contains("Do not return source bodies, snippets, or line-range selectors"),
        "{reason}"
    );
    assert!(
        reason.contains(
            "asp typescript search owner src/cli/agent-hooks.ts items --workspace . --view seeds"
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
    assert!(
        context.contains("ASP evidence-state search routing"),
        "{context}"
    );
    assert!(
        context.contains("Search is not a mandatory pipeline"),
        "{context}"
    );
    assert!(
        context.contains("Choose the narrowest ASP route"),
        "{context}"
    );
    assert!(context.contains("skip `search prime`"), "{context}");
    assert!(
        context.contains("search prime --workspace <workspace-root> --view seeds"),
        "{context}"
    );
    assert!(
        context.contains(
            "search pipe '<question-or-feature-term>' --workspace <workspace-root> --view seeds"
        ),
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
        context.contains("query --selector <exact-selector> --workspace . --code"),
        "{context}"
    );
    assert!(
        context.contains("return one compact `[asp-search-subagent]` graph-route receipt"),
        "{context}"
    );
    assert!(
        context.contains("never source bodies or line-range selectors"),
        "{context}"
    );
    assert!(
        context.contains("display line ranges and sourceLocatorHint as hints"),
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
    assert!(
        context.contains("compact `[asp-search-subagent]` graph-route receipt"),
        "{context}"
    );
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
            "asp rust search owner crates/agent-semantic-hook/src/classifier.rs items --workspace . --view seeds"
        ),
        "{reason}"
    );
    assert!(
        reason.contains(
            "asp typescript search owner languages/typescript-lang-project-harness/src/config.ts items --workspace . --view seeds"
        ),
        "{reason}"
    );
    assert!(
        reason.contains(
            "asp python search owner languages/python-lang-project-harness/src/python_lang_project_harness/_project_config.py items --workspace . --view seeds"
        ),
        "{reason}"
    );
    assert!(!reason.contains("## Detected Binaries"), "{reason}");
    assert!(!reason.contains("### TypeScript"), "{reason}");
}

#[test]
fn platform_response_reflects_disabled_semantic_ast_patch_config() {
    let config_dir = std::env::temp_dir().join(format!(
        "asp-hook-test-disabled-semantic-ast-patch-{}-{}",
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

    assert!(
        reason.starts_with("ASP denied `direct-source-read`"),
        "{reason}"
    );
    assert!(
        reason.contains("configured resident ASP session"),
        "{reason}"
    );
    assert!(
        reason.contains("Return one compact `[asp-search-subagent]` graph-route receipt"),
        "{reason}"
    );
    assert!(
        reason.contains("Do not return source bodies, snippets, or line-range selectors"),
        "{reason}"
    );
    assert!(!reason.contains("call `send_input`"), "{reason}");
    assert!(
        reason.contains("start the configured resident ASP subagent"),
        "{reason}"
    );
    assert!(
        reason.contains("register it from this root session"),
        "{reason}"
    );
    assert!(
        reason.contains("Forward ASP search/query, owner/frontier ranking"),
        "{reason}"
    );
    assert!(
        !reason.contains("fall back to `agent_type=\"explorer\"`"),
        "{reason}"
    );
    assert!(!reason.contains("`fork_context=false`"), "{reason}");
    assert!(!reason.contains("fork_turns"), "{reason}");
    assert!(
        !reason.contains("Record the returned `agent-...` id"),
        "{reason}"
    );
    assert!(!reason.contains("Keep model and reasoning settings in Codex config"));
    assert!(
        !reason.contains(
            "`ast-patch` is available for structural/mechanical edits after a provider dry-run receipt"
        ),
        "{reason}"
    );

    let _ = fs::remove_dir_all(config_dir);
}

#[test]
fn platform_response_uses_configured_recovery_prompt_template() {
    let config_dir = std::env::temp_dir().join(format!(
        "asp-hook-test-configured-recovery-prompt-{}-{}",
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

[recoveryPrompt]
template = "reason={reason}\nflow={agent_flow}\nroutes={routes}"
codexAgentFlow = "configured asp-explore flow"
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

    assert!(reason.contains("reason=direct-source-read"), "{reason}");
    assert!(
        reason.contains("flow=configured asp-explore flow"),
        "{reason}"
    );
    assert!(reason.contains("routes=```sh"), "{reason}");
    assert!(
        reason.contains(
            "asp typescript search owner src/cli/agent-hooks.ts items --workspace . --view seeds"
        ),
        "{reason}"
    );
    assert!(
        !reason.contains("start the ASP explorer subagent"),
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
                "--workspace",
                ".",
                "--view",
                "seeds",
            ])),
        ),
    )
}
#[test]
fn read_only_subagent_write_denial_uses_sandbox_permission_context() {
    let payload = serde_json::json!({
        "session_id": "child-session",
        "tool_name": "Write",
        "tool_input": {
            "path": "src/lib.rs"
        }
    });
    let context = agent_semantic_hook::HookSubagentPermissionContext {
        is_asp_managed: true,
        managed_child_name: "asp-explore",
        registered_name: "asp-explore",
        registry_status: "active",
        sandbox_mode: Some("read-only"),
        session_id: "child-session",
    };

    let decision = agent_semantic_hook::classify_read_only_subagent_write(
        "codex", "pre-tool", &payload, &context,
    )
    .expect("read-only ASP-managed write should be denied");

    assert_eq!(decision.decision, agent_semantic_hook::DecisionKind::Deny);
    assert_eq!(
        decision.reason_kind,
        agent_semantic_hook::ReasonKind::ReadOnlySubagentWrite
    );
    assert_eq!(
        decision.fields.get("configuredSandboxMode"),
        Some(&serde_json::json!("read-only"))
    );
    assert!(
        decision
            .message
            .contains("selector-only graph-route `[asp-search-subagent]` receipt"),
        "{}",
        decision.message
    );
    assert!(
        decision
            .message
            .contains("schema/intent/route/state/evidence/next"),
        "{}",
        decision.message
    );
    assert!(
        decision
            .message
            .contains("do not return source bodies, snippets, or line-range selectors"),
        "{}",
        decision.message
    );
    assert!(!decision.message.contains("return compact evidence"));
}

#[test]
fn read_only_subagent_write_denial_ignores_unmanaged_subagents() {
    let payload = serde_json::json!({
        "session_id": "child-session",
        "tool_name": "Write",
        "tool_input": {
            "path": "src/lib.rs"
        }
    });
    let context = agent_semantic_hook::HookSubagentPermissionContext {
        is_asp_managed: false,
        managed_child_name: "asp-explore",
        registered_name: "user-subagent",
        registry_status: "active",
        sandbox_mode: Some("read-only"),
        session_id: "child-session",
    };

    assert!(
        agent_semantic_hook::classify_read_only_subagent_write(
            "codex", "pre-tool", &payload, &context,
        )
        .is_none()
    );
}

#[test]
fn read_only_subagent_receipt_accepts_graph_route_receipts() {
    let context = agent_semantic_hook::HookSubagentPermissionContext {
        is_asp_managed: true,
        managed_child_name: "asp-explore",
        registered_name: "asp-explore",
        registry_status: "active",
        sandbox_mode: Some("read-only"),
        session_id: "child-session",
    };

    for message in [
        "[asp-search-subagent]\nschema=asp-search-subagent.graph.v1\nintent=receipt-validation\nroute=hook/read-only-subagent -> tests\nstate=selector-ready\nevidence=E1 kind=item role=primary owner=crates/agent-semantic-hook/src/read_only_subagent.rs selector=rust://crates/agent-semantic-hook/src/read_only_subagent.rs#item/function/classify_read_only_subagent_receipt relation=validates-receipt\nnext=E1 asp rust query --selector rust://crates/agent-semantic-hook/src/read_only_subagent.rs#item/function/classify_read_only_subagent_receipt --workspace . --code\navoid=raw-read,flat-selector-list\nomit=source,line-range,confidence,long-explanation",
        "[asp-search-subagent]\nschema=asp-search-subagent.graph.v1\nintent=receipt-validation\nroute=owner -> item -> test\nstate=selector-ready\nrankedEvidence=E1 kind=item role=primary owner=src/lib.rs selector=rust://src/lib.rs#item/function/run relation=selected; E2 kind=test role=guard owner=tests/run.rs selector=rust://tests/run.rs#item/function/run_is_guarded relation=covers\nedges=E1-covered-by->E2\nnext=E1 asp rust query --selector rust://src/lib.rs#item/function/run --workspace . --code\nalt=E2 asp rust query --selector rust://tests/run.rs#item/function/run_is_guarded --workspace . --code\navoid=raw-read,flat-selector-list\nomit=source,line-range,confidence,long-explanation,not-found-inventory",
    ] {
        let payload = serde_json::json!({
            "session_id": "child-session",
            "last_assistant_message": message
        });

        let decision = agent_semantic_hook::classify_read_only_subagent_receipt(
            "codex",
            "subagent-stop",
            &payload,
            &context,
        )
        .expect("managed read-only ASP subagent receipt should be classified");

        assert_eq!(decision.decision, agent_semantic_hook::DecisionKind::Allow);
        assert_eq!(
            decision.fields.get("subagentReceiptStatus"),
            Some(&serde_json::json!("accepted"))
        );
    }
}

#[test]
fn read_only_subagent_receipt_blocks_broad_or_explanatory_receipts() {
    let context = agent_semantic_hook::HookSubagentPermissionContext {
        is_asp_managed: true,
        managed_child_name: "asp-explore",
        registered_name: "asp-explore",
        registry_status: "active",
        sandbox_mode: Some("read-only"),
        session_id: "child-session",
    };

    for message in [
        "[asp-search-subagent]\nowner=src/lib.rs\nread=src/lib.rs:1-80\nnext=asp rust query --selector src/lib.rs:1-80 --workspace . --code",
        "[asp-search-subagent]\nschema=asp-search-subagent.graph.v1\nintent=bad-line-range\nroute=owner -> item\nstate=selector-ready\nevidence=E1 kind=item role=primary owner=src/lib.rs selector=src/lib.rs:1-80 relation=bad\nnext=E1 asp rust query --selector src/lib.rs:1-80 --workspace . --code\navoid=raw-read\nomit=source,line-range",
        "[asp-search-subagent]\nschema=asp-search-subagent.graph.v1\nintent=item-skeleton\nroute=owner -> item\nstate=selector-ready\nevidence=E1 kind=item role=primary owner=src/lib.rs selector=rust://src/lib.rs#item/function/run relation=bad\nnext=E1 asp rust query --from-hook item-skeleton --selector rust://src/lib.rs#item/function/run --workspace . --names-only\navoid=raw-read\nomit=source,line-range",
        "[asp-search-subagent]\nschema=asp-search-subagent.graph.v1\nintent=prose\nroute=owner -> item\nstate=selector-ready\nevidence=E1 kind=item role=primary owner=src/lib.rs selector=rust://src/lib.rs#item/function/run relation=bad\nnext=E1 asp rust query --selector rust://src/lib.rs#item/function/run --workspace . --code\nconfidence=high",
        "[asp-search-subagent]\nschema=asp-search-subagent.graph.v1\nintent=ranked-evidence-missing-owner\nroute=owner -> item\nstate=selector-ready\nrankedEvidence=E1 kind=item role=primary selector=rust://src/lib.rs#item/function/run relation=bad\nnext=E1 asp rust query --selector rust://src/lib.rs#item/function/run --workspace . --code\navoid=raw-read\nomit=source,line-range",
    ] {
        let payload = serde_json::json!({
            "session_id": "child-session",
            "last_assistant_message": message
        });

        let decision = agent_semantic_hook::classify_read_only_subagent_receipt(
            "codex",
            "subagent-stop",
            &payload,
            &context,
        )
        .expect("managed read-only ASP subagent receipt should be classified");

        assert_eq!(decision.decision, agent_semantic_hook::DecisionKind::Block);
        assert_eq!(
            decision.reason_kind,
            agent_semantic_hook::ReasonKind::SubagentReceiptRequired
        );
        assert!(
            decision
                .message
                .contains("valid selector-only graph-route `[asp-search-subagent]` receipt"),
            "{}",
            decision.message
        );
    }
}

#[test]
fn read_only_subagent_receipt_ignores_unmanaged_subagents() {
    let payload = serde_json::json!({
        "session_id": "child-session",
        "last_assistant_message": "ordinary user subagent final message"
    });
    let context = agent_semantic_hook::HookSubagentPermissionContext {
        is_asp_managed: false,
        managed_child_name: "asp-explore",
        registered_name: "user-subagent",
        registry_status: "active",
        sandbox_mode: Some("read-only"),
        session_id: "child-session",
    };

    assert!(
        agent_semantic_hook::classify_read_only_subagent_receipt(
            "codex",
            "subagent-stop",
            &payload,
            &context,
        )
        .is_none()
    );
}
