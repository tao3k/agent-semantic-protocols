use agent_semantic_hook::{ActivatedProvider, CommandTemplate, HookRoutes, HookRuntime, StdinMode};

mod activation_contract;
mod platform;
mod routes;

pub(crate) fn command(argv: &[&str]) -> CommandTemplate {
    CommandTemplate {
        argv: argv.iter().map(|arg| (*arg).to_string()).collect(),
        stdin_mode: None,
    }
}

pub(crate) fn command_with_stdin(argv: &[&str], stdin_mode: StdinMode) -> CommandTemplate {
    CommandTemplate {
        argv: argv.iter().map(|arg| (*arg).to_string()).collect(),
        stdin_mode: Some(stdin_mode),
    }
}

pub(crate) fn registry() -> HookRuntime {
    HookRuntime {
        rankers: Vec::new(),
        project_root: ".".to_string(),
        providers: vec![typescript_provider()],
    }
}

pub(crate) fn registry_without_providers() -> HookRuntime {
    HookRuntime {
        rankers: Vec::new(),
        project_root: ".".to_string(),
        providers: Vec::new(),
    }
}

pub(crate) fn rust_registry() -> HookRuntime {
    HookRuntime {
        rankers: Vec::new(),
        project_root: ".".to_string(),
        providers: vec![rust_provider()],
    }
}

pub(super) fn builtin_provider_manifest(language_id: &str, provider_id: &str) -> ProviderManifest {
    builtin_provider_manifests()
        .into_iter()
        .find(|manifest| {
            manifest.language_id().as_str() == language_id
                && manifest.provider_id().as_str() == provider_id
        })
        .unwrap_or_else(|| {
            panic!(
                "missing canonical provider manifest language={language_id} provider={provider_id}"
            )
        })
}

pub(super) fn typescript_provider() -> ActivatedProvider {
    let manifest = builtin_provider_manifest("typescript", "ts-harness");
    let routes =
        agent_semantic_hook::materialize_provider_routes(&manifest).expect("TypeScript routes");
    provider(
        &manifest,
        ProviderFixtureLayout {
            source_extensions: &[".ts", ".tsx", ".js", ".jsx", ".mts", ".cts", ".mjs", ".cjs"],
            config_files: &["package.json", "tsconfig.json"],
            source_roots: &["src", "tests"],
            ignored_path_prefixes: &["node_modules", "dist"],
        },
        routes,
    )
}

fn rust_provider() -> ActivatedProvider {
    let manifest = builtin_provider_manifest("rust", "rs-harness");
    let routes = agent_semantic_hook::materialize_provider_routes(&manifest).expect("Rust routes");
    provider(
        &manifest,
        ProviderFixtureLayout {
            source_extensions: &[".rs"],
            config_files: &["Cargo.toml"],
            source_roots: &["src", "tests", "benches", "examples"],
            ignored_path_prefixes: &["target"],
        },
        routes,
    )
}

pub(super) struct ProviderFixtureLayout<'a> {
    pub(super) source_extensions: &'a [&'a str],
    pub(super) config_files: &'a [&'a str],
    pub(super) source_roots: &'a [&'a str],
    pub(super) ignored_path_prefixes: &'a [&'a str],
}

pub(super) fn provider(
    manifest: &ProviderManifest,
    layout: ProviderFixtureLayout<'_>,
    routes: HookRoutes,
) -> ActivatedProvider {
    ActivatedProvider {
        manifest_id: manifest.manifest_id().to_string(),
        manifest_digest: provider_manifest_digest(manifest)
            .expect("digest canonical provider manifest"),
        language_id: manifest.language_id().clone(),
        provider_id: manifest.provider_id().clone(),
        binary: manifest.binary().to_string(),
        execution: manifest.execution(),
        provider_command_prefix: Vec::new(),
        execution_command_digest: "test-execution-command-digest".to_string(),
        namespace: manifest.namespace().to_string(),
        package_roots: vec![".".to_string()],
        source_extensions: layout
            .source_extensions
            .iter()
            .map(|extension| (*extension).to_string())
            .collect(),
        config_files: layout
            .config_files
            .iter()
            .map(|config| (*config).to_string())
            .collect(),
        source_roots: layout
            .source_roots
            .iter()
            .map(|root| (*root).to_string())
            .collect(),
        ignored_path_prefixes: layout
            .ignored_path_prefixes
            .iter()
            .map(|prefix| (*prefix).to_string())
            .collect(),
        search_capabilities: manifest.search_capabilities().clone(),
        semantic_facts_descriptor: manifest.semantic_facts_descriptor().cloned(),
        query_pack_descriptor: manifest.query_pack_descriptor().clone(),
        semantic_registry_digest: agent_semantic_hook::semantic_registry_digest(),
        policy: manifest.policy().clone(),
        routes,
    }
}

pub(super) fn provider_routes(binary: &str, query: Option<CommandTemplate>) -> HookRoutes {
    HookRoutes {
        prime: command(&[binary, "search", "prime", "."]),
        owner: command(&[
            binary, "search", "owner", "{path}", "items", "--query", "{query}", ".",
        ]),
        lexical: command(&[
            binary, "search", "lexical", "{query}", "owner", "tests", "--view", "seeds", ".",
        ]),
        query,
        exact_selector_native: None,
        ingest: command_with_stdin(
            &[
                binary, "search", "ingest", "owner", "tests", "--view", "seeds", ".",
            ],
            StdinMode::PipeCandidates,
        ),
        check_changed: command(&[binary, "check", "--changed", "."]),
        workspace_scope: None,
        dependency_topology: None,
        dependency_topology_metadata: None,
        export_index: None,
        guide: Some(command(&[binary, "agent", "guide", "."])),
    }
}
use agent_semantic_hook::{ProviderManifest, builtin_provider_manifests, provider_manifest_digest};
