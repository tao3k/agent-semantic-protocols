use semantic_agent_hook::{
    DecisionKind, HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, PROFILE_REGISTRY_SCHEMA_ID,
    PROFILE_REGISTRY_SCHEMA_VERSION, ProfileRegistry, ReasonKind, classify_hook,
    merge_profile_registries, parse_profiles, render_platform_response,
};
use serde_json::{Value, json};

fn registry_value() -> Value {
    json!({
        "schemaId": PROFILE_REGISTRY_SCHEMA_ID,
        "schemaVersion": PROFILE_REGISTRY_SCHEMA_VERSION,
        "protocolId": HOOK_PROTOCOL_ID,
        "protocolVersion": HOOK_PROTOCOL_VERSION,
        "projectRoot": ".",
        "profiles": [{
            "languageId": "typescript",
            "providerId": "ts-harness",
            "binary": "ts-harness",
            "namespace": "agent.semantic-protocols.languages.typescript.ts-harness",
            "sourceExtensions": [".ts", ".tsx"],
            "configFiles": ["package.json", "tsconfig.json"],
            "sourceRoots": ["src", "tests"],
            "ignoredPathPrefixes": ["node_modules", "dist"],
            "commands": {
                "prime": {"argv": ["ts-harness", "search", "prime", "."]},
                "owner": {"argv": ["ts-harness", "search", "owner", "{path}", "."]},
                "text": {"argv": ["ts-harness", "search", "text", "{query}", "owner", "tests", "--view", "seeds", "."]},
                "ingest": {"argv": ["ts-harness", "search", "ingest", "owner", "tests", "--view", "seeds", "."], "stdinMode": "pipe-candidates"},
                "checkChanged": {"argv": ["ts-harness", "check", "--changed", "."]}
            }
        }]
    })
}

fn registry() -> ProfileRegistry {
    parse_profiles(&registry_value().to_string()).unwrap()
}

#[test]
fn profile_registry_protocol_identity_is_validated() {
    let mut value = registry_value();
    value["schemaId"] = json!("agent.semantic-protocols.wrong-profile-registry");

    let error = parse_profiles(&value.to_string()).unwrap_err();

    assert!(format!("{error:?}").contains("schemaId"));
}

#[test]
fn profile_registry_merge_replaces_same_provider_profile() {
    let mut replacement = registry_value();
    replacement["profiles"][0]["sourceRoots"] = json!(["src", "packages"]);

    let merged = merge_profile_registries(vec![
        parse_profiles(&registry_value().to_string()).unwrap(),
        parse_profiles(&replacement.to_string()).unwrap(),
    ]);

    assert_eq!(merged.schema_id, PROFILE_REGISTRY_SCHEMA_ID);
    assert_eq!(merged.profiles.len(), 1);
    assert_eq!(
        merged.profiles[0].source_roots,
        ["src".to_string(), "packages".to_string()]
    );
}

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
    assert_eq!(response["agentHookDecision"]["decision"], "deny");
    assert_eq!(
        response["agentHookDecision"]["reasonKind"],
        "direct-source-read"
    );
}

#[test]
fn direct_read_routes_to_owner_search() {
    let decision = classify_hook(
        &registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "Read",
            "tool_input": {"path": "src/cli/agent-hooks.ts"}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::DirectSourceRead);
    assert_eq!(
        decision.routes[0].argv,
        [
            "ts-harness",
            "search",
            "owner",
            "src/cli/agent-hooks.ts",
            "."
        ]
    );
}

#[test]
fn search_json_routes_to_compact_search() {
    let decision = classify_hook(
        &registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "ts-harness search text projectRoot owner tests --json ."}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::AgentSearchJson);
    assert_eq!(
        decision.routes[0].argv,
        [
            "ts-harness",
            "search",
            "text",
            "projectRoot",
            "owner",
            "tests",
            "--view",
            "seeds",
            "."
        ]
    );
}

#[test]
fn broad_raw_search_routes_to_ingest() {
    let decision = classify_hook(
        &registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "rg -n WorkflowExecution src tests"}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::RawBroadSearch);
    assert_eq!(decision.routes[0].kind, "ingest");
}

#[test]
fn wrapper_raw_search_routes_to_ingest() {
    for command in [
        "DIRENV_SILENCE=1 direnv exec . rg -n WorkflowExecution src",
        "direnv exec . rg -n WorkflowExecution src",
        "env CODEX=1 rg -n WorkflowExecution src",
        "rtk --ultra-compact rg -n WorkflowExecution src",
        "rtk proxy rg -n WorkflowExecution src",
        "rtk run -c 'rg -n WorkflowExecution src'",
        "uv run --with ./languages/typescript-lang-project-harness rg -n WorkflowExecution src",
    ] {
        let decision = classify_hook(
            &registry(),
            "codex",
            "pre-tool",
            &json!({
                "tool_name": "functions.exec_command",
                "tool_input": {"cmd": command}
            }),
        );

        assert_eq!(decision.decision, DecisionKind::Deny, "{command}");
        assert_eq!(decision.reason_kind, ReasonKind::RawBroadSearch);
        assert_eq!(decision.routes[0].kind, "ingest");
    }
}

#[test]
fn rtk_read_routes_to_owner_search() {
    let decision = classify_hook(
        &registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "rtk read src/cli/agent-hooks.ts"}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::BulkSourceDump);
    assert_eq!(
        decision.routes[0].argv,
        [
            "ts-harness",
            "search",
            "owner",
            "src/cli/agent-hooks.ts",
            "."
        ]
    );
}

#[test]
fn non_source_extension_raw_search_is_allowed() {
    for command in [
        "rg --files -g '*.md' src",
        "rg -t markdown WorkflowExecution docs",
        "fd -e md src",
        "find . -name '*.md' -print",
    ] {
        let decision = classify_hook(
            &registry(),
            "codex",
            "pre-tool",
            &json!({
                "tool_name": "functions.exec_command",
                "tool_input": {"cmd": command}
            }),
        );

        assert_eq!(decision.decision, DecisionKind::Allow, "{command}");
    }
}

#[test]
fn source_extension_raw_search_is_denied() {
    for command in [
        "rg --files -g '*.ts' src",
        "rg -t ts WorkflowExecution src",
        "rg --type=typescript WorkflowExecution src",
        "fd -e ts src",
        "find . -name '*.ts' -print",
    ] {
        let decision = classify_hook(
            &registry(),
            "codex",
            "pre-tool",
            &json!({
                "tool_name": "functions.exec_command",
                "tool_input": {"cmd": command}
            }),
        );

        assert_eq!(decision.decision, DecisionKind::Deny, "{command}");
        assert_eq!(decision.reason_kind, ReasonKind::RawBroadSearch);
    }
}

#[test]
fn raw_search_piped_to_ingest_is_allowed() {
    let decision = classify_hook(
        &registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "rg -n WorkflowExecution src | ts-harness search ingest owner tests --view seeds ."}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Allow);
}
