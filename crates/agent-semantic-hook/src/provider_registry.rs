//! Registry-style language registrations used to derive hook provider manifests.

use serde::Deserialize;

use crate::protocol::{
    HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, PROVIDER_MANIFEST_SCHEMA_ID,
    PROVIDER_MANIFEST_SCHEMA_VERSION,
};
use crate::protocol_activation::protocol_activation_manifest::ProviderManifest;

// The embedded registry is the admission authority for provider-native routes.
const SCHEMA_REGISTRY_JSON: &str = include_str!(concat!(
    env!("OUT_DIR"),
    "/semantic-language-registry.providers.resolved.v1.json"
));

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
    include_str!("../../../languages/orgize/provider/asp-org-provider-manifest.json"),
    include_str!("../../../languages/orgize/provider/asp-md-provider-manifest.json"),
];

pub fn schema_registry_provider_manifests() -> Vec<ProviderManifest> {
    static MANIFESTS: std::sync::OnceLock<Vec<ProviderManifest>> = std::sync::OnceLock::new();
    MANIFESTS.get_or_init(build_provider_manifests).clone()
}

fn build_provider_manifests() -> Vec<ProviderManifest> {
    let language_manifests = language_provider_manifests();
    let manifests = schema_registry()
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
            validate_provider_development_descriptor(&manifest).unwrap_or_else(|error| {
                panic!(
                    "invalid provider development descriptor for language `{}` provider `{}`: {error}",
                    language.language_id, language.provider_id
                )
            });
            manifest
        })
        .collect::<Vec<_>>();
    validate_registered_provider_projection_contracts(&manifests)
        .unwrap_or_else(|error| panic!("invalid registered provider projection contract: {error}"));
    manifests
}

pub(crate) fn validate_registered_provider_projection_contracts(
    manifests: &[ProviderManifest],
) -> Result<(), String> {
    let mut canonical_projection_contract = None;
    let mut capability_drift = Vec::new();

    for manifest in manifests {
        match (
            manifest.project_resolution(),
            manifest.document_resolution(),
            manifest.language_projection(),
        ) {
            (Some(_), None, Some(projection)) => {
                let contract = (
                    projection.schema_id().to_owned(),
                    projection.schema_version().to_owned(),
                    projection.command_binding().to_owned(),
                    projection.transport().to_owned(),
                    projection.request_schema().to_owned(),
                    projection.response_schema().to_owned(),
                    projection.identity_schema().to_owned(),
                );
                if let Some(expected) = canonical_projection_contract.as_ref() {
                    if &contract != expected {
                        capability_drift.push(format!(
                            "language={} provider={} projectionContract={contract:?} expected={expected:?}",
                            manifest.language_id(),
                            manifest.provider_id(),
                        ));
                    }
                } else {
                    canonical_projection_contract = Some(contract);
                }
            }
            (None, Some(_), None) => {}
            _ => capability_drift.push(format!(
                "language={} provider={} projectResolution={} documentResolution={} languageProjection={}",
                manifest.language_id(),
                manifest.provider_id(),
                manifest.project_resolution().is_some(),
                manifest.document_resolution().is_some(),
                manifest.language_projection().is_some(),
            )),
        }
    }

    if canonical_projection_contract.is_none() {
        capability_drift
            .push("registered catalog has no programming provider projection contract".to_owned());
    }
    if capability_drift.is_empty() {
        Ok(())
    } else {
        Err(capability_drift.join("\n"))
    }
}

fn validate_provider_development_descriptor(manifest: &ProviderManifest) -> Result<(), String> {
    let development = manifest.development();
    if development.schema_id != "agent.semantic-protocols.provider-development-descriptor"
        || development.schema_version != "1"
    {
        return Err("development descriptor schema identity must be v1".to_string());
    }
    if development.build_binding != "root-development-installer-v1" {
        return Err(format!(
            "unsupported development build binding `{}`",
            development.build_binding
        ));
    }
    let source_root = std::path::Path::new(&development.source_root);
    if source_root.is_absolute()
        || source_root.as_os_str().is_empty()
        || source_root
            .components()
            .any(|component| component == std::path::Component::ParentDir)
    {
        return Err(format!(
            "development sourceRoot must be a non-empty repository-relative path: {}",
            development.source_root
        ));
    }
    Ok(())
}

