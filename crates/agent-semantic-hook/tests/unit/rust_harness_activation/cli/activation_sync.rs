use std::io::Write;
use std::path::Path;
use std::process::Stdio;

use agent_semantic_hook::{
    HOOK_ACTIVATION_SCHEMA_ID, HOOK_ACTIVATION_SCHEMA_VERSION, HOOK_PROTOCOL_ID,
    HOOK_PROTOCOL_VERSION, parse_hook_activation,
};
use serde_json::json;

use crate::rust_harness_activation::support::{
    asp_command, temp_project_root, write_fake_provider_binary,
};

#[test]
fn cli_hook_fails_closed_for_source_read_when_activation_is_missing() {
    let root = temp_project_root("hook-activation-missing-fail-closed");
    let activation_path = root.join(".cache/agent-semantic-protocol/hooks/activation.json");

    let (decision, stderr) = run_hook_with_activation(
        &activation_path,
        json!({"tool_name": "Read", "tool_input": {"file_path": "main.rs"}}),
    );

    assert_eq!(decision["decision"], "deny");
    assert_eq!(decision["reasonKind"], "direct-source-read");
    assert_eq!(decision["subject"]["toolName"], "Read");
    assert_eq!(decision["subject"]["paths"], json!(["main.rs"]));
    assert!(
        decision["message"]
            .as_str()
            .expect("message")
            .contains("source reads fail closed")
    );
    assert!(stderr.contains("activation disabled for this hook event"));
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn cli_hook_allows_non_source_read_when_activation_is_missing() {
    let root = temp_project_root("hook-activation-missing-non-source-allow");
    let activation_path = root.join(".cache/agent-semantic-protocol/hooks/activation.json");

    let (decision, stderr) = run_hook_with_activation(
        &activation_path,
        json!({"tool_name": "Read", "tool_input": {"file_path": "README.md"}}),
    );

    assert_eq!(decision["decision"], "allow");
    assert_eq!(decision["reasonKind"], "none");
    assert!(
        decision["message"]
            .as_str()
            .expect("message")
            .contains("allowing tool use so activation can be repaired")
    );
    assert!(stderr.contains("activation disabled for this hook event"));
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn cli_hook_fails_closed_on_generated_activation_drift_for_source_read() {
    let root = temp_project_root("hook-activation-drift-fail-closed");
    let activation_path = write_invalid_generated_activation(&root);
    let (decision, stderr) = run_hook_with_activation(
        &activation_path,
        json!({"tool_name": "Read", "tool_input": {"path": "src/lib.rs"}}),
    );

    assert_eq!(decision["decision"], "deny");
    assert_eq!(decision["reasonKind"], "direct-source-read");
    assert_eq!(decision["subject"]["paths"], json!(["src/lib.rs"]));
    assert!(
        decision["message"]
            .as_str()
            .expect("message")
            .contains("source reads fail closed")
    );
    assert!(stderr.contains("activation disabled for this hook event"));
    assert!(!stderr.contains("syncing generated activation"));
    let unchanged = std::fs::read_to_string(&activation_path).expect("invalid activation");
    assert!(unchanged.contains("\"text\""));
    assert!(unchanged.contains("\"argv\""));
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn cli_doctor_syncs_generated_activation_drift() {
    let root = temp_project_root("doctor-activation-sync");
    let activation_path = write_invalid_generated_activation(&root);
    let provider_path = write_fake_provider_binary(&root, "rs-harness");

    let output = asp_command()
        .env_remove("PRJ_CACHE_HOME")
        .env("PATH", &provider_path)
        .args([
            "hook",
            "doctor",
            "--client",
            "codex",
            root.to_str().expect("utf8 temp root"),
        ])
        .output()
        .expect("run agent-semantic-protocol doctor");

    assert!(
        output.status.success(),
        "doctor stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("syncing generated activation"));
    let synced = std::fs::read_to_string(&activation_path).expect("synced activation");
    let registry = parse_hook_activation(&synced).expect("canonical synced activation");
    assert_eq!(registry.providers.len(), 1);
    assert_eq!(registry.providers[0].provider_id, "rs-harness");
    assert_eq!(
        registry.providers[0].routes.prime.argv,
        vec![
            "rs-harness",
            "search",
            "prime",
            "--view",
            "seeds",
            "{projectRoot}"
        ]
    );
    assert!(!synced.contains("\"stdinMode\": null"));
    assert!(!synced.contains("\"text\": {"));
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

fn write_invalid_generated_activation(root: &std::path::Path) -> std::path::PathBuf {
    let activation_path = root.join(".cache/agent-semantic-protocol/hooks/activation.json");
    std::fs::create_dir_all(activation_path.parent().expect("activation parent"))
        .expect("create activation dir");
    std::fs::write(
        &activation_path,
        serde_json::to_string_pretty(&json!({
            "schemaId": HOOK_ACTIVATION_SCHEMA_ID,
            "schemaVersion": HOOK_ACTIVATION_SCHEMA_VERSION,
            "protocolId": HOOK_PROTOCOL_ID,
            "protocolVersion": HOOK_PROTOCOL_VERSION,
            "projectRoot": ".",
        "generatedBy": {"runtime": "agent-semantic-hook", "version": "test"},
            "activation": [{
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
        .expect("serialize legacy activation"),
    )
    .expect("write legacy activation");
    activation_path
}

fn run_hook_with_activation(
    activation_path: &Path,
    payload: serde_json::Value,
) -> (serde_json::Value, String) {
    let mut child = asp_command()
        .env("PATH", "")
        .args([
            "hook",
            "--client",
            "codex",
            "pre-tool",
            "--activation",
            activation_path.to_str().expect("utf8 activation path"),
            "--emit",
            "decision",
        ])
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

    let output = child.wait_with_output().expect("wait for hook output");

    assert!(
        output.status.success(),
        "hook stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let decision = serde_json::from_slice(&output.stdout).expect("hook decision JSON");
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (decision, stderr)
}
