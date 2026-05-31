use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use semantic_agent_hook::{
    classify_hook, parse_profiles, DecisionKind, ProfileRegistry, ReasonKind,
};
use serde_json::json;

fn generated_rust_profile_path() -> &'static str {
    env!("SEMANTIC_AGENT_HOOK_RUST_PROFILE_REGISTRY")
}

fn temp_project_root(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("semantic-agent-hook-{name}-{unique}"));
    std::fs::create_dir_all(&root).expect("create temp project root");
    root
}

fn rust_harness_profile_registry() -> ProfileRegistry {
    let contents = std::fs::read_to_string(generated_rust_profile_path())
        .expect("generated rust profile registry");
    parse_profiles(&contents).expect("valid generated rust profile registry")
}

#[test]
fn build_script_uses_rust_harness_source_roots() {
    assert_eq!(env!("SEMANTIC_AGENT_HOOK_RUST_SOURCE_ROOTS"), "src");
}

#[test]
fn generated_rust_harness_profile_uses_provider_identity() {
    let registry = rust_harness_profile_registry();
    assert_eq!(registry.profiles.len(), 1);
    let profile = &registry.profiles[0];
    assert_eq!(profile.language_id, "rust");
    assert_eq!(profile.provider_id, "rs-harness");
    assert_eq!(profile.binary, "rs-harness");
    assert!(profile.source_roots.iter().any(|root| root == "src"));
    assert!(profile
        .source_extensions
        .iter()
        .any(|extension| extension == ".rs"));
}

#[test]
fn rust_harness_profile_routes_direct_reads_to_owner_search() {
    let decision = classify_hook(
        &rust_harness_profile_registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "Read",
            "tool_input": {"path": "src/lib.rs"}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::DirectSourceRead);
    assert_eq!(
        decision.routes[0].argv,
        [
            "rs-harness",
            "search",
            "owner",
            "src/lib.rs",
            "items",
            "--view",
            "seeds",
            "."
        ]
    );
}

#[test]
fn rust_harness_profile_routes_raw_root_search_to_ingest() {
    let decision = classify_hook(
        &rust_harness_profile_registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "rg -n \"HookDecision\" ."}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::RawBroadSearch);
    assert_eq!(decision.routes[0].kind, "ingest");
    assert_eq!(
        decision.routes[0].stdin_mode.as_deref(),
        Some("pipe-candidates")
    );
}

#[test]
fn cli_doctor_accepts_generated_rust_profile_registry() {
    let output = Command::new(env!("CARGO_BIN_EXE_semantic-agent-hook"))
        .args(["doctor", "--profiles", generated_rust_profile_path()])
        .output()
        .expect("run semantic-agent-hook doctor");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("doctor stdout");
    assert!(stdout.contains("[agent-doctor] status=ok"));
    assert!(stdout.contains("profiles=1"));
    assert!(stdout.contains("|profile language=rust provider=rs-harness"));
}