pub fn registered_provider_development_v1(
    language_id: &str,
) -> Result<ProviderDevelopmentRegistrationV1, String> {
    let manifest = schema_registry_provider_manifests()
        .into_iter()
        .find(|manifest| manifest.language_id().as_str() == language_id)
        .ok_or_else(|| format!("no ProviderRegistry development descriptor for `{language_id}`"))?;
    let development = manifest.development().clone();
    Ok(ProviderDevelopmentRegistrationV1 {
        language_id: manifest.language_id().clone(),
        provider_id: manifest.provider_id().clone(),
        binary: manifest.binary().to_string(),
        development,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderDevelopmentRegistrationV1 {
    pub language_id: agent_semantic_config::LanguageId,
    pub provider_id: agent_semantic_config::ProviderId,
    pub binary: String,
    pub development:
        crate::protocol_activation::protocol_activation_manifest::ProviderDevelopmentDescriptor,
}

pub fn registered_provider_projection_command_binding_v1(
    language_id: &str,
    provider_id: &str,
) -> Result<Option<String>, String> {
    let manifests = language_provider_manifests();
    let Some(manifest) = manifests
        .iter()
        .find(|manifest| manifest.language_id.as_str() == language_id)
    else {
        return Ok(None);
    };
    if manifest.provider_id.as_str() != provider_id {
        return Err(format!(
            "ProviderRegistry provider drift for language `{language_id}`: expected {}, got {provider_id}",
            manifest.provider_id
        ));
    }
    Ok(manifest
        .language_projection
        .as_ref()
        .map(|descriptor| descriptor.command_binding().to_owned()))
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

/// Registry-owned execution class derived from the provider manifest contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegisteredProviderKind {
    ProgrammingLanguage,
    Document,
}

/// Return the execution class declared by the registered provider manifest.
///
/// Programming-language providers own package/project resolution and may be
/// installed independently. Document providers are compiled into the ASP
/// runtime and must never enter the external language-provider installer.
pub fn registered_provider_kind(language_id: &str) -> Result<RegisteredProviderKind, String> {
    let manifests = language_provider_manifests();
    let manifest = manifests
        .iter()
        .find(|manifest| manifest.language_id().as_str() == language_id)
        .ok_or_else(|| format!("no registered provider for language `{language_id}`"))?;
    match (
        manifest.project_resolution().is_some(),
        manifest.document_resolution().is_some(),
    ) {
        (true, false) => Ok(RegisteredProviderKind::ProgrammingLanguage),
        (false, true) => Ok(RegisteredProviderKind::Document),
        _ => Err(format!(
            "registered provider kind is ambiguous: languageId={} providerId={}",
            manifest.language_id(),
            manifest.provider_id(),
        )),
    }
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
            let mut manifest =
                serde_json::from_str::<ProviderManifest>(json).unwrap_or_else(|error| {
                    panic!("embedded language provider manifest must be valid JSON: {error}")
                });
            normalize_language_provider_manifest(&mut manifest);
            manifest
        })
        .collect()
}

include!(concat!(env!("OUT_DIR"), "/registered_language_ids.rs"));

/// Return registered ASP language ids from the provider registry schema.
pub fn registered_language_ids() -> Vec<agent_semantic_config::LanguageId> {
    REGISTERED_LANGUAGE_ID_STRINGS
        .iter()
        .map(|language_id| agent_semantic_config::LanguageId::new(*language_id))
        .collect()
}

pub(crate) fn registered_language_id(candidate: &str) -> Option<agent_semantic_config::LanguageId> {
    REGISTERED_LANGUAGE_ID_STRINGS
        .iter()
        .find(|language_id| language_id.eq_ignore_ascii_case(candidate))
        .map(|language_id| agent_semantic_config::LanguageId::new(*language_id))
}

#[cfg(test)]
#[path = "../tests/unit/provider_registry/registered_language_projection.rs"]
mod registered_language_projection_tests;

fn normalize_language_provider_manifest(manifest: &mut ProviderManifest) {
    manifest.schema_id = PROVIDER_MANIFEST_SCHEMA_ID.to_string();
    manifest.schema_version = PROVIDER_MANIFEST_SCHEMA_VERSION.to_string();
    manifest.protocol_id = HOOK_PROTOCOL_ID.to_string();
    manifest.protocol_version = HOOK_PROTOCOL_VERSION.to_string();
    manifest.manifest_version = env!("CARGO_PKG_VERSION").to_string();
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
    profile: RuntimeBinaryProfileV1,
}

/// Runtime authority profile for an executable identity.
///
/// Profiles are registry/publisher metadata. They are intentionally not
/// inferred from executable-name substrings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeBinaryProfileV1 {
    Facade,
    ProviderInternal,
    SupportTool,
    HostTool,
    TestFixture,
}

/// Authority that may invoke an executable with a runtime profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeBinaryInvocationAuthorityV1 {
    DirectToolOrShell,
    AspFacadeDispatch,
    AspRuntimeDispatch,
    TestRuntime,
}

/// Whether a dispatch receipt must bind a session identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionBindingV1 {
    None,
    Optional,
    Required,
}

