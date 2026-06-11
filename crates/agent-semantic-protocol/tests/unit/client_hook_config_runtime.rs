use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_hook::{builtin_provider_manifests, provider_manifest_digest};
use serde_json::{Value, json};

#[test]
fn explicit_client_config_path_is_loaded() {
    let root = temp_project_root("client-config-explicit-path");
    let activation_path = root.join("activation.json");
    let config_path = root.join("custom-hook-config.toml");
    std::fs::write(&activation_path, root_owned_rust_activation_json()).expect("write activation");
    std::fs::write(
        &config_path,
        r#"
[[rules]]
id = "explicit-config-deny"
decision = "deny"
message = "explicit config loaded"
[rules.match]
tool = "Bash"
"#,
    )
    .expect("write explicit config");

    let decision = run_hook_decision_with_args(
        &root,
        &activation_path,
        "pre-tool",
        &[("--config", config_path.to_str().expect("utf8 config path"))],
        json!({"tool_name": "Bash", "tool_input": {"command": "printf ok"}}),
    );

    assert_eq!(decision["decision"], "deny");
    assert_eq!(decision["message"], "explicit config loaded");
    assert_eq!(decision["fields"]["configRuleId"], "explicit-config-deny");
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn client_config_is_reloaded_on_each_hook_invocation() {
    let root = temp_project_root("client-config-reload");
    let activation_path = root.join("activation.json");
    std::fs::write(&activation_path, root_owned_rust_activation_json()).expect("write activation");
    write_config(
        &root,
        r#"
[[rules]]
id = "reload-deny"
enabled = false
decision = "deny"
message = "reload config denied"
[rules.match]
tool = "Bash"
"#,
    );
    let allow_decision = run_hook_decision(
        &root,
        &activation_path,
        json!({"tool_name": "Bash", "tool_input": {"command": "printf ok"}}),
    );
    assert_eq!(allow_decision["decision"], "allow");

    write_config(
        &root,
        r#"
[[rules]]
id = "reload-deny"
enabled = true
decision = "deny"
message = "reload config denied"
[rules.match]
tool = "Bash"
"#,
    );
    let deny_decision = run_hook_decision(
        &root,
        &activation_path,
        json!({"tool_name": "Bash", "tool_input": {"command": "printf ok"}}),
    );
    assert_eq!(deny_decision["decision"], "deny");
    assert_eq!(deny_decision["message"], "reload config denied");
    assert_eq!(deny_decision["fields"]["configRuleId"], "reload-deny");
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn hook_runtime_blocks_source_apply_patch_but_allows_non_source_patch() {
    let root = temp_project_root("source-apply-patch-gate");
    let activation_path = root.join("activation.json");
    std::fs::write(&activation_path, root_owned_rust_activation_json()).expect("write activation");
    write_config(&root, "");

    let source_command = r#"apply_patch <<'PATCH'
*** Begin Patch
*** Update File: src/lib.rs
@@
-fn old() {}
+fn new() {}
*** End Patch
PATCH
"#;
    let deny_decision = run_hook_decision(
        &root,
        &activation_path,
        json!({
            "tool_name": "functions.exec_command",
            "tool_input": { "cmd": source_command }
        }),
    );
    assert_eq!(deny_decision["decision"], "deny");
    assert_eq!(deny_decision["reasonKind"], "semantic-ast-patch-required");
    assert_eq!(deny_decision["subject"]["paths"], json!(["src/lib.rs"]));
    assert_eq!(deny_decision["routes"][0]["providerId"], "rs-harness");
    assert_eq!(deny_decision["routes"][0]["argv"][0], "asp");
    assert_eq!(deny_decision["routes"][0]["argv"][1], "rust");
    assert_eq!(deny_decision["routes"][0]["argv"][4], "src/lib.rs");
    assert!(
        deny_decision["message"]
            .as_str()
            .unwrap()
            .contains("asp rust ast-patch dry-run")
    );

    let docs_command = r#"apply_patch <<'PATCH'
*** Begin Patch
*** Update File: README.md
@@
-old
+new
*** End Patch
PATCH
"#;
    let allow_decision = run_hook_decision(
        &root,
        &activation_path,
        json!({
            "tool_name": "functions.exec_command",
            "tool_input": { "cmd": docs_command }
        }),
    );
    assert_eq!(allow_decision["decision"], "allow");
    assert_eq!(allow_decision["reasonKind"], "none");
    std::fs::remove_dir_all(root).expect("remove temp project root");
}

#[test]
fn client_config_event_platform_and_language_filters_must_match() {
    let root = temp_project_root("client-config-filter");
    let activation_path = root.join("activation.json");
    std::fs::write(&activation_path, root_owned_rust_activation_json()).expect("write activation");
    write_config(
        &root,
        r#"
[[rules]]
id = "wrong-event"
event = "post-tool"
decision = "deny"
message = "wrong event"
[rules.match]
tool = "Bash"

[[rules]]
id = "wrong-platform"
platform = "claude"
decision = "deny"
message = "wrong platform"
[rules.match]
tool = "Bash"

[[rules]]
id = "wrong-language"
languageIds = ["typescript"]
decision = "deny"
message = "wrong language"
[rules.match]
tool = "Bash"
pathGlobAny = ["**/*.rs"]
"#,
    );

    let decision = run_hook_decision(
        &root,
        &activation_path,
        json!({"tool_name": "Bash", "tool_input": {"command": "printf ok", "path": "README.md"}}),
    );

    assert_eq!(decision["decision"], "allow");
    assert_ne!(decision["message"], "wrong event");
    assert_ne!(decision["message"], "wrong platform");
    assert_ne!(decision["message"], "wrong language");
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn client_config_priority_selects_highest_matching_rule() {
    let root = temp_project_root("client-config-priority");
    let activation_path = root.join("activation.json");
    std::fs::write(&activation_path, root_owned_rust_activation_json()).expect("write activation");
    write_config(
        &root,
        r#"
[[rules]]
id = "low"
priority = 1
decision = "deny"
message = "low priority"
[rules.match]
tool = "Bash"

[[rules]]
id = "high"
priority = 100
decision = "block"
message = "high priority"
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
    assert_eq!(decision["message"], "high priority");
    assert_eq!(decision["fields"]["configRuleId"], "high");
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn client_config_equal_priority_preserves_config_order() {
    let root = temp_project_root("client-config-equal-priority");
    let activation_path = root.join("activation.json");
    std::fs::write(&activation_path, root_owned_rust_activation_json()).expect("write activation");
    write_config(
        &root,
        r#"
[[rules]]
id = "first"
priority = 10
decision = "deny"
message = "first matching rule"
[rules.match]
tool = "Bash"

[[rules]]
id = "second"
priority = 10
decision = "block"
message = "second matching rule"
[rules.match]
tool = "Bash"
"#,
    );
    let decision = run_hook_decision(
        &root,
        &activation_path,
        json!({"tool_name": "Bash", "tool_input": {"command": "printf ok"}}),
    );
    assert_eq!(decision["decision"], "deny");
    assert_eq!(decision["message"], "first matching rule");
    assert_eq!(decision["fields"]["configRuleId"], "first");
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn client_config_routes_can_override_provider_command_shape() {
    let root = temp_project_root("client-config-route");
    let activation_path = root.join("activation.json");
    std::fs::write(&activation_path, root_owned_rust_activation_json()).expect("write activation");
    write_config(
        &root,
        r#"
[[rules]]
id = "custom-route"
decision = "deny"
reasonKind = "raw-broad-search"
message = "custom route"
[rules.match]
tool = "Bash"
[[rules.routes]]
providerId = "rs-harness"
languageId = "rust"
binary = "rs-harness"
kind = "query"
argv = ["rs-harness", "query", "--from-hook", "custom", "."]
stdinMode = "none"
"#,
    );

    let decision = run_hook_decision(
        &root,
        &activation_path,
        json!({"tool_name": "Bash", "tool_input": {"command": "printf ok"}}),
    );

    assert_eq!(decision["decision"], "deny");
    assert_eq!(decision["fields"]["configRuleId"], "custom-route");
    assert_eq!(decision["routes"][0]["kind"], "query");
    assert_eq!(
        decision["routes"][0]["argv"],
        json!(["rs-harness", "query", "--from-hook", "custom", "."])
    );
    assert_eq!(decision["routes"][0]["stdinMode"], "none");
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

fn write_config(root: &std::path::Path, content: &str) {
    let config_path = root.join(".codex/agent-semantic-protocol/hooks/config.toml");
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
