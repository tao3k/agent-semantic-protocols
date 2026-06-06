use agent_semantic_hook::{
    ActivatedProvider, CommandTemplate, HookPolicy, HookRoutes, HookRuntime, ProviderExecution,
    StdinMode,
};

mod activation_contract;
mod platform;
mod raw_search_policy;
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
        project_root: ".".to_string(),
        providers: vec![typescript_provider()],
    }
}

pub(super) fn typescript_provider() -> ActivatedProvider {
    provider(
        "typescript",
        "ts-harness",
        "ts-harness",
        "agent.semantic-protocols.languages.typescript.ts-harness",
        &[".ts", ".tsx", ".js", ".jsx", ".mts", ".cts", ".mjs", ".cjs"],
        &["package.json", "tsconfig.json"],
        &["src", "tests"],
        &["node_modules", "dist"],
        provider_routes(
            "ts-harness",
            Some(command(&[
                "asp",
                "typescript",
                "query",
                "--from-hook",
                "direct-source-read",
                "--selector",
                "{selector}",
                "{termArgs}",
                "--surface",
                "owners,tests",
                "--view",
                "seeds",
                ".",
            ])),
        ),
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn provider(
    language_id: &str,
    provider_id: &str,
    binary: &str,
    namespace: &str,
    source_extensions: &[&str],
    config_files: &[&str],
    source_roots: &[&str],
    ignored_path_prefixes: &[&str],
    routes: HookRoutes,
) -> ActivatedProvider {
    ActivatedProvider {
        manifest_id: namespace.to_string(),
        manifest_digest: "sha256:test".to_string(),
        language_id: language_id.to_string(),
        provider_id: provider_id.to_string(),
        binary: binary.to_string(),
        execution: ProviderExecution::ExternalProcess,
        provider_command_prefix: Vec::new(),
        namespace: namespace.to_string(),
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
        policy: HookPolicy::default(),
        routes,
    }
}

pub(super) fn provider_routes(binary: &str, query: Option<CommandTemplate>) -> HookRoutes {
    HookRoutes {
        prime: command(&[binary, "search", "prime", "."]),
        owner: command(&[
            binary, "search", "owner", "{path}", "items", "--query", "{query}", ".",
        ]),
        fzf: command(&[
            binary, "search", "fzf", "{query}", "owner", "tests", "--view", "seeds", ".",
        ]),
        query,
        ingest: command_with_stdin(
            &[
                binary, "search", "ingest", "owner", "tests", "--view", "seeds", ".",
            ],
            StdinMode::PipeCandidates,
        ),
        check_changed: command(&[binary, "check", "--changed", "."]),
        guide: Some(command(&[binary, "agent", "guide", "."])),
    }
}
