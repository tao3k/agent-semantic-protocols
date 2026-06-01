use semantic_agent_hook::{
    HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, PROFILE_REGISTRY_SCHEMA_ID,
    PROFILE_REGISTRY_SCHEMA_VERSION, ProfileRegistry, parse_profiles,
};
use serde_json::{Value, json};

mod platform;
mod profile_registry;
mod raw_search_policy;
mod routes;

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
                "prime": {"argv": ["ts-harness", "search", "prime", "."]},
                    "owner": {"argv": ["ts-harness", "search", "owner", "{path}", "items", "--query", "{query}", "."]},
                "text": {"argv": ["ts-harness", "search", "text", "{query}", "owner", "tests", "--view", "seeds", "."]},
                "ingest": {"argv": ["ts-harness", "search", "ingest", "owner", "tests", "--view", "seeds", "."], "stdinMode": "pipe-candidates"},
                "checkChanged": {"argv": ["ts-harness", "check", "--changed", "."]},
                "guide": {"argv": ["ts-harness", "agent", "guide", "."]}
            }
        }]
    })
}

pub(super) fn registry() -> ProfileRegistry {
    parse_profiles(&registry_value().to_string()).unwrap()
}
