//! Registry-style language registrations used to derive hook provider manifests.

use serde::Deserialize;

use crate::protocol::{
    CommandTemplate, HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, HookRoutes,
    PROVIDER_MANIFEST_SCHEMA_ID, PROVIDER_MANIFEST_SCHEMA_VERSION,
};
use crate::protocol_activation::{ManifestSourceDefaults, ProviderManifest};

const SCHEMA_REGISTRY_JSON: &str =
    include_str!("../../../schemas/semantic-language-registry.providers.v1.json");

const LANGUAGE_PROVIDER_MANIFEST_JSON: &[&str] = &[
    include_str!(
        "../../../languages/rust-lang-project-harness/provider/asp-provider-manifest.json"
    ),
    include_str!(
        "../../../languages/typescript-lang-project-harness/provider/asp-provider-manifest.json"
    ),
    include_str!(
        "../../../languages/python-lang-project-harness/provider/asp-provider-manifest.json"
    ),
    include_str!(
        "../../../languages/gerbil-scheme-language-project-harness/provider/asp-provider-manifest.json"
    ),
    include_str!("../../../languages/JuliaLangProjectHarness.jl/juliac/asp-provider-manifest.json"),
    include_str!("../../../languages/org/provider/asp-org-provider-manifest.json"),
    include_str!("../../../languages/org/provider/asp-md-provider-manifest.json"),
];

const COMMON_IGNORED_PATH_PREFIXES: &[&str] = &[
    ".cache",
    ".codex/harness-state",
    ".codex/rs-harness",
    ".data",
    ".devenv",
    ".direnv",
    ".git",
    ".idea",
    ".jj",
    ".run",
    ".vscode",
    "node_modules",
    "target",
];

pub(crate) fn schema_registry_provider_manifests() -> Vec<ProviderManifest> {
    let language_manifests = language_provider_manifests();
    schema_registry()
        .languages
        .into_iter()
        .map(|language| {
            language_manifests
                .iter()
                .find(|manifest| {
                    manifest.language_id == language.language_id
                        && manifest.provider_id == language.provider_id
                })
                .cloned()
                .unwrap_or_else(|| {
                    panic!(
                        "missing language provider manifest for registry language `{}` provider `{}`",
                        language.language_id, language.provider_id
                    )
                })
        })
        .collect()
}

fn language_provider_manifests() -> Vec<ProviderManifest> {
    LANGUAGE_PROVIDER_MANIFEST_JSON
        .iter()
        .map(|json| {
            let mut manifest = serde_json::from_str::<ProviderManifest>(json)
                .expect("embedded language provider manifest must be valid JSON");
            normalize_language_provider_manifest(&mut manifest);
            manifest
        })
        .collect()
}

/// Return registered ASP language ids from the embedded provider manifests.
pub fn registered_language_ids() -> Vec<String> {
    let mut language_ids = language_provider_manifests()
        .into_iter()
        .map(|manifest| manifest.language_id)
        .collect::<Vec<_>>();
    language_ids.sort();
    language_ids.dedup();
    language_ids
}

fn normalize_language_provider_manifest(manifest: &mut ProviderManifest) {
    manifest.schema_id = PROVIDER_MANIFEST_SCHEMA_ID.to_string();
    manifest.schema_version = PROVIDER_MANIFEST_SCHEMA_VERSION.to_string();
    manifest.protocol_id = HOOK_PROTOCOL_ID.to_string();
    manifest.protocol_version = HOOK_PROTOCOL_VERSION.to_string();
    manifest.manifest_version = env!("CARGO_PKG_VERSION").to_string();
    normalize_source_defaults(&mut manifest.source);
    normalize_hook_routes(&mut manifest.routes);
}

fn normalize_source_defaults(source: &mut ManifestSourceDefaults) {
    for prefix in COMMON_IGNORED_PATH_PREFIXES {
        if !source
            .default_ignored_path_prefixes
            .iter()
            .any(|seen| seen == prefix)
        {
            source
                .default_ignored_path_prefixes
                .push(prefix.to_string());
        }
    }
}

fn normalize_hook_routes(routes: &mut HookRoutes) {
    normalize_command_template(&mut routes.prime);
    normalize_command_template(&mut routes.owner);
    normalize_command_template(&mut routes.lexical);
    if let Some(query) = routes.query.as_mut() {
        normalize_command_template(query);
    }
    normalize_command_template(&mut routes.ingest);
    normalize_command_template(&mut routes.check_changed);
    if let Some(export_index) = routes.export_index.as_mut() {
        normalize_command_template(export_index);
    }
    if let Some(guide) = routes.guide.as_mut() {
        normalize_command_template(guide);
    }
}

fn normalize_command_template(template: &mut CommandTemplate) {
    for argument in &mut template.argv {
        if argument == "." {
            *argument = "{projectRoot}".to_string();
        }
    }
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
}
