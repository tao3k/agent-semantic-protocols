use semantic_agent_hook::{
    HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, PROFILE_REGISTRY_SCHEMA_ID,
    PROFILE_REGISTRY_SCHEMA_VERSION, ProfileRegistry, parse_profiles,
};
use serde_json::{Value, json};

mod platform;
mod profile_registry;
mod raw_search_policy;
mod routes;

pub(crate) fn command(argv: &[&str]) -> Value {
    json!({
        "text": argv.join(" "),
        "argv": argv,
    })
}

pub(crate) fn command_with_stdin(argv: &[&str], stdin_mode: &str) -> Value {
    json!({
        "text": argv.join(" "),
        "argv": argv,
        "stdinMode": stdin_mode,
    })
}

pub(super) fn registry_value() -> Value {
    json!({
        "schemaId": PROFILE_REGISTRY_SCHEMA_ID,
        "schemaVersion": PROFILE_REGISTRY_SCHEMA_VERSION,
        "protocolId": HOOK_PROTOCOL_ID,
        "protocolVersion": HOOK_PROTOCOL_VERSION,
        "projectRoot": ".",
        "profiles": [{
            "languageId": "typescript",
            "providerId": "ts-harness",
            "binary": "ts-harness",
            "namespace": "agent.semantic-protocols.languages.typescript.ts-harness",
            "sourceExtensions": [".ts", ".tsx"],
            "configFiles": ["package.json", "tsconfig.json"],
            "sourceRoots": ["src", "tests"],
            "ignoredPathPrefixes": ["node_modules", "dist"],
            "commands": {
                "prime": command(&["ts-harness", "search", "prime", "."]),
                "owner": command(&["ts-harness", "search", "owner", "{path}", "items", "--query", "{query}", "."]),
                "fzf": command(&["ts-harness", "search", "fzf", "{query}", "owner", "tests", "--view", "seeds", "."]),
                "query": command(&["ts-harness", "search", "query", "--from-hook", "direct-source-read", "--selector", "{selector}", "{termArgs}", "--surface", "owner,tests", "--view", "seeds", "."]),
                "ingest": command_with_stdin(&["ts-harness", "search", "ingest", "owner", "tests", "--view", "seeds", "."], "pipe-candidates"),
                "checkChanged": command(&["ts-harness", "check", "--changed", "."]),
                "guide": command(&["ts-harness", "agent", "guide", "."])
            }
        }]
    })
}

pub(super) fn registry() -> ProfileRegistry {
    parse_profiles(&registry_value().to_string()).unwrap()
}
