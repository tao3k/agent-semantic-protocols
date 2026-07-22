use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

#[test]
fn client_config_rule_can_deny_tool_use() {
    let root = temp_project_root("client-config-deny");
    let activation_path = root.join("activation.json");
    std::fs::write(&activation_path, root_owned_rust_activation_json(&root))
        .expect("write activation");
    write_config(
        &root,
        r#"
schemaId = "agent.semantic-protocols.hook.client-config"
schemaVersion = "1"
protocolId = "agent.semantic-protocols.hook"
protocolVersion = "1"

[[rules]]
id = "deny-custom-command"
event = "pre-tool"
decision = "deny"
reasonKind = "raw-broad-search"
message = "custom config deny"

[rules.match]
tool = "Bash"
commandContainsAny = ["custom-config-deny"]

[[rules.routes]]
providerId = "rs-harness"
languageId = "rust"
binary = "asp"
kind = "ingest"
argv = ["asp", "rust", "search", "ingest", "items", "tests", "--view", "seeds", "."]
stdinMode = "pipe-candidates"
"#,
    );

    let decision = run_hook_decision(
        &root,
        &activation_path,
        json!({"tool_name": "Bash", "tool_input": {"command": "printf custom-config-deny"}}),
    );

    assert_eq!(decision["decision"], "deny");
    assert_eq!(decision["reasonKind"], "raw-broad-search");
    assert_eq!(decision["message"], "custom config deny");
    assert_eq!(decision["languageIds"], json!([]));
    assert_eq!(decision["routes"][0]["providerId"], "rs-harness");
    assert_eq!(decision["routes"][0]["binary"], "asp");
    assert_eq!(
        decision["routes"][0]["argv"],
        json!([
            "asp", "rust", "search", "ingest", "items", "tests", "--view", "seeds", "."
        ])
    );
    assert_eq!(decision["routes"][0]["stdinMode"], "pipe-candidates");
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn client_config_rule_denies_shell_alias_scheme_source_argv() {
    let root = temp_project_root("client-config-shell-scheme-source-argv");
    let activation_path = root.join("activation.json");
    std::fs::write(&activation_path, root_owned_rust_activation_json(&root))
        .expect("write activation");
    write_config(
        &root,
        r#"
[[rules]]
id = "deny-shell-source-argv"
event = "pre-tool"
decision = "deny"
reasonKind = "bulk-source-dump"
message = "Use the language harness instead of shell argv source reads."

[rules.match]
toolAny = ["Bash", "shell", "functions.exec_command", "exec_command", "command_execution"]
commandAny = ["sed", "perl", "rg", "wl"]
argvSourceGlobAny = ["*.ss", "**/*.ss", "*.scm", "**/*.scm"]
argvSourceExcludeFlagAny = ["--output", "--output-file", "--out", "-o"]
"#,
    );

    let decision = run_hook_decision(
        &root,
        &activation_path,
        json!({
            "tool_name": "shell",
            "tool_input": {
                "command": "rg -n -xx self-apply-findings.ss | sed -n '1,10p'"
            }
        }),
    );

    assert_eq!(decision["decision"], "deny");
    assert_eq!(decision["reasonKind"], "bulk-source-dump");
    assert_eq!(
        decision["subject"]["paths"],
        json!(["self-apply-findings.ss"])
    );
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn disabled_client_config_rule_does_not_fire() {
    let root = temp_project_root("client-config-disabled");
    let activation_path = root.join("activation.json");
    std::fs::write(&activation_path, root_owned_rust_activation_json(&root))
        .expect("write activation");
    write_config(
        &root,
        r#"
[[rules]]
id = "disabled"
enabled = false
decision = "deny"
message = "should not fire"

[rules.match]
tool = "Bash"
"#,
    );

    let decision = run_hook_decision(
        &root,
        &activation_path,
        json!({"tool_name": "Bash", "tool_input": {"command": "printf ok"}}),
    );

    assert_eq!(decision["decision"], "allow");
    assert_ne!(decision["message"], "should not fire");
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn invalid_client_config_auto_refresh_repairs_without_blocking_tool_use() {
    let root = temp_project_root("client-config-invalid");
    let activation_path = root.join("activation.json");
    std::fs::write(&activation_path, root_owned_rust_activation_json(&root))
        .expect("write activation");
    write_config(&root, "[[rules]\ninvalid");

    let decision = run_hook_decision(
        &root,
        &activation_path,
        json!({"tool_name": "Bash", "tool_input": {"command": "printf ok"}}),
    );

    assert_hook_config_auto_repaired(&decision, "failed to parse");
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn missing_resident_route_auto_refreshes_then_routes_search_to_codex_profile() {
    let root = temp_project_root("client-config-missing-resident-route");
    let activation_path = root.join("activation.json");
    std::fs::write(&activation_path, root_owned_rust_activation_json(&root))
        .expect("write activation");
    write_config(
        &root,
        r#"
[agents]
residentAgents = []
"#,
    );

    let decision = run_hook_decision(
        &root,
        &activation_path,
        json!({
            "session_id": "019f-hook-auto-refresh-root",
            "transcript_path": "/tmp/rollout-hook-auto-refresh-root.jsonl",
            "cwd": root,
            "tool_name": "Bash",
            "tool_input": {"command": "asp rust search pipe owner --workspace . --view seeds"}
        }),
    );

    assert_eq!(decision["decision"], "deny", "{decision}");
    assert_eq!(
        decision["reasonKind"], "subagent-receipt-required",
        "{decision}"
    );
    assert_eq!(decision["fields"]["targetAgentName"], "asp_explorer");
    assert_eq!(decision["fields"]["residentChildName"], "asp-explore");
    assert_eq!(
        decision["fields"]["configRuleId"],
        "registered-asp-reasoning-search"
    );
    assert_eq!(decision["fields"]["intent"], "reasoning-search");
    assert_eq!(decision["fields"]["residentName"], "asp-explore");
    assert_eq!(decision["fields"]["registeredLanguageId"], "rust");
    assert_eq!(decision["fields"]["hookConfigStatus"], "refreshed-by-hook");
    assert!(
        decision["fields"]["hookConfigAutoRefresh"]
            .as_str()
            .is_some_and(|receipt| receipt.starts_with("completed:")),
        "{decision}"
    );
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn duplicate_client_config_rule_ids_auto_refresh_without_blocking_tool_use() {
    let root = temp_project_root("client-config-duplicate-rule-id");
    let activation_path = root.join("activation.json");
    std::fs::write(&activation_path, root_owned_rust_activation_json(&root))
        .expect("write activation");
    write_config(
        &root,
        r#"
[[rules]]
id = "duplicate-rule"
decision = "deny"
[rules.match]
tool = "Bash"

[[rules]]
id = "duplicate-rule"
decision = "deny"
[rules.match]
tool = "Bash"
"#,
    );
    let decision = run_hook_decision(
        &root,
        &activation_path,
        json!({"tool_name": "Bash", "tool_input": {"command": "printf ok"}}),
    );
    assert_hook_config_auto_repaired(&decision, "duplicate client hook rule id");
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn client_config_schema_shape_errors_auto_refresh_without_blocking_tool_use() {
    let cases = [
        (
            "bad-rule-id",
            r#"
[[rules]]
id = "BadRule"
decision = "deny"
"#,
            "invalid rules[].id",
        ),
        (
            "empty-message",
            r#"
[[rules]]
id = "empty-message"
decision = "deny"
message = ""
"#,
            "rules[].message must not be empty",
        ),
        (
            "bad-event",
            r#"
[[rules]]
id = "bad-event"
event = "pre_tool"
decision = "deny"
"#,
            "unsupported event",
        ),
        (
            "bad-platform",
            r#"
[[rules]]
id = "bad-platform"
platform = "Codex"
decision = "deny"
"#,
            "unsupported platform",
        ),
        (
            "duplicate-language-id",
            r#"
[[rules]]
id = "duplicate-language-id"
languageIds = ["rust", "rust"]
decision = "deny"
"#,
            "duplicate rules[].languageIds",
        ),
        (
            "empty-match-tool",
            r#"
[[rules]]
id = "empty-match-tool"
decision = "deny"
[rules.match]
tool = ""
"#,
            "rules[].match.tool must not be empty",
        ),
        (
            "empty-route-argv",
            r#"
[[rules]]
id = "empty-route-argv"
decision = "deny"

[[rules.routes]]
providerId = "rs-harness"
kind = "query"
argv = []
"#,
            "rules[].routes[].argv must contain at least one item",
        ),
        (
            "bad-route-binary",
            r#"
[[rules]]
id = "bad-route-binary"
decision = "deny"

[[rules.routes]]
providerId = "rs-harness"
binary = "../rs-harness"
kind = "query"
argv = ["rs-harness"]
"#,
            "invalid rules[].routes[].binary",
        ),
    ];
    for (name, config, expected_error) in cases {
        let root = temp_project_root(&format!("client-config-schema-shape-{name}"));
        let activation_path = root.join("activation.json");
        std::fs::write(&activation_path, root_owned_rust_activation_json(&root))
            .expect("write activation");
        write_config(&root, config);
        let decision = run_hook_decision(
            &root,
            &activation_path,
            json!({"tool_name": "Bash", "tool_input": {"command": "printf ok"}}),
        );
        assert_hook_config_auto_repaired(&decision, expected_error);
        std::fs::remove_dir_all(root).expect("cleanup temp project root");
    }
}

#[test]
fn wrong_client_config_schema_id_auto_refreshes_without_blocking_tool_use() {
    let root = temp_project_root("client-config-wrong-schema");
    let activation_path = root.join("activation.json");
    std::fs::write(&activation_path, root_owned_rust_activation_json(&root))
        .expect("write activation");
    write_config(
        &root,
        r#"
schemaId = "agent.semantic-protocols.wrong"

[[rules]]
id = "block"
decision = "block"
"#,
    );

    let decision = run_hook_decision(
        &root,
        &activation_path,
        json!({"tool_name": "Bash", "tool_input": {"command": "printf ok"}}),
    );

    assert_hook_config_auto_repaired(&decision, "expected schemaId=");
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn stale_contract_fingerprint_is_atomically_refreshed_before_the_gate() {
    let root = temp_project_root("client-config-stale-fingerprint");
    let activation_path = root.join("activation.json");
    std::fs::write(&activation_path, root_owned_rust_activation_json(&root))
        .expect("write activation");
    let expected_fingerprint = agent_semantic_config::hook_client_contract_fingerprint();
    let stale = agent_semantic_config::default_hook_client_config_template()
        .replace(&expected_fingerprint, "hook-client-v1-9a7c3cad98a8c0dc");
    assert_ne!(
        stale,
        agent_semantic_config::default_hook_client_config_template(),
        "fixture must replace the active binary fingerprint"
    );
    write_config(&root, &stale);

    let decision = run_hook_decision(
        &root,
        &activation_path,
        json!({"tool_name": "Bash", "tool_input": {"command": "printf ok"}}),
    );

    assert_hook_config_auto_repaired(&decision, "fingerprint must equal");
    let refreshed =
        std::fs::read_to_string(root.join(".agent-semantic-protocols/hooks/config.toml"))
            .expect("read refreshed config");
    assert_eq!(
        refreshed,
        agent_semantic_config::default_hook_client_config_template()
    );
    assert!(refreshed.contains(&expected_fingerprint));
    assert!(!refreshed.contains("hook-client-v1-9a7c3cad98a8c0dc"));

    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

fn write_config(root: &std::path::Path, content: &str) {
    let config_path = root.join(".agent-semantic-protocols/hooks/config.toml");
    std::fs::create_dir_all(config_path.parent().expect("config parent"))
        .expect("create config dir");
    std::fs::write(config_path, content).expect("write config");
}

fn assert_hook_config_auto_repaired(decision: &Value, expected_reason: &str) {
    assert_eq!(decision["decision"], "allow", "{decision}");
    assert_eq!(
        decision["fields"]["hookConfigStatus"], "refreshed-by-hook",
        "{decision}"
    );
    assert!(
        decision["fields"]["hookConfigAutoRefresh"]
            .as_str()
            .is_some_and(|receipt| receipt.starts_with("completed:")),
        "{decision}"
    );
    assert!(
        decision["fields"]["hookConfigRepairReasons"]
            .as_array()
            .is_some_and(|reasons| reasons.iter().any(|reason| reason
                .as_str()
                .is_some_and(|reason| reason.contains(expected_reason)))),
        "{decision}"
    );
}

fn temp_project_root(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("agent-semantic-hook-{name}-{unique}"));
    std::fs::create_dir_all(&root).expect("create temp project root");
    std::fs::create_dir_all(root.join(".git")).expect("create git marker");
    root
}

fn root_owned_rust_activation_json(root: &std::path::Path) -> String {
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"hook-config-fixture\"\nversion = \"0.1.0\"\n",
    )
    .expect("write Rust project anchor");
    let provider = root.join(".bin/rs-harness");
    std::fs::create_dir_all(provider.parent().expect("provider parent"))
        .expect("create provider bin dir");
    std::fs::write(&provider, "#!/bin/sh\nexit 0\n").expect("write fixture provider");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&provider)
            .expect("provider metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&provider, permissions).expect("provider executable");
    }

    let activation =
        agent_semantic_hook::build_default_activation(root).expect("build typed activation");
    assert!(
        activation
            .providers
            .iter()
            .any(|provider| provider.language_id == "rust"),
        "typed activation must select the Rust provider"
    );
    serde_json::to_string_pretty(&activation).expect("serialize typed activation")
}

fn run_hook_decision(
    root: &std::path::Path,
    activation_path: &std::path::Path,
    payload: Value,
) -> Value {
    run_hook_decision_with_args(root, activation_path, "pre-tool", &[], payload)
}

fn run_hook_decision_with_args(
    root: &std::path::Path,
    activation_path: &std::path::Path,
    event: &str,
    extra_args: &[(&str, &str)],
    payload: Value,
) -> Value {
    let mut args = vec![
        "hook".to_string(),
        "--client".to_string(),
        "codex".to_string(),
        event.to_string(),
        "--emit".to_string(),
        "decision".to_string(),
        "--activation".to_string(),
        activation_path
            .to_str()
            .expect("utf8 activation path")
            .to_string(),
    ];
    for (name, value) in extra_args {
        args.push((*name).to_string());
        args.push((*value).to_string());
    }
    let mut child = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(root)
        .args(args)
        .env("ASP_STATE_HOME", root.join(".agent-semantic-protocols"))
        .env("CODEX_HOME", root.join(".codex-home"))
        .env("CLAUDE_HOME", root.join(".claude-home"))
        .env_remove("PRJ_CACHE_HOME")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run asp hook");
    child
        .stdin
        .as_mut()
        .expect("hook stdin")
        .write_all(payload.to_string().as_bytes())
        .expect("write hook payload");
    let output = child.wait_with_output().expect("wait for hook");
    assert!(
        output.status.success(),
        "hook stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("decision JSON")
}
