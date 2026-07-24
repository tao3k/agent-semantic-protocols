//! Registry-style language registrations used to derive hook provider manifests.

use serde::Deserialize;

use crate::protocol::{
    HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, PROVIDER_MANIFEST_SCHEMA_ID,
    PROVIDER_MANIFEST_SCHEMA_VERSION,
};
use crate::protocol_activation::protocol_activation_manifest::{
    ManifestSourceDefaults, ProviderManifest,
};

const SCHEMA_REGISTRY_JSON: &str =
    include_str!("../../../schemas/semantic-language-registry.providers.v1.json");

pub fn semantic_registry_digest() -> String {
    let digest = <sha2::Sha256 as sha2::Digest>::digest(SCHEMA_REGISTRY_JSON.as_bytes());
    format!("sha256:{digest:x}")
}

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
            let manifest = language_manifests
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
                });
            assert_eq!(
                language.query_pack_descriptor, manifest.query_pack_descriptor,
                "registry queryPackDescriptor drift for language `{}` provider `{}`",
                language.language_id, language.provider_id
            );
            manifest
        })
        .collect()
}

fn resolve_route_invocation(
    language: &LanguageRegistration,
    method: &str,
) -> Result<crate::protocol::CommandTemplate, String> {
    let mut matches = language
        .method_descriptors
        .iter()
        .filter(|descriptor| descriptor.method == method);
    let descriptor = matches.next().ok_or_else(|| {
        format!(
            "semantic registry has no method descriptor `{method}` for language `{}` provider `{}`",
            language.language_id, language.provider_id
        )
    })?;
    if matches.next().is_some() {
        return Err(format!(
            "semantic registry has duplicate method descriptor `{method}` for language `{}` provider `{}`",
            language.language_id, language.provider_id
        ));
    }
    Ok(descriptor.invocation.clone())
}

pub fn materialize_provider_routes(
    manifest: &ProviderManifest,
) -> Result<crate::protocol::HookRoutes, String> {
    let registry = schema_registry();
    let language = registry
        .languages
        .iter()
        .find(|language| {
            language.language_id == manifest.language_id
                && language.provider_id == manifest.provider_id
        })
        .ok_or_else(|| {
            format!(
                "semantic registry has no language `{}` provider `{}`",
                manifest.language_id, manifest.provider_id
            )
        })?;
    if language.binary != manifest.binary {
        return Err(format!(
            "semantic registry binary `{}` does not match manifest binary `{}` for language `{}` provider `{}`",
            language.binary, manifest.binary, manifest.language_id, manifest.provider_id
        ));
    }

    let bindings = &manifest.route_bindings;
    let optional = |method: &Option<String>| {
        method
            .as_deref()
            .map(|method| resolve_route_invocation(language, method))
            .transpose()
    };
    Ok(crate::protocol::HookRoutes {
        prime: resolve_route_invocation(language, &bindings.prime)?,
        owner: resolve_route_invocation(language, &bindings.owner)?,
        lexical: resolve_route_invocation(language, &bindings.lexical)?,
        query: optional(&bindings.query)?,
        ingest: resolve_route_invocation(language, &bindings.ingest)?,
        check_changed: resolve_route_invocation(language, &bindings.check_changed)?,
        dependency_topology: optional(&bindings.dependency_topology)?,
        dependency_topology_metadata: optional(&bindings.dependency_topology_metadata)?,
        workspace_scope: optional(&bindings.workspace_scope)?,
        export_index: optional(&bindings.export_index)?,
        guide: optional(&bindings.guide)?,
    })
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
    static REGISTERED_LANGUAGE_IDS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    REGISTERED_LANGUAGE_IDS
        .get_or_init(|| {
            let mut language_ids = language_provider_manifests()
                .into_iter()
                .map(|manifest| manifest.language_id)
                .collect::<Vec<_>>();
            language_ids.sort();
            language_ids.dedup();
            language_ids
        })
        .clone()
}

fn normalize_language_provider_manifest(manifest: &mut ProviderManifest) {
    manifest.schema_id = PROVIDER_MANIFEST_SCHEMA_ID.to_string();
    manifest.schema_version = PROVIDER_MANIFEST_SCHEMA_VERSION.to_string();
    manifest.protocol_id = HOOK_PROTOCOL_ID.to_string();
    manifest.protocol_version = HOOK_PROTOCOL_VERSION.to_string();
    manifest.manifest_version = env!("CARGO_PKG_VERSION").to_string();
    normalize_source_defaults(&mut manifest.source);
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
    method_descriptors: Vec<SemanticMethodDescriptor>,
    query_pack_descriptor: crate::ProviderQueryPackDescriptor,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SemanticMethodDescriptor {
    method: String,
    invocation: crate::protocol::CommandTemplate,
}
