//! Built-in provider manifests and default project activations.

use std::env;
use std::fs;
use std::path::Path;

use crate::protocol::{
    CommandTemplate, HOOK_ACTIVATION_SCHEMA_ID, HOOK_ACTIVATION_SCHEMA_VERSION, HOOK_PROTOCOL_ID,
    HOOK_PROTOCOL_VERSION, HookPolicy, HookRoutes, PROVIDER_MANIFEST_SCHEMA_ID,
    PROVIDER_MANIFEST_SCHEMA_VERSION, StdinMode,
};
use crate::protocol_activation::{
    ActivatedProviderConfig, ActivationCoverage, ActivationGeneratedBy, HookActivation,
    ManifestSourceDefaults, ProviderManifest, provider_manifest_digest,
};

/// Returns the built-in language provider manifests known to this hook runtime.
pub fn builtin_provider_manifests() -> Vec<ProviderManifest> {
    provider_manifests()
}

pub(crate) fn provider_manifests() -> Vec<ProviderManifest> {
    vec![rust_manifest(), typescript_manifest(), python_manifest()]
}

pub(crate) fn build_default_activation(project_root: &Path) -> Result<HookActivation, String> {
    let mut providers = Vec::new();
    for manifest in provider_manifests() {
        if !provider_binary_available(&manifest.binary) {
            continue;
        }
        providers.push(activate_provider(project_root, &manifest)?);
    }
    if providers.is_empty() {
        return Err(
            "expected PATH to contain at least one executable semantic provider binary".to_string(),
        );
    }
    Ok(HookActivation {
        schema_id: HOOK_ACTIVATION_SCHEMA_ID.to_string(),
        schema_version: HOOK_ACTIVATION_SCHEMA_VERSION.to_string(),
        protocol_id: HOOK_PROTOCOL_ID.to_string(),
        protocol_version: HOOK_PROTOCOL_VERSION.to_string(),
        project_root: project_root.display().to_string(),
        generated_by: ActivationGeneratedBy {
            runtime: "semantic-agent-hook".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        },
        generated_at: None,
        providers,
    })
}

pub(crate) fn provider_binary_available(binary: &str) -> bool {
    env::var_os("PATH")
        .and_then(|paths| {
            env::split_paths(&paths)
                .map(|path| path.join(binary))
                .find(|candidate| is_executable_file(candidate))
        })
        .is_some()
}

fn activate_provider(
    project_root: &Path,
    manifest: &ProviderManifest,
) -> Result<ActivatedProviderConfig, String> {
    let provider_command_prefix = Vec::new();
    Ok(ActivatedProviderConfig {
        manifest_id: manifest.manifest_id.clone(),
        manifest_digest: provider_manifest_digest(manifest)
            .map_err(|error| format!("failed to digest provider manifest: {error:?}"))?,
        language_id: manifest.language_id.clone(),
        provider_id: manifest.provider_id.clone(),
        binary: manifest.binary.clone(),
        provider_command_prefix,
        coverage: ActivationCoverage {
            package_roots: discover_package_roots(project_root, manifest),
            source_roots: manifest.source.default_source_roots.clone(),
            config_files: manifest.source.default_config_files.clone(),
            source_extensions: manifest.source.default_extensions.clone(),
            ignored_path_prefixes: manifest.source.default_ignored_path_prefixes.clone(),
        },
    })
}

fn discover_package_roots(project_root: &Path, manifest: &ProviderManifest) -> Vec<String> {
    let mut roots = Vec::new();
    collect_package_roots(project_root, project_root, manifest, 0, &mut roots);
    if roots.is_empty() {
        roots.push(".".to_string());
    }
    roots.sort_by(|left, right| {
        left.matches('/')
            .count()
            .cmp(&right.matches('/').count())
            .then(left.cmp(right))
    });
    roots.dedup();
    roots
}

fn collect_package_roots(
    project_root: &Path,
    current: &Path,
    manifest: &ProviderManifest,
    depth: usize,
    roots: &mut Vec<String>,
) {
    if depth > 5 {
        return;
    }
    if package_root_matches(current, manifest) {
        roots.push(relative_package_root(project_root, current));
    }
    let Ok(entries) = fs::read_dir(current) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() || should_skip_package_root_dir(&path, manifest) {
            continue;
        }
        collect_package_roots(project_root, &path, manifest, depth + 1, roots);
    }
}

fn package_root_matches(candidate: &Path, manifest: &ProviderManifest) -> bool {
    manifest
        .source
        .default_config_files
        .iter()
        .any(|config| candidate.join(config).is_file())
        && manifest
            .source
            .default_source_roots
            .iter()
            .any(|root| candidate.join(root).is_dir())
}

