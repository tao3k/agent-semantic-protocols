use semantic_agent_hook::{
    DecisionKind, HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, PROFILE_REGISTRY_SCHEMA_ID,
    PROFILE_REGISTRY_SCHEMA_VERSION, ProfileRegistry, ReasonKind, classify_hook, parse_profiles,
};
use serde_json::json;

pub(super) fn assert_raw_search_denied(command: &str, binary: &str) {
    let decision = classify_hook(
        &polyglot_registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": command}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny, "{command}");
    assert_eq!(decision.reason_kind, ReasonKind::RawBroadSearch);
    assert_eq!(decision.routes[0].binary, binary, "{command}");
}

pub(super) fn assert_direct_read_denied(command: &str, binary: &str) {
    let decision = classify_hook(
        &polyglot_registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": command}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny, "{command}");
    assert_eq!(decision.reason_kind, ReasonKind::DirectSourceRead);
    assert_eq!(decision.routes[0].binary, binary, "{command}");
}

pub(super) fn assert_content_dump_denied(command: &str, binary: &str) {
    let decision = classify_hook(
        &polyglot_registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": command}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny, "{command}");
    assert_eq!(decision.reason_kind, ReasonKind::BulkSourceDump);
    assert_eq!(decision.routes[0].binary, binary, "{command}");
}

pub(super) fn assert_allowed(command: &str) {
    let decision = classify_hook(
        &polyglot_registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": command}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Allow, "{command}");
}

pub(super) fn rust_registry() -> ProfileRegistry {
    parse_profiles(
        &json!({
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
                "sourceRoots": ["src", "tests", "crates"],
                "ignoredPathPrefixes": ["target", ".git"],
                "commands": {
                    "prime": {"argv": ["rs-harness", "search", "prime", "--view", "seeds", "."]},
                    "owner": {"argv": ["rs-harness", "search", "owner", "{path}", "items", "--view", "seeds", "."]},
                    "text": {"argv": ["rs-harness", "search", "text", "{query}", "tests", "--view", "seeds", "."]},
                    "ingest": {"argv": ["rs-harness", "search", "ingest", "items", "tests", "--view", "seeds", "."], "stdinMode": "pipe-candidates"},
                    "checkChanged": {"argv": ["rs-harness", "check", "--changed", "."]},
                    "guide": {"argv": ["rs-harness", "agent", "guide", "."]}
                }
            }]
        })
        .to_string(),
    )
    .expect("valid rust registry")
}

pub(super) fn polyglot_registry() -> ProfileRegistry {
    parse_profiles(&polyglot_registry_json().to_string()).expect("valid polyglot registry")
}

fn polyglot_registry_json() -> serde_json::Value {
    json!({
        "schemaId": PROFILE_REGISTRY_SCHEMA_ID,
        "schemaVersion": PROFILE_REGISTRY_SCHEMA_VERSION,
        "protocolId": HOOK_PROTOCOL_ID,
        "protocolVersion": HOOK_PROTOCOL_VERSION,
        "projectRoot": ".",
        "profiles": [
            language_profile(
                "typescript",
                "ts-harness",
                "agent.semantic-protocols.languages.typescript.ts-harness",
                &[".ts", ".tsx", ".mts", ".cts", ".js", ".jsx", ".mjs", ".cjs"],
                &["package.json", "tsconfig.json"],
                &["src", "test", "tests", "__tests__", "packages", "apps"],
                &["node_modules", "dist", ".git"]
            ),
            language_profile(
                "rust",
                "rs-harness",
                "agent.semantic-protocols.languages.rust.rs-harness",
                &[".rs"],
                &["Cargo.toml", "Cargo.lock"],
                &["src", "tests", "crates", "examples", "benches"],
                &["target", ".git"]
            ),
            language_profile(
                "python",
                "py-harness",
                "agent.semantic-protocols.languages.python.py-harness",
                &[".py", ".pyi"],
                &["pyproject.toml", "setup.py", "setup.cfg"],
                &["src", "test", "tests", "packages"],
                &[".venv", "__pycache__", ".git"]
            )
        ]
    })
}

fn language_profile(
    language_id: &str,
    binary: &str,
    namespace: &str,
    source_extensions: &[&str],
    config_files: &[&str],
    source_roots: &[&str],
    ignored_path_prefixes: &[&str],
) -> serde_json::Value {
    let owner_argv = if language_id == "rust" {
        vec![
            binary,
            "query",
            "--from-hook",
            "direct-source-read",
            "--selector",
            "{path}",
            ".",
        ]
    } else if language_id == "typescript" {
        vec![
            binary, "search", "owner", "{path}", "items", "--query", "{query}", ".",
        ]
    } else {
        vec![binary, "search", "owner", "{path}", "."]
    };
    json!({
        "languageId": language_id,
        "providerId": binary,
        "binary": binary,
        "namespace": namespace,
        "sourceExtensions": source_extensions,
        "configFiles": config_files,
        "sourceRoots": source_roots,
        "ignoredPathPrefixes": ignored_path_prefixes,
        "commands": {
            "prime": {"argv": [binary, "search", "prime", "."]},
            "owner": {"argv": owner_argv},
            "text": {"argv": [binary, "search", "text", "{query}", "owner", "tests", "--view", "seeds", "."]},
            "ingest": {"argv": [binary, "search", "ingest", "owner", "tests", "--view", "seeds", "."], "stdinMode": "pipe-candidates"},
            "checkChanged": {"argv": [binary, "check", "--changed", "."]},
            "guide": {"argv": [binary, "agent", "guide", "."]}
        }
    })
}
