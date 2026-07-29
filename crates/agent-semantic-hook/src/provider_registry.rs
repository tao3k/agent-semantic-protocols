//! Registry-style language registrations used to derive hook provider manifests.

use serde::Deserialize;

use crate::protocol::{
    HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, PROVIDER_MANIFEST_SCHEMA_ID,
    PROVIDER_MANIFEST_SCHEMA_VERSION,
};
use crate::protocol_activation::protocol_activation_manifest::{
    ManifestSourceDefaults, ProviderManifest,
};

// The embedded registry is the admission authority for provider-native routes.
const SCHEMA_REGISTRY_JSON: &str =
    include_str!("../../../schemas/semantic-language-registry.providers.v1.json");

pub fn semantic_registry_digest() -> String {
    let digest = <sha2::Sha256 as sha2::Digest>::digest(SCHEMA_REGISTRY_JSON.as_bytes());
    format!("sha256:{digest:x}")
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegisteredProviderCatalogIdentity {
    pub language_id: String,
    pub manifest_digest: String,
    pub provider_registry_digest: String,
    pub query_pack_digest: String,
    pub exact_query_pack_identity_digest: String,
}

pub fn registered_provider_catalog_identities() -> &'static [RegisteredProviderCatalogIdentity] {
    static IDENTITIES: std::sync::OnceLock<Vec<RegisteredProviderCatalogIdentity>> =
        std::sync::OnceLock::new();
    IDENTITIES
        .get_or_init(|| {
            let manifests = schema_registry_provider_manifests();
            let registry: serde_json::Value = serde_json::from_str(SCHEMA_REGISTRY_JSON)
                .expect("embedded semantic language registry must be valid JSON");
            registry
                .get("languages")
                .and_then(serde_json::Value::as_array)
                .expect("embedded semantic language registry must declare languages")
                .iter()
                .map(|descriptor| {
                    let language_id = descriptor
                        .get("languageId")
                        .and_then(serde_json::Value::as_str)
                        .expect("registered language descriptor must declare languageId");
                    let manifest = manifests
                        .iter()
                        .find(|manifest| manifest.language_id.as_str() == language_id)
                        .unwrap_or_else(|| {
                            panic!(
                                "missing provider manifest for registered language `{}`",
                                language_id
                            )
                        });
                    let descriptor_bytes = serde_json::to_vec(descriptor)
                        .expect("registered language descriptor must serialize");
                    let query_pack_bytes =
                        serde_json::to_vec(descriptor.get("queryPackDescriptor").expect(
                            "registered language descriptor must declare queryPackDescriptor",
                        ))
                        .expect("registered query pack descriptor must serialize");
                    RegisteredProviderCatalogIdentity {
                        language_id: language_id.to_owned(),
                        manifest_digest:
                            crate::protocol_activation::digest::provider_manifest_digest(manifest)
                                .expect("registered provider manifest must serialize"),
                        provider_registry_digest: format!(
                            "sha256:{:x}",
                            <sha2::Sha256 as sha2::Digest>::digest(descriptor_bytes)
                        ),
                        query_pack_digest: format!(
                            "sha256:{:x}",
                            <sha2::Sha256 as sha2::Digest>::digest(&query_pack_bytes)
                        ),
                        exact_query_pack_identity_digest:
                            agent_semantic_content_identity::exact_selector_projection_packet::
                                derive_query_pack_identity_digest_v1(&query_pack_bytes)
                                .as_str()
                                .to_owned(),
                    }
                })
                .collect()
        })
        .as_slice()
}

pub fn registered_language_descriptor_digest(language_id: &str) -> Option<String> {
    registered_provider_catalog_identities()
        .iter()
        .find(|identity| identity.language_id == language_id)
        .map(|identity| identity.provider_registry_digest.clone())
}

