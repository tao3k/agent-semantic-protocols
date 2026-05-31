use std::env;
use std::fs;
use std::path::PathBuf;

use serde_json::json;

fn main() {
    let config = rust_lang_project_harness::default_rust_harness_config();
    println!("cargo:rerun-if-changed=../../languages/rust-lang-project-harness/Cargo.toml");
    println!("cargo:rerun-if-changed=../../languages/rust-lang-project-harness/src/model.rs");
    println!("cargo:rerun-if-changed=../../languages/rust-lang-project-harness/src/runner.rs");
    println!(
        "cargo:rustc-env=SEMANTIC_AGENT_HOOK_RUST_SOURCE_ROOTS={}",
        config.source_dir_names.join(",")
    );

    let mut ignored_path_prefixes = config.ignored_dir_names.into_iter().collect::<Vec<_>>();
    ignored_path_prefixes.sort();

    let mut source_roots = config.source_dir_names;
    source_roots.extend(config.test_dir_names);
    source_roots.extend(["examples".to_string(), "benches".to_string()]);

    let registry = json!({
        "schemaId": "agent.semantic-protocols.semantic-agent-hook-profile-registry",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.agent-hooks",
        "protocolVersion": "1",
        "projectRoot": ".",
        "profiles": [{
            "languageId": "rust",
            "providerId": "rs-harness",
            "binary": "rs-harness",
            "namespace": "agent.semantic-protocols.languages.rust.rs-harness",
            "sourceExtensions": [".rs"],
            "configFiles": ["Cargo.toml", "Cargo.lock"],
            "sourceRoots": source_roots,
            "ignoredPathPrefixes": ignored_path_prefixes,
            "policy": {
                "blockDirectRead": true,
                "blockBroadRawSearch": true,
                "blockAgentSearchJson": true,
                "requirePrimeBeforeEdit": true
            },
            "commands": {
                "prime": {"argv": ["rs-harness", "search", "prime", "--view", "seeds", "."]},
                "owner": {
                    "argv": [
                        "rs-harness",
                        "search",
                        "owner",
                        "{path}",
                        "items",
                        "--view",
                        "seeds",
                        "."
                    ]
                },
                "text": {
                    "argv": [
                        "rs-harness",
                        "search",
                        "text",
                        "{query}",
                        "owner",
                        "tests",
                        "--view",
                        "seeds",
                        "."
                    ]
                },
                "ingest": {
                    "argv": [
                        "rs-harness",
                        "search",
                        "ingest",
                        "items",
                        "tests",
                        "--view",
                        "seeds",
                        "."
                    ],
                    "stdinMode": "pipe-candidates"
                },
                "checkChanged": {"argv": ["rs-harness", "check", "--changed", "."]}
            }
        }]
    });

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    let profile_path = out_dir.join("rust-agent-hook-profile-registry.json");
    fs::write(
        &profile_path,
        serde_json::to_string_pretty(&registry).expect("serialize rust profile registry"),
    )
    .expect("write rust profile registry");
    println!(
        "cargo:rustc-env=SEMANTIC_AGENT_HOOK_RUST_PROFILE_REGISTRY={}",
        profile_path.display()
    );
}
