use std::io::Write;
use std::process::{Command, Stdio};

use semantic_agent_hook::{
    HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, PROFILE_REGISTRY_SCHEMA_ID,
    PROFILE_REGISTRY_SCHEMA_VERSION, parse_profiles,
};
use serde_json::json;

use crate::rust_harness_profile::support::{temp_project_root, write_fake_provider_binary};

#[test]
fn cli_hook_fails_open_on_generated_profile_registry_drift() {
    let root = temp_project_root("hook-profile-drift-fail-open");
    let profiles_path = write_legacy_generated_profile_registry(&root);
    let mut child = Command::new(env!("CARGO_BIN_EXE_semantic-agent-hook"))
        .env("PATH", "")
        .args([
            "hook",
            "--client",
            "codex",
            "pre-tool",
            "--profiles",
            profiles_path.to_str().expect("utf8 profiles path"),
            "--emit",
            "decision",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run semantic-agent-hook hook");
    child
        .stdin
        .as_mut()
        .expect("hook stdin")
        .write_all(br#"{"tool_name":"Read","tool_input":{"path":"src/lib.rs"}}"#)
        .expect("write hook payload");

    let output = child.wait_with_output().expect("wait for hook output");

    assert!(
        output.status.success(),
        "hook stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let decision: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("hook decision JSON");
    assert_eq!(decision["decision"], "allow");
    assert_eq!(decision["reasonKind"], "none");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("profile registry disabled for this hook event"));
    assert!(!stderr.contains("syncing generated profile registry"));
    let unchanged = std::fs::read_to_string(&profiles_path).expect("legacy profile registry");
    assert!(unchanged.contains("\"text\""));
    assert!(unchanged.contains("\"argv\""));
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn cli_doctor_syncs_generated_profile_registry_drift() {
    let root = temp_project_root("doctor-profile-sync");
    let profiles_path = write_legacy_generated_profile_registry(&root);
    let provider_path = write_fake_provider_binary(&root, "rs-harness");

    let output = Command::new(env!("CARGO_BIN_EXE_semantic-agent-hook"))
        .env("PATH", &provider_path)
        .args([
            "doctor",
            "--client",
            "codex",
            root.to_str().expect("utf8 temp root"),
        ])
        .output()
        .expect("run semantic-agent-hook doctor");

    assert!(
        output.status.success(),
        "doctor stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("syncing generated profile registry"));
    let synced = std::fs::read_to_string(&profiles_path).expect("synced profile registry");
    let registry = parse_profiles(&synced).expect("canonical synced profile registry");
    assert_eq!(registry.profiles.len(), 1);
    assert_eq!(registry.profiles[0].provider_id, "rs-harness");
    assert_eq!(
        registry.profiles[0].commands.prime.text,
        "rs-harness search prime --view seeds ."
    );
    assert!(!synced.contains("\"stdinMode\": null"));
    assert!(!synced.contains("\"text\": {"));
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

fn write_legacy_generated_profile_registry(root: &std::path::Path) -> std::path::PathBuf {
    let profiles_path = root.join(".codex/semantic-agent-hook/profiles.json");
    std::fs::create_dir_all(profiles_path.parent().expect("profiles parent"))
        .expect("create profile registry dir");
    std::fs::write(
        &profiles_path,
        serde_json::to_string_pretty(&json!({
            "schemaId": PROFILE_REGISTRY_SCHEMA_ID,
            "schemaVersion": PROFILE_REGISTRY_SCHEMA_VERSION,
            "protocolId": HOOK_PROTOCOL_ID,
            "protocolVersion": HOOK_PROTOCOL_VERSION,
            "projectRoot": ".",
            "profiles": [{
                "languageId": "rust",
                "providerId": "rs-harness",
                "binary": "rs-harness",
                "namespace": "agent.semantic-protocols.languages.rust.rs-harness",
                "sourceExtensions": [".rs"],
                "configFiles": ["Cargo.toml", "Cargo.lock"],
                "sourceRoots": ["src", "tests"],
                "ignoredPathPrefixes": ["target", ".git"],
                "commands": {
                    "prime": {"argv": ["rs-harness", "search", "prime", "."]},
                    "owner": {"argv": ["rs-harness", "search", "owner", "{path}", "."]},
                    "text": {"argv": ["rs-harness", "search", "text", "{query}", "."]},
                    "ingest": {"argv": ["rs-harness", "search", "ingest", "."], "stdinMode": "pipe-candidates"},
                    "checkChanged": {"argv": ["rs-harness", "check", "--changed", "."]}
                }
            }]
        }))
        .expect("serialize legacy profile registry"),
    )
    .expect("write legacy profile registry");
    profiles_path
}