pub fn registered_query_pack_digest(language_id: &str) -> Option<String> {
    registered_provider_catalog_identities()
        .iter()
        .find(|identity| identity.language_id == language_id)
        .map(|identity| identity.query_pack_digest.clone())
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

pub fn schema_registry_provider_manifests() -> Vec<ProviderManifest> {
    let language_manifests = language_provider_manifests();
    schema_registry()
        .languages
        .iter()
        .map(|language| {
            let manifest = language_manifests
                .iter()
                .find(|manifest| {
                    manifest.language_id.as_str() == language.language_id
                        && manifest.provider_id.as_str() == language.provider_id
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
            assert_eq!(
                language.binary, manifest.binary,
                "registry binary drift for language `{}` provider `{}`",
                language.language_id, language.provider_id
            );
            manifest
        })
        .collect()
}

pub fn registered_provider_method_invocation_v1(
    language_id: &str,
    provider_id: &str,
    method: &str,
) -> Result<Option<crate::protocol::CommandTemplate>, String> {
    let registry = schema_registry();
    let Some(language) = registry
        .languages
        .iter()
        .find(|language| language.language_id == language_id)
    else {
        return Ok(None);
    };
    if language.provider_id != provider_id {
        return Err(format!(
            "ProviderRegistry provider drift for language `{language_id}`: expected {}, got {provider_id}",
            language.provider_id
        ));
    }
    Ok(language
        .method_descriptors
        .iter()
        .find(|descriptor| descriptor.method == method)
        .map(|descriptor| descriptor.invocation.clone()))
}

pub fn registered_provider_id_v1(language_id: &str) -> Option<String> {
    schema_registry()
        .languages
        .iter()
        .find(|language| language.language_id == language_id)
        .map(|language| language.provider_id.clone())
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
            language.language_id == manifest.language_id.as_str()
                && language.provider_id == manifest.provider_id.as_str()
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
        exact_selector_native: optional(&bindings.exact_selector_native)?,
        ingest: resolve_route_invocation(language, &bindings.ingest)?,
        check_changed: resolve_route_invocation(language, &bindings.check_changed)?,
        dependency_topology: optional(&bindings.dependency_topology)?,
        dependency_topology_metadata: optional(&bindings.dependency_topology_metadata)?,
        workspace_scope: optional(&bindings.workspace_scope)?,
        export_index: optional(&bindings.export_index)?,
        guide: optional(&bindings.guide)?,
    })
}

#[cfg(test)]
#[path = "../tests/unit/provider_registry.rs"]
mod provider_registry_tests;

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

/// Return registered ASP language ids from the provider registry schema.
pub fn registered_language_ids() -> Vec<agent_semantic_config::LanguageId> {
    static REGISTERED_LANGUAGE_IDS: std::sync::OnceLock<Vec<agent_semantic_config::LanguageId>> =
        std::sync::OnceLock::new();
    REGISTERED_LANGUAGE_IDS
        .get_or_init(|| {
            let mut language_ids = schema_registry()
                .languages
                .iter()
                .map(|registration| {
                    agent_semantic_config::LanguageId::new(registration.language_id.clone())
                })
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

fn schema_registry() -> &'static SemanticLanguageRegistry {
    static REGISTRY: std::sync::OnceLock<SemanticLanguageRegistry> = std::sync::OnceLock::new();
    REGISTRY.get_or_init(|| {
        let registry: SemanticLanguageRegistry = serde_json::from_str(SCHEMA_REGISTRY_JSON)
            .expect("embedded semantic language registry must be valid JSON");
        validate_schema_registry_v1(&registry)
            .expect("embedded semantic language registry must be internally consistent");
        registry
    })
}

fn validate_schema_registry_v1(registry: &SemanticLanguageRegistry) -> Result<(), String> {
    let mut language_ids = std::collections::BTreeSet::new();
    for language in &registry.languages {
        if !language_ids.insert(language.language_id.as_str()) {
            return Err(format!(
                "duplicate ProviderRegistry languageId `{}`",
                language.language_id
            ));
        }

        let descriptor_methods = language
            .method_descriptors
            .iter()
            .map(|descriptor| descriptor.method.as_str())
            .collect::<Vec<_>>();
        let declared_methods = language
            .methods
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        let descriptor_method_set = descriptor_methods
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        let declared_method_set = declared_methods
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        if descriptor_method_set != declared_method_set {
            return Err(format!(
                "ProviderRegistry method inventory drift for language `{}`: methods={declared_methods:?} descriptors={descriptor_methods:?}",
                language.language_id
            ));
        }
        if descriptor_method_set.len() != descriptor_methods.len()
            || declared_method_set.len() != declared_methods.len()
        {
            return Err(format!(
                "duplicate ProviderRegistry method for language `{}`",
                language.language_id
            ));
        }
    }
    Ok(())
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
    methods: Vec<String>,
    method_descriptors: Vec<SemanticMethodDescriptor>,
    query_pack_descriptor: crate::ProviderQueryPackDescriptor,
}

/// Binary identity declared by one v1 ProviderRegistry registration.
///
/// Runtime publication must consume this identity instead of inferring an
/// executable name from an install target or release archive.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegisteredProviderBinaryV1 {
    language_id: agent_semantic_config::LanguageId,
    provider_id: agent_semantic_config::ProviderId,
    binary: String,
}

impl RegisteredProviderBinaryV1 {
    #[must_use]
    pub fn language_id(&self) -> &agent_semantic_config::LanguageId {
        &self.language_id
    }

    #[must_use]
    pub fn provider_id(&self) -> &agent_semantic_config::ProviderId {
        &self.provider_id
    }

    #[must_use]
    pub fn binary(&self) -> &str {
        &self.binary
    }
}

#[must_use]
pub fn registered_provider_binaries_v1() -> Vec<RegisteredProviderBinaryV1> {
    schema_registry()
        .languages
        .iter()
        .map(|registration| RegisteredProviderBinaryV1 {
            language_id: agent_semantic_config::LanguageId::new(registration.language_id.clone()),
            provider_id: agent_semantic_config::ProviderId::new(registration.provider_id.clone()),
            binary: registration.binary.clone(),
        })
        .collect()
}

pub fn registered_provider_binary_v1(
    language_id: &str,
) -> Result<RegisteredProviderBinaryV1, String> {
    registered_provider_binaries_v1()
        .into_iter()
        .find(|registration| registration.language_id.as_str() == language_id)
        .ok_or_else(|| {
            format!(
                "language `{language_id}` is not registered in semantic-language-registry.providers.v1"
            )
        })
}

#[cfg(test)]
#[path = "../tests/unit/provider_registry_binary_identity.rs"]
mod provider_registry_binary_identity_tests;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SemanticMethodDescriptor {
    method: String,
    invocation: crate::protocol::CommandTemplate,
}
