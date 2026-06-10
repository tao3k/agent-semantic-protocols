use agent_semantic_hook::{ActivatedProvider, HookRuntime, StdinMode};

use super::{command, command_with_stdin, provider, provider_routes, typescript_provider};

mod codex_command_actions;
mod direct_read;
mod python_priority;
mod raw_search;
mod search_json;
mod wrappers;

fn registry_with_python() -> HookRuntime {
    HookRuntime {
        project_root: ".".to_string(),
        providers: vec![typescript_provider(), python_provider()],
    }
}

fn registry_with_rust_and_python() -> HookRuntime {
    HookRuntime {
        project_root: ".".to_string(),
        providers: vec![typescript_provider(), rust_provider(), python_provider()],
    }
}

fn registry_with_prefixed_python() -> HookRuntime {
    let mut python = provider(
        "python",
        "python-project-harness",
        "python-project-harness",
        "agent.semantic-protocols.languages.python.python-project-harness",
        &[".py"],
        &["Project.toml", "Manifest.toml"],
        &["src", "test"],
        &[".git", ".python"],
        provider_routes("python-project-harness", None),
    );
    python.provider_command_prefix = vec![
        "python".to_string(),
        "-m".to_string(),
        "tools.fake_provider".to_string(),
    ];
    HookRuntime {
        project_root: ".".to_string(),
        providers: vec![typescript_provider(), python],
    }
}

fn rust_provider() -> ActivatedProvider {
    let mut routes = provider_routes(
        "rs-harness",
        Some(command(&[
            "asp",
            "rust",
            "query",
            "--selector",
            "{selector}",
            "{termArgs}",
            "--surface",
            "owners,tests",
            "--view",
            "seeds",
            ".",
        ])),
    );
    routes.owner = command(&[
        "asp", "rust", "search", "owner", "{path}", "items", "--view", "seeds", ".",
    ]);
    routes.ingest = command_with_stdin(
        &[
            "asp", "rust", "search", "ingest", "items", "tests", "--view", "seeds", ".",
        ],
        StdinMode::PipeCandidates,
    );
    provider(
        "rust",
        "rs-harness",
        "rs-harness",
        "agent.semantic-protocols.languages.rust.rs-harness",
        &[".rs"],
        &["Cargo.toml", "Cargo.lock"],
        &["src", "tests", "crates"],
        &["target"],
        routes,
    )
}

fn python_provider() -> ActivatedProvider {
    let mut routes = provider_routes(
        "py-harness",
        Some(command(&[
            "asp",
            "python",
            "query",
            "--selector",
            "{selector}",
            "{termArgs}",
            "--surface",
            "owners,tests",
            "--view",
            "seeds",
            ".",
        ])),
    );
    routes.owner = command(&["py-harness", "search", "owner", "{path}", "."]);
    routes.ingest = command_with_stdin(
        &["py-harness", "search", "ingest", "."],
        StdinMode::PipeCandidates,
    );
    provider(
        "python",
        "py-harness",
        "py-harness",
        "agent.semantic-protocols.languages.python.py-harness",
        &[".py", ".pyi"],
        &["pyproject.toml"],
        &["src", "tests"],
        &[".venv", "__pycache__"],
        routes,
    )
}
