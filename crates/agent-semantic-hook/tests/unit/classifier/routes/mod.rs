use agent_semantic_hook::{ActivatedProvider, HookRuntime, StdinMode};

use super::{command, command_with_stdin, provider, provider_routes, typescript_provider};

mod codex_command_actions;
mod python_priority;
mod search_json;
mod source_access_rules;
mod wrappers;

fn registry_with_python() -> HookRuntime {
    HookRuntime {
        project_root: ".".to_string(),
        providers: vec![typescript_provider(), python_provider()],
    }
}

fn document_provider(language_id: &str, extension: &str) -> ActivatedProvider {
    let manifest = builtin_provider_manifest(language_id, "orgize");
    let routes =
        agent_semantic_hook::materialize_provider_routes(&manifest).expect("document routes");
    provider(&manifest, &[extension], &[], &[], &[], routes)
}

fn registry_with_documents() -> HookRuntime {
    HookRuntime {
        project_root: ".".to_string(),
        providers: vec![
            document_provider("org", ".org"),
            document_provider("md", ".md"),
        ],
    }
}

fn registry_with_rust_and_python() -> HookRuntime {
    HookRuntime {
        project_root: ".".to_string(),
        providers: vec![typescript_provider(), rust_provider(), python_provider()],
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
            "--workspace",
            ".",
            "--view",
            "seeds",
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
        &builtin_provider_manifest("rust", "rs-harness"),
        &[".rs"],
        &["Cargo.toml", "Cargo.lock"],
        &["src", "tests", "crates"],
        &["target"],
        routes,
    )
}

fn python_provider() -> ActivatedProvider {
    let manifest = builtin_provider_manifest("python", "py-harness");
    let routes =
        agent_semantic_hook::materialize_provider_routes(&manifest).expect("python routes");
    provider(
        &manifest,
        &[".py", ".pyi"],
        &["pyproject.toml"],
        &["src", "tests"],
        &[".venv", "__pycache__"],
        routes,
    )
}
use super::builtin_provider_manifest;
