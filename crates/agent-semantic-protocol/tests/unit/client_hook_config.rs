use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_hook::{builtin_provider_manifests, provider_manifest_digest};
use serde_json::{Value, json};

#[test]
fn client_config_rule_can_deny_tool_use() {
    let root = temp_project_root("client-config-deny");
    let activation_path = root.join("activation.json");
    std::fs::write(&activation_path, root_owned_rust_activation_json()).expect("write activation");
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
    std::fs::write(&activation_path, root_owned_rust_activation_json()).expect("write activation");
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
    std::fs::write(&activation_path, root_owned_rust_activation_json()).expect("write activation");
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
fn invalid_client_config_blocks_tool_use() {
    let root = temp_project_root("client-config-invalid");
    let activation_path = root.join("activation.json");
    std::fs::write(&activation_path, root_owned_rust_activation_json()).expect("write activation");
    write_config(&root, "[[rules]\ninvalid");

    let decision = run_hook_decision(
        &root,
        &activation_path,
        json!({"tool_name": "Bash", "tool_input": {"command": "printf ok"}}),
    );

    assert_eq!(decision["decision"], "block");
    assert!(
        decision["message"]
            .as_str()
            .expect("message string")
            .contains("Semantic hook config could not be loaded")
    );
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn duplicate_client_config_rule_ids_block_tool_use() {
    let root = temp_project_root("client-config-duplicate-rule-id");
    let activation_path = root.join("activation.json");
    std::fs::write(&activation_path, root_owned_rust_activation_json()).expect("write activation");
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
    assert_eq!(decision["decision"], "block");
    assert!(
        decision["message"]
            .as_str()
            .expect("message string")
            .contains("duplicate client hook rule id")
    );
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn client_config_schema_shape_errors_block_tool_use() {
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
        std::fs::write(&activation_path, root_owned_rust_activation_json())
            .expect("write activation");
        write_config(&root, config);
        let decision = run_hook_decision(
            &root,
            &activation_path,
            json!({"tool_name": "Bash", "tool_input": {"command": "printf ok"}}),
        );
        assert_eq!(decision["decision"], "block", "{name}");
        assert!(
            decision["message"]
                .as_str()
                .expect("message string")
                .contains(expected_error),
            "{name}: {}",
            decision["message"]
        );
        std::fs::remove_dir_all(root).expect("cleanup temp project root");
    }
}

#[test]
fn wrong_client_config_schema_id_blocks_tool_use() {
    let root = temp_project_root("client-config-wrong-schema");
    let activation_path = root.join("activation.json");
    std::fs::write(&activation_path, root_owned_rust_activation_json()).expect("write activation");
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

    assert_eq!(decision["decision"], "block");
    assert!(
        decision["message"]
            .as_str()
            .expect("message string")
            .contains("expected schemaId=")
    );
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

fn write_config(root: &std::path::Path, content: &str) {
    let config_path = root.join(".agent-semantic-protocols/hooks/config.toml");
    std::fs::create_dir_all(config_path.parent().expect("config parent"))
        .expect("create config dir");
    std::fs::write(config_path, content).expect("write config");
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

fn root_owned_rust_activation_json() -> String {
    let manifest = builtin_provider_manifests()
        .into_iter()
        .find(|manifest| manifest.language_id == "rust")
        .expect("rust manifest");
    let manifest_digest = provider_manifest_digest(&manifest).expect("digest manifest");
    serde_json::to_string_pretty(&json!({
        "schemaId": agent_semantic_hook::HOOK_ACTIVATION_SCHEMA_ID,
        "schemaVersion": agent_semantic_hook::HOOK_ACTIVATION_SCHEMA_VERSION,
        "protocolId": agent_semantic_hook::HOOK_PROTOCOL_ID,
        "protocolVersion": agent_semantic_hook::HOOK_PROTOCOL_VERSION,
        "projectRoot": ".",
        "generatedBy": {"runtime": "agent-semantic-hook", "version": "test"},
        "providers": [{
            "manifestId": manifest.manifest_id,
            "manifestDigest": manifest_digest,
            "languageId": manifest.language_id,
            "providerId": manifest.provider_id,
            "binary": manifest.binary,
            "providerCommandPrefix": [],
            "coverage": {
                "packageRoots": ["."],
                "sourceRoots": ["src", "tests", "crates", "examples", "benches"],
                "configFiles": ["Cargo.toml", "Cargo.lock"],
                "sourceExtensions": [".rs"],
                "ignoredPathPrefixes": [
                    ".cache",
                    ".direnv",
                    ".git",
                    ".idea",
                    ".jj",
                    ".run",
                    ".vscode",
                    "node_modules",
                    "target",
                    ".codex/harness-state",
                    ".codex/rs-harness"
                ]
            }
        }]
    }))
    .expect("serialize root-owned rust activation")
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
