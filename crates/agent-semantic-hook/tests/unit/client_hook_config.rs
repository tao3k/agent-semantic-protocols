use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use agent_semantic_hook::{
    DecisionKind, HookClassificationRequest, classify_hook_with_config,
    default_client_config_template, load_client_config,
};
use serde_json::json;

use crate::classifier::registry;

#[test]
fn argv_source_glob_rule_matches_source_argument_after_flags() {
    let root = temp_root("argv-source-glob");
    let config_path = root.join("config.toml");
    fs::write(
        &config_path,
        r#"
schemaId = "agent.semantic-protocols.hook.client-config"
schemaVersion = "1"
protocolId = "agent.semantic-protocols.hook"
protocolVersion = "1"

[[rules]]
id = "deny-wl-source-argv"
decision = "deny"
message = "matched configured argv source"

[rules.match]
tool = "Bash"
commandAny = ["wl"]
argvSourceGlobAny = ["*.ts"]
argvSourceExcludeFlagAny = ["--output"]
"#,
    )
    .expect("write config");
    let config = load_client_config(&config_path).expect("load client config");
    let registry = registry();

    let decision = classify_hook_with_config(HookClassificationRequest {
        registry: &registry,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &json!({
            "tool_name": "Bash",
            "tool_input": {"command": "wl --flag2 flag3 *.ts"}
        }),
    });

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(
        decision
            .fields
            .get("configRuleId")
            .and_then(|id| id.as_str()),
        Some("deny-wl-source-argv")
    );

    for command in [
        "wl --flag2 flag3 README",
        "wl --output *.ts README",
        "wl --output=*.ts README",
    ] {
        let decision = classify_hook_with_config(HookClassificationRequest {
            registry: &registry,
            config: &config,
            platform: "codex",
            event: "pre-tool",
            payload: &json!({
                "tool_name": "Bash",
                "tool_input": {"command": command}
            }),
        });

        assert_eq!(decision.decision, DecisionKind::Allow, "{command}");
    }

    let decision = classify_hook_with_config(HookClassificationRequest {
        registry: &registry,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &json!({
            "tool_name": "Bash",
            "tool_input": {"command": "wl --output ignored.txt source.ts"}
        }),
    });

    assert_eq!(decision.decision, DecisionKind::Deny);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn default_source_argv_rule_matches_command_names_not_harness_subcommands() {
    let root = temp_root("default-source-argv-command-name");
    let config_path = root.join("config.toml");
    fs::write(&config_path, default_client_config_template()).expect("write config");
    let config = load_client_config(&config_path).expect("load client config");
    let registry = registry();

    let asp_rg_decision = classify_hook_with_config(HookClassificationRequest {
        registry: &registry,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &json!({
            "tool_name": "Bash",
            "tool_input": {"command": "asp rg -query 'HookDecision' crates/agent-semantic-hook/src/hook_config.rs"}
        }),
    });

    assert_eq!(asp_rg_decision.decision, DecisionKind::Allow);

    let direct_rg_decision = classify_hook_with_config(HookClassificationRequest {
        registry: &registry,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &json!({
            "tool_name": "Bash",
            "tool_input": {"command": "rg HookDecision crates/agent-semantic-hook/src/hook_config.rs"}
        }),
    });

    assert_eq!(direct_rg_decision.decision, DecisionKind::Deny);
    assert_eq!(
        direct_rg_decision
            .fields
            .get("configRuleId")
            .and_then(|id| id.as_str()),
        Some("deny-shell-source-argv")
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn command_contains_any_rejects_empty_patterns() {
    let root = temp_root("command-contains-empty-pattern");
    let config_path = root.join("config.toml");
    fs::write(
        &config_path,
        r#"
schemaId = "agent.semantic-protocols.hook.client-config"
schemaVersion = "1"
protocolId = "agent.semantic-protocols.hook"
protocolVersion = "1"

[[rules]]
id = "deny-empty-command-contains"
decision = "deny"

[rules.match]
tool = "Bash"
commandContainsAny = [""]
"#,
    )
    .expect("write config");

    let error = load_client_config(&config_path).expect_err("reject empty commandContainsAny");
    assert!(
        error.contains("rules[].match.commandContainsAny[] must not be empty"),
        "{error}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn command_contains_any_matches_ascii_case_insensitively() {
    let root = temp_root("command-contains-case-insensitive");
    let config_path = root.join("config.toml");
    fs::write(
        &config_path,
        r#"
schemaId = "agent.semantic-protocols.hook.client-config"
schemaVersion = "1"
protocolId = "agent.semantic-protocols.hook"
protocolVersion = "1"

[[rules]]
id = "deny-case-insensitive-command-contains"
decision = "deny"

[rules.match]
tool = "Bash"
commandContainsAny = ["HOOKDECISION"]
"#,
    )
    .expect("write config");
    let config = load_client_config(&config_path).expect("load client config");
    let registry = registry();

    let decision = classify_hook_with_config(HookClassificationRequest {
        registry: &registry,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &json!({
            "tool_name": "Bash",
            "tool_input": {
                "command": "rg hookdecision crates/agent-semantic-hook/src/hook_config.rs"
            }
        }),
    });

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(
        decision
            .fields
            .get("configRuleId")
            .and_then(|id| id.as_str()),
        Some("deny-case-insensitive-command-contains")
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn configurable_hook_default_rule_classification_stays_fast() {
    let root = temp_root("default-source-argv-perf");
    let config_path = root.join("config.toml");
    fs::write(&config_path, default_client_config_template()).expect("write config");
    let config = load_client_config(&config_path).expect("load client config");
    let registry = registry();
    let payloads = [
        json!({
            "tool_name": "Bash",
            "tool_input": {"command": "rg HookDecision crates/agent-semantic-hook/src/hook_config.rs"}
        }),
        json!({
            "tool_name": "Bash",
            "tool_input": {"command": "sed -n '1,40p' crates/agent-semantic-hook/src/hook_config.rs"}
        }),
        json!({
            "tool_name": "Bash",
            "tool_input": {"command": "wl --output crates/agent-semantic-hook/src/hook_config.rs README.md"}
        }),
        json!({
            "tool_name": "Bash",
            "tool_input": {"command": "asp rg -query 'HookDecision' crates/agent-semantic-hook/src/hook_config.rs"}
        }),
    ];
    let samples = 4;
    let iterations = 20_000;
    let mut best_elapsed = Duration::MAX;
    let mut best_denied = 0usize;

    for _ in 0..samples {
        let start = Instant::now();
        let mut denied = 0usize;
        for index in 0..iterations {
            let decision = classify_hook_with_config(HookClassificationRequest {
                registry: &registry,
                config: &config,
                platform: "codex",
                event: "pre-tool",
                payload: &payloads[index % payloads.len()],
            });
            if decision.decision == DecisionKind::Deny {
                denied += 1;
            }
        }
        let elapsed = start.elapsed();
        if elapsed < best_elapsed {
            best_elapsed = elapsed;
            best_denied = denied;
        }
    }

    let per_decision = best_elapsed.as_nanos() / iterations as u128;
    eprintln!(
        "configurable_hook_default_rule_perf samples={samples} iterations={iterations} best_elapsed_ms={} best_ns_per_decision={per_decision}",
        best_elapsed.as_millis()
    );

    assert_eq!(best_denied, iterations / 2);
    assert!(
        best_elapsed < Duration::from_millis(5_000),
        "configurable hook classification regressed: {best_elapsed:?} for {iterations} iterations"
    );

    let _ = fs::remove_dir_all(root);
}

fn temp_root(label: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("agent-semantic-hook-{label}-{nonce}"));
    fs::create_dir_all(&root).expect("create temp root");
    canonical(&root)
}

fn canonical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}
