use agent_semantic_hook::{ActivatedProvider, HookRuntime, StdinMode};

use super::{
    ProviderFixtureLayout, command, command_with_stdin, provider, provider_routes,
    typescript_provider,
};

mod codex_command_actions;
mod python_priority;
mod search_json;
mod source_access_rules;
mod wrappers;

fn registry_with_python() -> HookRuntime {
    HookRuntime {
        rankers: Vec::new(),
        project_root: ".".to_string(),
        providers: vec![typescript_provider(), python_provider()],
        policy_providers: Vec::new(),
    }
}

fn document_provider(language_id: &str, extension: &str) -> ActivatedProvider {
    let provider_id = agent_semantic_hook::registered_provider_id_v1(language_id)
        .expect("registered document provider identity");
    let manifest = builtin_provider_manifest(language_id, provider_id.as_str());
    let routes =
        agent_semantic_hook::materialize_provider_routes(&manifest).expect("document routes");
    provider(
        &manifest,
        ProviderFixtureLayout {
            source_extensions: &[extension],
            config_files: &[],
        },
        routes,
    )
}

fn registry_with_documents() -> HookRuntime {
    HookRuntime {
        rankers: Vec::new(),
        project_root: ".".to_string(),
        providers: vec![
            document_provider("org", ".org"),
            document_provider("md", ".md"),
        ],
        policy_providers: Vec::new(),
    }
}

fn registry_with_rust_and_python() -> HookRuntime {
    HookRuntime {
        rankers: Vec::new(),
        project_root: ".".to_string(),
        providers: vec![typescript_provider(), rust_provider(), python_provider()],
        policy_providers: Vec::new(),
    }
}

fn rust_provider() -> ActivatedProvider {
    let mut routes = provider_routes(
        "asp-rust",
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
        &builtin_provider_manifest("rust", "asp-rust"),
        ProviderFixtureLayout {
            source_extensions: &[".rs"],
            config_files: &["Cargo.toml", "Cargo.lock"],
        },
        routes,
    )
}

fn python_provider() -> ActivatedProvider {
    let manifest = builtin_provider_manifest("python", "asp-python");
    let routes =
        agent_semantic_hook::materialize_provider_routes(&manifest).expect("python routes");
    provider(
        &manifest,
        ProviderFixtureLayout {
            source_extensions: &[".py", ".pyi"],
            config_files: &["pyproject.toml"],
        },
        routes,
    )
}
use super::builtin_provider_manifest;
