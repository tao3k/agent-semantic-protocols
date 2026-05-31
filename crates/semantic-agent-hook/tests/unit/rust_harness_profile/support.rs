use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use semantic_agent_hook::{ProfileRegistry, parse_profiles};
use serde_json::json;

pub(super) fn temp_project_root(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("semantic-agent-hook-{name}-{unique}"));
    std::fs::create_dir_all(&root).expect("create temp project root");
    root
}

pub(super) fn root_owned_rust_profile_registry_json() -> String {
    serde_json::to_string_pretty(&json!({
        "schemaId": semantic_agent_hook::PROFILE_REGISTRY_SCHEMA_ID,
        "schemaVersion": semantic_agent_hook::PROFILE_REGISTRY_SCHEMA_VERSION,
        "protocolId": semantic_agent_hook::HOOK_PROTOCOL_ID,
        "protocolVersion": semantic_agent_hook::HOOK_PROTOCOL_VERSION,
        "projectRoot": ".",
        "profiles": [{
            "languageId": "rust",
            "providerId": "rs-harness",
            "binary": "rs-harness",
            "namespace": "agent.semantic-protocols.languages.rust.rs-harness",
            "sourceExtensions": [".rs"],
            "configFiles": ["Cargo.toml", "Cargo.lock"],
            "sourceRoots": ["src", "tests", "crates", "examples", "benches"],
            "ignoredPathPrefixes": [".cache", ".direnv", ".git", ".idea", ".jj", ".run", ".vscode", "node_modules", "target", ".codex/harness-state", ".codex/rs-harness"],
            "policy": {
                "blockDirectRead": true,
                "blockBroadRawSearch": true,
                "blockAgentSearchJson": true,
                "requirePrimeBeforeEdit": true
            },
            "commands": {
                "prime": {"argv": ["rs-harness", "search", "prime", "--view", "seeds", "."]},
                "owner": {"argv": ["rs-harness", "search", "owner", "{path}", "items", "--view", "seeds", "."]},
                "text": {"argv": ["rs-harness", "search", "text", "{query}", "tests", "--view", "seeds", "."]},
                "ingest": {"argv": ["rs-harness", "search", "ingest", "items", "tests", "--view", "seeds", "."], "stdinMode": "pipe-candidates"},
                "checkChanged": {"argv": ["rs-harness", "check", "--changed", "."]}
            }
        }]
    }))
    .expect("serialize root-owned rust profile registry")
}

pub(super) fn write_root_owned_rust_profile_registry(root: &std::path::Path) -> PathBuf {
    let path = root.join("rust-profile-registry.json");
    std::fs::write(&path, root_owned_rust_profile_registry_json()).expect("write rust profile");
    path
}

pub(super) fn rust_harness_profile_registry() -> ProfileRegistry {
    parse_profiles(&root_owned_rust_profile_registry_json())
        .expect("valid root-owned rust profile registry")
}
