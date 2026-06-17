use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_client_core::{LanguageId, ProviderExecution, ProviderId, ResolvedProvider};
use serde_json::{Value, json};

mod prompt_output;
mod search;
mod structural_index;
mod syntax;

fn syntax_packet(input: &str, volatile_id: u64) -> Value {
    json!({
        "schemaId": "agent.semantic-protocols.semantic-tree-sitter-query",
        "method": "query",
        "languageId": "rust",
        "providerId": "rs-harness",
        "grammarId": "tree-sitter-rust",
        "grammarProfileVersion": "2026-06-04.v1",
        "query": {
            "input": input,
            "inputForm": "s-expression",
            "dialect": "tree-sitter-query",
            "fields": {
                "selector": "src/lib.rs:1:80",
                "codeOutput": false,
                "captures": ["function.name"],
                "volatileId": volatile_id
            }
        },
        "matches": []
    })
}

fn temp_root(name: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("agent-semantic-client-{name}-{unique}"));
    std::fs::create_dir_all(&root).expect("create temp root");
    root
}

fn rust_provider() -> ResolvedProvider {
    ResolvedProvider {
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("rs-harness"),
        binary: "rs-harness".to_string(),
        execution: ProviderExecution::ExternalProcess,
        provider_command_prefix: Vec::new(),
        runtime_command_argv: None,
        runtime_profile_status: None,
        package_roots: vec![".".to_string()],
        source_roots: vec!["src".to_string()],
        config_files: vec!["Cargo.toml".to_string()],
        source_extensions: vec!["rs".to_string()],
        ignored_path_prefixes: Vec::new(),
    }
}

fn python_provider() -> ResolvedProvider {
    ResolvedProvider {
        language_id: LanguageId::from("python"),
        provider_id: ProviderId::from("py-harness"),
        binary: "py-harness".to_string(),
        execution: ProviderExecution::ExternalProcess,
        provider_command_prefix: Vec::new(),
        runtime_command_argv: None,
        runtime_profile_status: None,
        package_roots: vec![".".to_string()],
        source_roots: vec!["src".to_string()],
        config_files: vec!["pyproject.toml".to_string()],
        source_extensions: vec!["py".to_string()],
        ignored_path_prefixes: Vec::new(),
    }
}

fn gerbil_scheme_provider() -> ResolvedProvider {
    ResolvedProvider {
        language_id: LanguageId::from("gerbil-scheme"),
        provider_id: ProviderId::from("gerbil-scheme-harness"),
        binary: "gerbil-scheme-harness".to_string(),
        execution: ProviderExecution::ExternalProcess,
        provider_command_prefix: Vec::new(),
        runtime_command_argv: None,
        runtime_profile_status: None,
        package_roots: vec![".".to_string()],
        source_roots: vec!["src".to_string()],
        config_files: vec!["gerbil.pkg".to_string()],
        source_extensions: vec!["ss".to_string()],
        ignored_path_prefixes: Vec::new(),
    }
}
