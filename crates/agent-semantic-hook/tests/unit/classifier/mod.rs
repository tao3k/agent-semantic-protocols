use agent_semantic_hook::{ActivatedProvider, CommandTemplate, HookRoutes, HookRuntime, StdinMode};

mod activation_contract;
mod platform;
mod routes;
mod search_flow_feedback;

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
        project_root: ".".to_string(),
        providers: vec![typescript_provider()],
    }
}

pub(super) fn builtin_provider_manifest(language_id: &str, provider_id: &str) -> ProviderManifest {
    builtin_provider_manifests()
        .into_iter()
        .find(|manifest| manifest.language_id == language_id && manifest.provider_id == provider_id)
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
        &[".ts", ".tsx", ".js", ".jsx", ".mts", ".cts", ".mjs", ".cjs"],
        &["package.json", "tsconfig.json"],
        &["src", "tests"],
        &["node_modules", "dist"],
        routes,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn provider(
    manifest: &ProviderManifest,
    source_extensions: &[&str],
    config_files: &[&str],
    source_roots: &[&str],
    ignored_path_prefixes: &[&str],
    routes: HookRoutes,
) -> ActivatedProvider {
    ActivatedProvider {
        manifest_id: manifest.manifest_id.clone(),
        manifest_digest: provider_manifest_digest(manifest)
            .expect("digest canonical provider manifest"),
        language_id: manifest.language_id.clone(),
        provider_id: manifest.provider_id.clone(),
        binary: manifest.binary.clone(),
        execution: manifest.execution,
        provider_command_prefix: Vec::new(),
        namespace: manifest.namespace.clone(),
        package_roots: vec![".".to_string()],
        source_extensions: source_extensions
            .iter()
            .map(|extension| (*extension).to_string())
            .collect(),
        config_files: config_files
            .iter()
            .map(|config| (*config).to_string())
            .collect(),
        source_roots: source_roots
            .iter()
            .map(|root| (*root).to_string())
            .collect(),
        ignored_path_prefixes: ignored_path_prefixes
            .iter()
            .map(|prefix| (*prefix).to_string())
            .collect(),
        search_capabilities: manifest.search_capabilities.clone(),
        semantic_facts_descriptor: manifest.semantic_facts_descriptor.clone(),
        query_pack_descriptor: manifest.query_pack_descriptor.clone(),
        semantic_registry_digest: agent_semantic_hook::semantic_registry_digest(),
        policy: manifest.policy.clone(),
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
