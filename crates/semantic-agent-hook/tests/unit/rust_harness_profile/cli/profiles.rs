use std::process::Command;

use semantic_agent_hook::parse_profiles;
use serde_json::json;

use crate::rust_harness_profile::support::{
    root_owned_rust_profile_registry_json, temp_project_root,
};

#[test]
fn cli_profiles_merge_writes_combined_registry() {
    let root = temp_project_root("profiles-merge");
    let rust_profile = root.join("rust.json");
    std::fs::write(&rust_profile, root_owned_rust_profile_registry_json())
        .expect("write rust profile");
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
                    "ingest": {"argv": ["py-harness", "search", "ingest", "."], "stdinMode": "pipe-candidates"},
                    "checkChanged": {"argv": ["py-harness", "check", "--changed", "."]},
                    "guide": {"argv": ["py-harness", "agent", "guide", "."]}
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
    assert!(
        registry
            .profiles
            .iter()
            .any(|profile| profile.language_id == "rust")
    );
    assert!(
        registry
            .profiles
            .iter()
            .any(|profile| profile.language_id == "python")
    );
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}