fn relative_package_root(project_root: &Path, package_root: &Path) -> String {
    package_root
        .strip_prefix(project_root)
        .ok()
        .filter(|relative| !relative.as_os_str().is_empty())
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|| ".".to_string())
}

fn should_skip_package_root_dir(path: &Path, manifest: &ProviderManifest) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    if name.starts_with('.') {
        return true;
    }
    matches!(name, "target" | "node_modules" | "dist" | "build" | "venv")
        || manifest
            .source
            .default_ignored_path_prefixes
            .iter()
            .any(|ignored| ignored == name)
}

fn is_executable_file(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn manifest(
    manifest_id: &str,
    language_id: &str,
    provider_id: &str,
    binary: &str,
    source: ManifestSourceDefaults,
) -> ProviderManifest {
    ProviderManifest {
        schema_id: PROVIDER_MANIFEST_SCHEMA_ID.to_string(),
        schema_version: PROVIDER_MANIFEST_SCHEMA_VERSION.to_string(),
        protocol_id: HOOK_PROTOCOL_ID.to_string(),
        protocol_version: HOOK_PROTOCOL_VERSION.to_string(),
        manifest_id: manifest_id.to_string(),
        manifest_version: env!("CARGO_PKG_VERSION").to_string(),
        language_id: language_id.to_string(),
        provider_id: provider_id.to_string(),
        namespace: format!("agent.semantic-protocols.languages.{language_id}.{provider_id}"),
        binary: binary.to_string(),
        source,
        policy: HookPolicy::default(),
        routes: provider_routes(binary),
    }
}

fn source_defaults(
    extensions: &[&str],
    config_files: &[&str],
    source_roots: &[&str],
    ignored_path_prefixes: &[&str],
) -> ManifestSourceDefaults {
    ManifestSourceDefaults {
        default_extensions: strings(extensions),
        default_config_files: strings(config_files),
        default_source_roots: strings(source_roots),
        default_ignored_path_prefixes: strings(ignored_path_prefixes),
    }
}

fn provider_routes(binary: &str) -> HookRoutes {
    HookRoutes {
        prime: command_template(&[
            binary,
            "search",
            "prime",
            "--view",
            "seeds",
            "{projectRoot}",
        ]),
        owner: command_template(&[
            binary,
            "search",
            "owner",
            "{path}",
            "--view",
            "seeds",
            "{projectRoot}",
        ]),
        fzf: command_template(&[
            binary,
            "search",
            "fzf",
            "{query}",
            "owner",
            "tests",
            "--view",
            "seeds",
            "{projectRoot}",
        ]),
        query: Some(command_template(&[
            binary,
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
            "{projectRoot}",
        ])),
        ingest: command_template_with_stdin(
            &[
                binary,
                "search",
                "ingest",
                "items",
                "tests",
                "--view",
                "seeds",
                "{projectRoot}",
            ],
            StdinMode::PipeCandidates,
        ),
        check_changed: command_template(&[binary, "check", "--changed", "{projectRoot}"]),
        guide: Some(command_template(&[
            binary,
            "agent",
            "guide",
            "{projectRoot}",
        ])),
    }
}

fn command_template(argv: &[&str]) -> CommandTemplate {
    CommandTemplate {
        argv: strings(argv),
        stdin_mode: None,
    }
}

fn command_template_with_stdin(argv: &[&str], stdin_mode: StdinMode) -> CommandTemplate {
    CommandTemplate {
        argv: strings(argv),
        stdin_mode: Some(stdin_mode),
    }
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

fn rust_manifest() -> ProviderManifest {
    manifest(
        "agent.semantic-protocols.providers.rust.rs-harness",
        "rust",
        "rs-harness",
        "rs-harness",
        source_defaults(
            &[".rs"],
            &["Cargo.toml"],
            &["src", "crates", "tests"],
            &["target"],
        ),
    )
}

fn typescript_manifest() -> ProviderManifest {
    manifest(
        "agent.semantic-protocols.providers.typescript.ts-harness",
        "typescript",
        "ts-harness",
        "ts-harness",
        source_defaults(
            &[".ts", ".tsx", ".js", ".jsx", ".mts", ".cts", ".mjs", ".cjs"],
            &["package.json", "tsconfig.json", "pnpm-workspace.yaml"],
            &["src", "tests", "app", "packages"],
            &["node_modules", "dist", "build", ".next"],
        ),
    )
}

fn python_manifest() -> ProviderManifest {
    manifest(
        "agent.semantic-protocols.providers.python.py-harness",
        "python",
        "py-harness",
        "py-harness",
        source_defaults(
            &[".py"],
            &["pyproject.toml", "setup.py", "setup.cfg"],
            &["src", "tests"],
            &[".venv", "venv", "__pycache__", ".mypy_cache"],
        ),
    )
}
