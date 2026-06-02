use semantic_agent_hook::{ProfileRegistry, parse_profiles};
use serde_json::json;

use super::{command, command_with_stdin};

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
            "prime": command(&["py-harness", "search", "prime", "."]),
            "owner": command(&["py-harness", "search", "owner", "{path}", "."]),
            "fzf": command(&["py-harness", "search", "fzf", "{query}", "owner", "tests", "--view", "seeds", "."]),
            "ingest": command_with_stdin(&["py-harness", "search", "ingest", "."], "pipe-candidates"),
            "checkChanged": command(&["py-harness", "check", "--changed", "."]),
            "guide": command(&["py-harness", "agent", "guide", "."])
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
            "prime": command(&["rs-harness", "search", "prime", "."]),
            "owner": command(&["rs-harness", "search", "owner", "{path}", "."]),
            "fzf": command(&["rs-harness", "search", "fzf", "{query}", "owner", "tests", "--view", "seeds", "."]),
            "ingest": command_with_stdin(&["rs-harness", "search", "ingest", "."], "pipe-candidates"),
            "checkChanged": command(&["rs-harness", "check", "--changed", "."]),
            "guide": command(&["rs-harness", "agent", "guide", "."])
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
            "prime": command(&["py-harness", "search", "prime", "."]),
            "owner": command(&["py-harness", "search", "owner", "{path}", "."]),
            "fzf": command(&["py-harness", "search", "fzf", "{query}", "owner", "tests", "--view", "seeds", "."]),
            "ingest": command_with_stdin(&["py-harness", "search", "ingest", "."], "pipe-candidates"),
            "checkChanged": command(&["py-harness", "check", "--changed", "."]),
            "guide": command(&["py-harness", "agent", "guide", "."])
        }
    }));
    parse_profiles(&value.to_string()).unwrap()
}

fn registry_with_workspace_julia() -> ProfileRegistry {
    let mut value = super::registry_value();
    value["profiles"].as_array_mut().unwrap().push(json!({
        "languageId": "julia",
        "providerId": "julia-project-harness",
        "binary": "julia-project-harness",
        "providerCommandPrefix": [
            "julia",
            "--project=languages/JuliaLangProjectHarness.jl",
            "languages/JuliaLangProjectHarness.jl/bin/julia-project-harness.jl"
        ],
        "namespace": "agent.semantic-protocols.languages.julia.julia-project-harness",
        "sourceExtensions": [".jl"],
        "configFiles": ["Project.toml", "Manifest.toml"],
        "sourceRoots": ["src", "test"],
        "ignoredPathPrefixes": [".git", ".julia"],
        "commands": {
            "prime": command(&["julia", "--project=languages/JuliaLangProjectHarness.jl", "languages/JuliaLangProjectHarness.jl/bin/julia-project-harness.jl", "search", "prime", "--view", "seeds", "."]),
            "owner": command(&["julia", "--project=languages/JuliaLangProjectHarness.jl", "languages/JuliaLangProjectHarness.jl/bin/julia-project-harness.jl", "search", "owner", "{path}", "--view", "seeds", "."]),
            "fzf": command(&["julia", "--project=languages/JuliaLangProjectHarness.jl", "languages/JuliaLangProjectHarness.jl/bin/julia-project-harness.jl", "search", "fzf", "{query}", "owner", "tests", "--view", "seeds", "."]),
            "ingest": command_with_stdin(&["julia", "--project=languages/JuliaLangProjectHarness.jl", "languages/JuliaLangProjectHarness.jl/bin/julia-project-harness.jl", "search", "ingest", "owner", "tests", "--view", "seeds", "."], "pipe-candidates"),
            "checkChanged": command(&["julia", "--project=languages/JuliaLangProjectHarness.jl", "languages/JuliaLangProjectHarness.jl/bin/julia-project-harness.jl", "check", "--changed", "."]),
            "guide": command(&["julia", "--project=languages/JuliaLangProjectHarness.jl", "languages/JuliaLangProjectHarness.jl/bin/julia-project-harness.jl", "agent", "guide", "."])
        }
    }));
    parse_profiles(&value.to_string()).unwrap()
}
