use agent_semantic_hook::{classify_hook, render_platform_response};
use serde_json::json;

use super::registry;

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
fn read_only_subagent_write_denial_uses_sandbox_permission_context() {
    let payload = serde_json::json!({
        "session_id": "child-session",
        "tool_name": "Write",
        "tool_input": {
            "path": "src/lib.rs"
        }
    });
    let context = agent_semantic_hook::HookSubagentPermissionContext {
        resident_enabled: true,
        managed_child_name: "asp-explore",
        configured_codex_agent_name: "asp_explorer",
        configured_role: "asp_explorer",
        codex_hook_agent_id: Some("child-agent"),
        codex_hook_agent_type: Some("explorer"),
        resident_child_identity_proof: Some("codex-hook-payload-live-target"),
        resident_child_session_id: Some("child-session"),
        identity_status: "live-target-verified",
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
        resident_enabled: false,
        managed_child_name: "asp-explore",
        configured_codex_agent_name: "asp_explorer",
        configured_role: "asp_explorer",
        codex_hook_agent_id: Some("user-subagent"),
        codex_hook_agent_type: Some("default"),
        resident_child_identity_proof: None,
        resident_child_session_id: None,
        identity_status: "unverified",
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
        resident_enabled: true,
        managed_child_name: "asp-explore",
        configured_codex_agent_name: "asp_explorer",
        configured_role: "asp_explorer",
        codex_hook_agent_id: Some("child-agent"),
        codex_hook_agent_type: Some("explorer"),
        resident_child_identity_proof: Some("codex-hook-payload-live-target"),
        resident_child_session_id: Some("child-session"),
        identity_status: "live-target-verified",
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
        resident_enabled: true,
        managed_child_name: "asp-explore",
        configured_codex_agent_name: "asp_explorer",
        configured_role: "asp_explorer",
        codex_hook_agent_id: Some("child-agent"),
        codex_hook_agent_type: Some("explorer"),
        resident_child_identity_proof: Some("codex-hook-payload-live-target"),
        resident_child_session_id: Some("child-session"),
        identity_status: "live-target-verified",
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
        resident_enabled: false,
        managed_child_name: "asp-explore",
        configured_codex_agent_name: "asp_explorer",
        configured_role: "asp_explorer",
        codex_hook_agent_id: Some("user-subagent"),
        codex_hook_agent_type: Some("default"),
        resident_child_identity_proof: None,
        resident_child_session_id: None,
        identity_status: "unverified",
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
