use semantic_agent_hook::{ProfileRegistry, parse_profiles};
use serde_json::json;

mod direct_read;
mod python_priority;
mod raw_search;
mod search_json;
mod wrappers;

fn registry_with_python() -> ProfileRegistry {
    let mut value = super::registry_value();
    value["profiles"].as_array_mut().unwrap().push(json!({
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
    }));
    parse_profiles(&value.to_string()).unwrap()
}

fn registry_with_rust_and_python() -> ProfileRegistry {
    let mut value = super::registry_value();
    value["profiles"].as_array_mut().unwrap().push(json!({
        "languageId": "rust",
        "providerId": "rs-harness",
        "binary": "rs-harness",
        "namespace": "agent.semantic-protocols.languages.rust.rs-harness",
        "sourceExtensions": [".rs"],
        "configFiles": ["Cargo.toml", "Cargo.lock"],
        "sourceRoots": ["src", "tests", "crates"],
        "ignoredPathPrefixes": ["target"],
        "commands": {
            "prime": {"argv": ["rs-harness", "search", "prime", "."]},
            "owner": {"argv": ["rs-harness", "search", "owner", "{path}", "."]},
            "text": {"argv": ["rs-harness", "search", "text", "{query}", "owner", "tests", "--view", "seeds", "."]},
            "ingest": {"argv": ["rs-harness", "search", "ingest", "."], "stdinMode": "pipe-candidates"},
            "checkChanged": {"argv": ["rs-harness", "check", "--changed", "."]},
            "guide": {"argv": ["rs-harness", "agent", "guide", "."]}
        }
    }));
    value["profiles"].as_array_mut().unwrap().push(json!({
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
    }));
    parse_profiles(&value.to_string()).unwrap()
}