/// Whether a dispatch receipt must bind an identity exactly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeBinaryIdentityBindingV1 {
    Unbound,
    Exact,
}

/// Registry-owned requirements for invoking one runtime binary profile.
///
/// Concrete session IDs, generations, argv, and attempt IDs belong to a
/// server-issued runtime dispatch receipt, not to this descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeBinaryDispatchRequirementsV1 {
    pub profile: RuntimeBinaryProfileV1,
    pub allowed_authorities: &'static [RuntimeBinaryInvocationAuthorityV1],
    pub root_session: SessionBindingV1,
    pub child_session: SessionBindingV1,
    pub generation: RuntimeBinaryIdentityBindingV1,
    pub artifact: RuntimeBinaryIdentityBindingV1,
    pub argv: RuntimeBinaryIdentityBindingV1,
    pub attempt: RuntimeBinaryIdentityBindingV1,
    pub single_use: bool,
}

/// Parser/registry-owned executable classification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeBinaryClassificationV1 {
    Facade,
    RegisteredProviderInternal(Vec<RegisteredProviderBinaryV1>),
    Unregistered,
}

/// Fail-closed reasons available to the hook runtime-binary boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeBinaryAdmissionDenialV1 {
    DirectProviderInternal,
    MissingDispatchCapability,
    UnregisteredRuntimeBinary,
}

const DIRECT_AUTHORITY_V1: &[RuntimeBinaryInvocationAuthorityV1] =
    &[RuntimeBinaryInvocationAuthorityV1::DirectToolOrShell];
const FACADE_AUTHORITIES_V1: &[RuntimeBinaryInvocationAuthorityV1] = &[
    RuntimeBinaryInvocationAuthorityV1::DirectToolOrShell,
    RuntimeBinaryInvocationAuthorityV1::AspFacadeDispatch,
];
const PROVIDER_INTERNAL_AUTHORITIES_V1: &[RuntimeBinaryInvocationAuthorityV1] =
    &[RuntimeBinaryInvocationAuthorityV1::AspRuntimeDispatch];
const TEST_AUTHORITIES_V1: &[RuntimeBinaryInvocationAuthorityV1] =
    &[RuntimeBinaryInvocationAuthorityV1::TestRuntime];

/// Return the dispatch requirements owned by a runtime binary profile.
#[must_use]
pub const fn runtime_binary_dispatch_requirements_v1(
    profile: RuntimeBinaryProfileV1,
) -> RuntimeBinaryDispatchRequirementsV1 {
    match profile {
        RuntimeBinaryProfileV1::Facade => RuntimeBinaryDispatchRequirementsV1 {
            profile,
            allowed_authorities: FACADE_AUTHORITIES_V1,
            root_session: SessionBindingV1::Optional,
            child_session: SessionBindingV1::Optional,
            generation: RuntimeBinaryIdentityBindingV1::Unbound,
            artifact: RuntimeBinaryIdentityBindingV1::Unbound,
            argv: RuntimeBinaryIdentityBindingV1::Unbound,
            attempt: RuntimeBinaryIdentityBindingV1::Unbound,
            single_use: false,
        },
        RuntimeBinaryProfileV1::ProviderInternal => RuntimeBinaryDispatchRequirementsV1 {
            profile,
            allowed_authorities: PROVIDER_INTERNAL_AUTHORITIES_V1,
            root_session: SessionBindingV1::Required,
            child_session: SessionBindingV1::Required,
            generation: RuntimeBinaryIdentityBindingV1::Exact,
            artifact: RuntimeBinaryIdentityBindingV1::Exact,
            argv: RuntimeBinaryIdentityBindingV1::Exact,
            attempt: RuntimeBinaryIdentityBindingV1::Exact,
            single_use: true,
        },
        RuntimeBinaryProfileV1::SupportTool => RuntimeBinaryDispatchRequirementsV1 {
            profile,
            allowed_authorities: PROVIDER_INTERNAL_AUTHORITIES_V1,
            root_session: SessionBindingV1::Required,
            child_session: SessionBindingV1::Required,
            generation: RuntimeBinaryIdentityBindingV1::Exact,
            artifact: RuntimeBinaryIdentityBindingV1::Exact,
            argv: RuntimeBinaryIdentityBindingV1::Exact,
            attempt: RuntimeBinaryIdentityBindingV1::Exact,
            single_use: true,
        },
        RuntimeBinaryProfileV1::HostTool => RuntimeBinaryDispatchRequirementsV1 {
            profile,
            allowed_authorities: DIRECT_AUTHORITY_V1,
            root_session: SessionBindingV1::None,
            child_session: SessionBindingV1::None,
            generation: RuntimeBinaryIdentityBindingV1::Unbound,
            artifact: RuntimeBinaryIdentityBindingV1::Unbound,
            argv: RuntimeBinaryIdentityBindingV1::Unbound,
            attempt: RuntimeBinaryIdentityBindingV1::Unbound,
            single_use: false,
        },
        RuntimeBinaryProfileV1::TestFixture => RuntimeBinaryDispatchRequirementsV1 {
            profile,
            allowed_authorities: TEST_AUTHORITIES_V1,
            root_session: SessionBindingV1::None,
            child_session: SessionBindingV1::None,
            generation: RuntimeBinaryIdentityBindingV1::Unbound,
            artifact: RuntimeBinaryIdentityBindingV1::Unbound,
            argv: RuntimeBinaryIdentityBindingV1::Exact,
            attempt: RuntimeBinaryIdentityBindingV1::Unbound,
            single_use: false,
        },
    }
}