#[test]
fn cli_hook_emits_decision_for_generated_rust_profile_registry() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_semantic-agent-hook"))
        .args([
            "hook",
            "--client",
            "codex",
            "pre-tool",
            "--profiles",
            generated_rust_profile_path(),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("run semantic-agent-hook hook");
    child
        .stdin
        .as_mut()
        .expect("hook stdin")
        .write_all(br#"{"tool_name":"Read","tool_input":{"path":"src/lib.rs"}}"#)
        .expect("write hook payload");

    let output = child.wait_with_output().expect("wait for hook output");

    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("hook JSON");
    assert_eq!(value["hookSpecificOutput"]["permissionDecision"], "deny");
    assert_eq!(value["agentHookDecision"]["decision"], "deny");
    assert_eq!(
        value["agentHookDecision"]["reasonKind"],
        "direct-source-read"
    );
    assert_eq!(
        value["agentHookDecision"]["routes"][0]["binary"],
        "rs-harness"
    );
    assert_eq!(
        value["agentHookDecision"]["routes"][0]["argv"][3],
        "src/lib.rs"
    );
}

#[test]
fn cli_hook_can_emit_raw_decision_for_schema_tests() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_semantic-agent-hook"))
        .args([
            "hook",
            "--client",
            "codex",
            "pre-tool",
            "--profiles",
            generated_rust_profile_path(),
            "--emit",
            "decision",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("run semantic-agent-hook hook");
    child
        .stdin
        .as_mut()
        .expect("hook stdin")
        .write_all(br#"{"tool_name":"Read","tool_input":{"path":"src/lib.rs"}}"#)
        .expect("write hook payload");

    let output = child.wait_with_output().expect("wait for hook output");

    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("hook JSON");
    assert_eq!(value["decision"], "deny");
    assert_eq!(value["reasonKind"], "direct-source-read");
}

#[test]
fn cli_install_writes_root_owned_codex_hook_config() {
    let root = temp_project_root("install");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write temp Cargo.toml");
    let output = Command::new(env!("CARGO_BIN_EXE_semantic-agent-hook"))
        .args([
            "install",
            "--client",
            "codex",
            root.to_str().expect("utf8 temp root"),
        ])
        .output()
        .expect("run semantic-agent-hook install");

    assert!(
        output.status.success(),
        "install stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("install stdout");
    assert!(stdout.contains("[agent-install] client=codex"));
    assert!(stdout.contains("profiles=.codex/semantic-agent-hook/profiles.json"));
    let config =
        std::fs::read_to_string(root.join(".codex/config.toml")).expect("installed config");
    assert!(config.contains("# BEGIN semantic-agent-hook agent hooks"));
    assert!(config.contains(".codex/semantic-agent-hook/bin/semantic-agent-hook"));
    assert!(config.contains("semantic-agent-hook hook --client codex pre-tool"));
    assert!(config.contains("--profiles \"$profiles\""));
    toml::from_str::<toml::Value>(&config).expect("installed Codex config is valid TOML");
    assert!(config.contains("fs\\\\.read"));
    assert!(!config.contains("ts-harness agent hook --client codex"));
    assert!(!config.contains("rs-harness agent hook --client codex"));
    assert!(root
        .join(".codex/semantic-agent-hook/bin/semantic-agent-hook")
        .is_file());
    let profiles = std::fs::read_to_string(root.join(".codex/semantic-agent-hook/profiles.json"))
        .expect("installed profile registry");
    let registry = parse_profiles(&profiles).expect("valid installed profile registry");
    assert_eq!(registry.profiles.len(), 1);
    assert_eq!(registry.profiles[0].language_id, "rust");
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn cli_install_refuses_to_overwrite_invalid_codex_toml() {
    let root = temp_project_root("install-invalid-toml");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write temp Cargo.toml");
    std::fs::create_dir_all(root.join(".codex")).expect("create .codex");
    let config_path = root.join(".codex/config.toml");
    std::fs::write(&config_path, "unified_exec = \"unterminated\n").expect("write invalid config");

    let output = Command::new(env!("CARGO_BIN_EXE_semantic-agent-hook"))
        .args([
            "install",
            "--client",
            "codex",
            root.to_str().expect("utf8 temp root"),
        ])
        .output()
        .expect("run semantic-agent-hook install");

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("refusing to write invalid Codex config TOML"));
    let config = std::fs::read_to_string(&config_path).expect("preserved config");
    assert_eq!(config, "unified_exec = \"unterminated\n");
    assert!(!config.contains("# BEGIN semantic-agent-hook agent hooks"));
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn cli_profiles_merge_writes_combined_registry() {
    let root = temp_project_root("profiles-merge");
    let rust_profile = root.join("rust.json");
    std::fs::copy(generated_rust_profile_path(), &rust_profile).expect("copy rust profile");
    let python_profile = root.join("python.json");
    std::fs::write(
        &python_profile,
        serde_json::to_string_pretty(&json!({
            "schemaId": semantic_agent_hook::PROFILE_REGISTRY_SCHEMA_ID,
            "schemaVersion": semantic_agent_hook::PROFILE_REGISTRY_SCHEMA_VERSION,
            "protocolId": semantic_agent_hook::HOOK_PROTOCOL_ID,
            "protocolVersion": semantic_agent_hook::HOOK_PROTOCOL_VERSION,
            "projectRoot": ".",
            "profiles": [{
                "languageId": "python",
                "providerId": "py-harness",
                "binary": "py-harness",
                "namespace": "agent.semantic-protocols.languages.python.py-harness",
                "sourceExtensions": [".py", ".pyi"],
                "configFiles": ["pyproject.toml"],
                "sourceRoots": ["src", "tests"],
                "ignoredPathPrefixes": [".venv", "__pycache__"],
                "commands": {
                    "prime": {"argv": ["py-harness", "search", "prime", "."]},
                    "owner": {"argv": ["py-harness", "search", "owner", "{path}", "."]},
                    "text": {"argv": ["py-harness", "search", "text", "{query}", "owner", "tests", "--view", "seeds", "."]},
                    "ingest": {"argv": ["py-harness", "search", "ingest", "owner", "tests", "--view", "seeds", "."], "stdinMode": "pipe-candidates"},
                    "checkChanged": {"argv": ["py-harness", "check", "--changed", "."]}
                }
            }]
        }))
        .expect("serialize python profile"),
    )
    .expect("write python profile");
    let output_path = root.join(".codex/semantic-agent-hook/profiles.json");

    let output = Command::new(env!("CARGO_BIN_EXE_semantic-agent-hook"))
        .args([
            "profiles",
            "merge",
            "--output",
            output_path.to_str().expect("utf8 output path"),
            rust_profile.to_str().expect("utf8 rust profile"),
            python_profile.to_str().expect("utf8 python profile"),
        ])
        .output()
        .expect("run semantic-agent-hook profiles merge");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("merge stdout");
    assert!(stdout.contains("[profiles-merge]"));
    assert!(stdout.contains("profiles=2"));
    let merged = std::fs::read_to_string(&output_path).expect("merged registry");
    let registry = parse_profiles(&merged).expect("valid merged registry");
    assert_eq!(registry.profiles.len(), 2);
    assert!(registry
        .profiles
        .iter()
        .any(|profile| profile.language_id == "rust"));
    assert!(registry
        .profiles
        .iter()
        .any(|profile| profile.language_id == "python"));
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}
