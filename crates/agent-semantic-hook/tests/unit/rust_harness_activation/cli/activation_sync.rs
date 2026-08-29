use agent_semantic_hook::{
    HOOK_ACTIVATION_SCHEMA_ID, HOOK_ACTIVATION_SCHEMA_VERSION, HOOK_PROTOCOL_ID,
    HOOK_PROTOCOL_VERSION, parse_hook_activation,
};
use serde_json::json;

use crate::rust_harness_activation::support::{
    asp_command, stage_project_candidates, temp_project_root, write_state_home_provider_binary,
};

#[test]
fn cli_doctor_syncs_generated_activation_drift() {
    let root = temp_project_root("doctor-activation-sync");
    let state_home = root.join(".agent-semantic-protocols");
    super::super::support::write_default_client_hook_config(&root);
    let activation_path = write_invalid_generated_activation(&root);
    write_state_home_provider_binary(&state_home, "rust", "asp-rust", "asp-rust");
    std::fs::create_dir_all(root.join("src")).expect("create Rust source fixture directory");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"activation-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("write Rust manifest fixture");
    std::fs::write(root.join("src/lib.rs"), "pub fn fixture() {}\n")
        .expect("write Rust source fixture");
    stage_project_candidates(&root);

    let output = asp_command()
        .env_remove("PRJ_CACHE_HOME")
        .env("ASP_STATE_HOME", &state_home)
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
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("[agent-doctor] status=ok")
            || stdout.contains("[agent-doctor] status=warning")
    );
    let synced = std::fs::read_to_string(&activation_path).expect("synced activation");
    let registry = parse_hook_activation(&synced).unwrap_or_else(|error| {
        panic!(
            "canonical synced activation: {error:?}\nactivationPath={}\nstdout={}\nstderr={}\nsynced={synced}",
            activation_path.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        )
    });
    let rust_provider = registry
        .providers
        .iter()
        .find(|provider| provider.provider_id == "asp-rust")
        .expect("synced rust provider");
    assert_eq!(
        rust_provider.routes.prime.argv,
        vec![
            "asp-rust",
            "search",
            "prime",
            "--workspace",
            "{workspace}",
            "--view",
            "seeds"
        ]
    );
    assert!(!synced.contains("\"stdinMode\": null"));
    assert!(!synced.contains("\"text\": {"));
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

fn write_invalid_generated_activation(root: &std::path::Path) -> std::path::PathBuf {
    let activation_path = test_activation_path(root);
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
                "providerId": "asp-rust",
                "binary": "asp-rust",
                "namespace": "agent.semantic-protocols.languages.rust.asp-rust",
                "sourceExtensions": [".rs"],
                "configFiles": ["Cargo.toml", "Cargo.lock"],
                "sourceRoots": ["src", "tests"],
                "ignoredPathPrefixes": ["target", ".git"],
                "commands": {
                    "prime": {"argv": ["asp-rust", "search", "prime", "."]},
                    "owner": {"argv": ["asp-rust", "search", "owner", "{path}", "."]},
                    "text": {"argv": ["asp-rust", "search", "text", "{query}", "."]},
                    "ingest": {"argv": ["asp-rust", "search", "ingest", "."], "stdinMode": "pipe-candidates"},
                    "checkChanged": {"argv": ["asp-rust", "check", "--changed", "."]}
                }
            }]
        }))
        .expect("serialize retired activation"),
    )
    .expect("write retired activation");
    activation_path
}

fn test_activation_path(root: &std::path::Path) -> std::path::PathBuf {
    let project_root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let resolved = agent_semantic_runtime::state_core::ResolvedState::resolve_with_state_home(
        &project_root,
        root.join(".agent-semantic-protocols"),
    )
    .expect("resolve test state");
    std::fs::create_dir_all(&resolved.paths.workspace_dir).expect("create workspace state dir");
    std::fs::write(
        &resolved.paths.workspace_json,
        serde_json::to_string(&serde_json::json!({
            "root": project_root.display().to_string()
        }))
        .expect("serialize workspace manifest"),
    )
    .expect("write workspace manifest");
    resolved
        .paths
        .hooks_dir
        .join("state")
        .join("activation.json")
}