fn executable_identity_matches_v1(executable: &str, registered_binary: &str) -> bool {
    let executable_path = std::path::Path::new(executable);
    let registered_path = std::path::Path::new(registered_binary);
    if executable_path == registered_path
        || executable_path.file_name() == registered_path.file_name()
    {
        return true;
    }

    std::fs::canonicalize(executable_path)
        .ok()
        .and_then(|path| path.file_name().map(std::ffi::OsStr::to_owned))
        .is_some_and(|name| Some(name.as_os_str()) == registered_path.file_name())
}

/// Classify an executable using only the facade identity and provider registry.
///
/// Unknown executables remain explicitly unregistered; callers decide whether
/// their normalized action context is a host tool or a runtime-artifact path.
#[must_use]
pub fn classify_runtime_executable_v1(executable: &str) -> RuntimeBinaryClassificationV1 {
    if std::path::Path::new(executable).file_name() == Some(std::ffi::OsStr::new("asp")) {
        return RuntimeBinaryClassificationV1::Facade;
    }

    let registrations = registered_provider_binaries_v1()
        .into_iter()
        .filter(|registration| executable_identity_matches_v1(executable, registration.binary()))
        .collect::<Vec<_>>();
    if registrations.is_empty() {
        RuntimeBinaryClassificationV1::Unregistered
    } else {
        RuntimeBinaryClassificationV1::RegisteredProviderInternal(registrations)
    }
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

    #[must_use]
    pub const fn profile(&self) -> RuntimeBinaryProfileV1 {
        self.profile
    }
}

#[must_use]
pub fn registered_provider_binaries_v1() -> Vec<RegisteredProviderBinaryV1> {
    all_registered_provider_binaries_v1()
        .into_iter()
        .filter(|registration| {
            registered_provider_kind(registration.language_id().as_str())
                == Ok(RegisteredProviderKind::ProgrammingLanguage)
        })
        .collect()
}

fn all_registered_provider_binaries_v1() -> Vec<RegisteredProviderBinaryV1> {
    schema_registry()
        .languages
        .iter()
        .map(|registration| RegisteredProviderBinaryV1 {
            language_id: agent_semantic_config::LanguageId::new(registration.language_id.clone()),
            provider_id: agent_semantic_config::ProviderId::new(registration.provider_id.clone()),
            binary: registration.binary.clone(),
            profile: RuntimeBinaryProfileV1::ProviderInternal,
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

pub fn registered_provider_matches_candidate_paths<'a>(
    language_id: &str,
    provider_id: &str,
    candidates: impl IntoIterator<Item = &'a std::path::Path>,
) -> Result<bool, String> {
    let manifests = schema_registry_provider_manifests();
    let manifest = manifests
        .iter()
        .find(|manifest| {
            manifest.language_id().as_str() == language_id
                && manifest.provider_id().as_str() == provider_id
        })
        .ok_or_else(|| {
            format!(
                "provider registry has no registered language manifest: languageId={language_id} providerId={provider_id}"
            )
        })?;
    if let Some(document) = manifest.document_resolution() {
        return Ok(candidates.into_iter().any(|candidate| {
            candidate
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| {
                    document
                        .extensions
                        .iter()
                        .any(|registered| registered.trim_start_matches('.') == extension)
                })
        }));
    }
    if let Some(project) = manifest.project_resolution() {
        return Ok(candidates.into_iter().any(|candidate| {
            project.entry_markers.iter().any(|marker| {
                candidate == std::path::Path::new(marker) || candidate.ends_with(marker)
            })
        }));
    }
    Ok(true)
}

#[cfg(test)]
#[path = "../tests/unit/provider_registry_binary_identity.rs"]
mod provider_registry_binary_identity_tests;
#[cfg(test)]
#[path = "../tests/unit/provider_registry_candidate_admission.rs"]
mod provider_registry_candidate_admission_tests;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SemanticMethodDescriptor {
    method: String,
    invocation: crate::protocol::CommandTemplate,
}
