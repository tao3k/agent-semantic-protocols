//! Registry-style language registrations used to derive hook provider manifests.

use serde::Deserialize;

use crate::protocol::{
    CommandTemplate, HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, HookPolicy, HookRoutes,
    PROVIDER_MANIFEST_SCHEMA_ID, PROVIDER_MANIFEST_SCHEMA_VERSION, StdinMode,
};
use crate::protocol_activation::{ManifestSourceDefaults, ProviderManifest};

const SCHEMA_REGISTRY_JSON: &str =
    include_str!("../../../schemas/semantic-language-registry.providers.v1.json");

pub(crate) fn schema_registry_provider_manifests() -> Vec<ProviderManifest> {
    schema_registry()
        .languages
        .into_iter()
        .map(|language| {
            let overlay = hook_overlay_for(&language.language_id).unwrap_or_else(|| {
                panic!(
                    "missing hook provider overlay for registry language `{}`",
                    language.language_id
                )
            });
            language.into_manifest(overlay)
        })
        .collect()
}

fn schema_registry() -> SemanticLanguageRegistry {
    serde_json::from_str(SCHEMA_REGISTRY_JSON)
        .expect("embedded semantic language registry must be valid JSON")
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SemanticLanguageRegistry {
    languages: Vec<LanguageRegistration>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LanguageRegistration {
    language_id: String,
    provider_id: String,
    binary: String,
    namespace: String,
}

impl LanguageRegistration {
    fn into_manifest(self, overlay: HookProviderOverlay) -> ProviderManifest {
        ProviderManifest {
            schema_id: PROVIDER_MANIFEST_SCHEMA_ID.to_string(),
            schema_version: PROVIDER_MANIFEST_SCHEMA_VERSION.to_string(),
            protocol_id: HOOK_PROTOCOL_ID.to_string(),
            protocol_version: HOOK_PROTOCOL_VERSION.to_string(),
            manifest_id: format!(
                "agent.semantic-protocols.providers.{}.{}",
                self.language_id, self.provider_id
            ),
            manifest_version: env!("CARGO_PKG_VERSION").to_string(),
            language_id: self.language_id.clone(),
            provider_id: self.provider_id,
            namespace: self.namespace,
            binary: self.binary.clone(),
            source: overlay.source.into_defaults(),
            policy: HookPolicy::default(),
            routes: overlay
                .route_profile
                .routes(&self.language_id, &self.binary),
        }
    }
}

struct HookProviderOverlay {
    source: SourceRegistration,
    route_profile: RouteProfile,
}

fn hook_overlay_for(language_id: &str) -> Option<HookProviderOverlay> {
    let overlay = match language_id {
        "rust" => HookProviderOverlay {
            source: SourceRegistration::new(
                &[".rs"],
                &["Cargo.toml"],
                &["src", "crates", "tests"],
                &["target"],
            ),
            route_profile: RouteProfile::Provider,
        },
        "typescript" => HookProviderOverlay {
            source: SourceRegistration::new(
                &[".ts", ".tsx", ".js", ".jsx", ".mts", ".cts", ".mjs", ".cjs"],
                &["package.json", "tsconfig.json", "pnpm-workspace.yaml"],
                &["src", "tests", "app", "packages"],
                &["node_modules", "dist", "build", ".next"],
            ),
            route_profile: RouteProfile::Provider,
        },
        "python" => HookProviderOverlay {
            source: SourceRegistration::new(
                &[".py"],
                &["pyproject.toml", "setup.py", "setup.cfg"],
                &["src", "tests"],
                &[".venv", "venv", "__pycache__", ".mypy_cache"],
            ),
            route_profile: RouteProfile::Provider,
        },
        "julia" => HookProviderOverlay {
            source: SourceRegistration::new(
                &[".jl"],
                &["Project.toml"],
                &["src", "test", "docs", "examples", "benchmark"],
                &[".devenv", ".git", "build", "Manifest.toml"],
            ),
            route_profile: RouteProfile::Julia,
        },
        "org" => HookProviderOverlay {
            source: SourceRegistration::new(&[".org", ".org_archive"], &[], &["docs"], &["target"]),
            route_profile: RouteProfile::Document,
        },
        "md" => HookProviderOverlay {
            source: SourceRegistration::new(&[".md", ".markdown"], &[], &["docs"], &["target"]),
            route_profile: RouteProfile::Document,
        },
        _ => return None,
    };
    Some(overlay)
}

struct SourceRegistration {
    extensions: &'static [&'static str],
    config_files: &'static [&'static str],
    source_roots: &'static [&'static str],
    ignored_path_prefixes: &'static [&'static str],
}

impl SourceRegistration {
    fn new(
        extensions: &'static [&'static str],
        config_files: &'static [&'static str],
        source_roots: &'static [&'static str],
        ignored_path_prefixes: &'static [&'static str],
    ) -> Self {
        Self {
            extensions,
            config_files,
            source_roots,
            ignored_path_prefixes,
        }
    }

    fn into_defaults(self) -> ManifestSourceDefaults {
        ManifestSourceDefaults {
            default_extensions: strings(self.extensions),
            default_config_files: strings(self.config_files),
            default_source_roots: strings(self.source_roots),
            default_ignored_path_prefixes: strings(self.ignored_path_prefixes),
        }
    }
}

enum RouteProfile {
    Provider,
    Julia,
    Document,
}

impl RouteProfile {
    fn routes(&self, language_id: &str, binary: &str) -> HookRoutes {
        match self {
            Self::Provider => provider_routes(binary),
            Self::Julia => julia_routes(binary),
            Self::Document => document_routes(language_id),
        }
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
        guide: Some(command_template(&[binary, "guide", "{projectRoot}"])),
    }
}

fn julia_routes(binary: &str) -> HookRoutes {
    HookRoutes {
        query: Some(command_template(&[
            binary,
            "search",
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
                "owner",
                "tests",
                "--view",
                "seeds",
                "{projectRoot}",
            ],
            StdinMode::PipeCandidates,
        ),
        ..provider_routes(binary)
    }
}

fn document_routes(language_id: &str) -> HookRoutes {
    let facade = ["asp", language_id];
    HookRoutes {
        prime: command_template(&[
            facade[0],
            facade[1],
            "search",
            "prime",
            "--view",
            "seeds",
            "{projectRoot}",
        ]),
        owner: command_template(&[
            facade[0],
            facade[1],
            "query",
            "--selector",
            "{path}",
            "--view",
            "metadata",
            "{projectRoot}",
        ]),
        fzf: command_template(&[
            facade[0],
            facade[1],
            "query",
            "--term",
            "{query}",
            "--view",
            "metadata",
            "{projectRoot}",
        ]),
        query: Some(command_template(&[
            facade[0],
            facade[1],
            "query",
            "{termArgs}",
            "--view",
            "metadata",
            "{projectRoot}",
        ])),
        ingest: command_template_with_stdin(
            &[
                facade[0],
                facade[1],
                "search",
                "prime",
                "--view",
                "seeds",
                "{projectRoot}",
            ],
            StdinMode::PipeCandidates,
        ),
        check_changed: command_template(&[
            facade[0],
            facade[1],
            "search",
            "prime",
            "--view",
            "seeds",
            "{projectRoot}",
        ]),
        guide: Some(command_template(&[
            facade[0],
            facade[1],
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
