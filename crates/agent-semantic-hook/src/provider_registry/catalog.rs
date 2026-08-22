//! Registry-style language registrations used to derive hook provider manifests.

use serde::Deserialize;

use super::argument_projection::{
    ProviderMethodArgumentProjectionV1, ProviderMethodArgumentSlotNameV1,
    ProviderMethodArgumentTokenV1,
};
use crate::protocol::{
    HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, PROVIDER_MANIFEST_SCHEMA_ID,
    PROVIDER_MANIFEST_SCHEMA_VERSION,
};
use crate::protocol_activation::protocol_activation_manifest::ProviderManifest;

fn provider_register_json() -> &'static str {
    agent_semantic_provider_protocol::builtin_provider_register_json()
}

pub fn semantic_registry_digest() -> String {
    let digest = <sha2::Sha256 as sha2::Digest>::digest(provider_register_json().as_bytes());
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
            let registry: serde_json::Value = serde_json::from_str(provider_register_json())
                .expect("embedded provider register must be valid JSON");
            registry
                .get("providers")
                .and_then(serde_json::Value::as_array)
                .expect("embedded provider register must declare providers")
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

pub fn schema_registry_provider_manifests() -> Vec<ProviderManifest> {
    static MANIFESTS: std::sync::OnceLock<Vec<ProviderManifest>> = std::sync::OnceLock::new();
    MANIFESTS.get_or_init(build_provider_manifests).clone()
}

fn build_provider_manifests() -> Vec<ProviderManifest> {
    let manifests = provider_register()
        .providers
        .iter()
        .map(|language| {
            let mut manifest = language.provider.clone();
            normalize_language_provider_manifest(&mut manifest);
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
    let mut programming_projection_contract_present = false;
    let mut capability_drift = Vec::new();

    for manifest in manifests {
        let project_resolution = manifest.project_resolution();
        let document_resolution = manifest.document_resolution();
        let source_snapshot = manifest.search_capabilities().source_snapshot.as_ref();
        match (project_resolution, document_resolution, source_snapshot) {
            (Some(_), Some(_), Some(projection))
            | (Some(_), None, Some(projection))
            | (None, Some(_), Some(projection)) => {
                if project_resolution.is_some() {
                    programming_projection_contract_present = true;
                }
                let contract = (
                    projection.descriptor_version().to_owned(),
                    projection.packet_schema_id().to_owned(),
                    projection.exact_source_packet_schema_id().to_owned(),
                    projection.canonical_item_selector_schema_id().to_owned(),
                    projection.source_snapshot_envelope_schema_id().to_owned(),
                    projection.derived_artifact_evidence_schema_id().to_owned(),
                    projection.algorithm().to_owned(),
                    projection.authority().to_owned(),
                    projection.exact_selector_resolution().to_owned(),
                    projection.overlay_mode().to_owned(),
                );
                if let Some(expected) = canonical_projection_contract.as_ref() {
                    if &contract != expected {
                        capability_drift.push(format!(
                            "language={} provider={} sourceSnapshotContract={contract:?} expected={expected:?}",
                            manifest.language_id(),
                            manifest.provider_id(),
                        ));
                    }
                } else {
                    canonical_projection_contract = Some(contract);
                }
            }
            _ => capability_drift.push(format!(
                "language={} provider={} projectResolution={} documentResolution={} sourceSnapshot={}",
                manifest.language_id(),
                manifest.provider_id(),
                manifest.project_resolution().is_some(),
                manifest.document_resolution().is_some(),
                source_snapshot.is_some(),
            )),
        }
    }

    if !programming_projection_contract_present {
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
    if development.build_binding != "root-development-installer-v1"
        && development.build_binding != "provider-workspace-install-v1"
    {
        return Err(format!(
            "unsupported development build binding `{}`",
            development.build_binding
        ));
    }
    match (
        development.build_binding.as_str(),
        development.workspace_install.as_deref(),
    ) {
        ("provider-workspace-install-v1", Some(reference)) => {
            let reference = std::path::Path::new(reference);
            if reference.is_absolute()
                || reference.as_os_str().is_empty()
                || reference
                    .components()
                    .any(|component| component == std::path::Component::ParentDir)
                || reference.extension().and_then(|value| value.to_str()) != Some("json")
            {
                return Err(format!(
                    "development workspaceInstall must be a repository-relative JSON reference: {}",
                    reference.display()
                ));
            }
        }
        ("provider-workspace-install-v1", None) => {
            return Err(
                "provider-workspace-install-v1 requires development.workspaceInstall".to_string(),
            );
        }
        ("root-development-installer-v1", Some(_)) => {
            return Err(
                "root-development-installer-v1 must not declare development.workspaceInstall"
                    .to_string(),
            );
        }
        _ => {}
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

pub fn registered_provider_projection_operation(
    language_id: &str,
    provider_id: &str,
) -> Result<Option<String>, String> {
    let Some(registration) = provider_register()
        .providers
        .iter()
        .find(|registration| registration.language_id == language_id)
    else {
        return Ok(None);
    };
    let manifest = &registration.provider;
    if manifest.provider_id.as_str() != provider_id {
        return Err(format!(
            "ProviderRegistry provider drift for language `{language_id}`: expected {}, got {provider_id}",
            manifest.provider_id
        ));
    }
    Ok(manifest
        .runtime_contract()
        .operations()
        .iter()
        .find(|operation| operation.operation() == "projection-batch")
        .map(|operation| operation.operation().to_owned()))
}

pub fn registered_provider_method_invocation_v1(
    language_id: &str,
    provider_id: &str,
    method: &str,
) -> Result<Option<crate::protocol::CommandTemplate>, String> {
    let register = provider_register();
    let Some(language) = register
        .providers
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
    provider_register()
        .providers
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
    let registration = provider_register()
        .providers
        .iter()
        .find(|registration| registration.language_id == language_id)
        .ok_or_else(|| format!("no registered provider for language `{language_id}`"))?;
    match registration.provider.execution() {
        crate::ProviderExecution::ExternalProcess => {
            Ok(RegisteredProviderKind::ProgrammingLanguage)
        }
        crate::ProviderExecution::Embedded => Ok(RegisteredProviderKind::Document),
    }
}

pub fn materialize_provider_routes(
    manifest: &ProviderManifest,
) -> Result<crate::protocol::HookRoutes, String> {
    let register = provider_register();
    let language = register
        .providers
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

    // A complete registry materialization resolves every route for every
    // provider. Index once per provider rather than rescanning the descriptor
    // list for each binding; this keeps the cold install-time path within its
    // millisecond budget without changing method-admission semantics.
    let mut invocations = std::collections::BTreeMap::new();
    for descriptor in &language.method_descriptors {
        if invocations
            .insert(descriptor.method.as_str(), descriptor.invocation.clone())
            .is_some()
        {
            return Err(format!(
                "semantic registry has duplicate method descriptor `{}` for language `{}` provider `{}`",
                descriptor.method, language.language_id, language.provider_id
            ));
        }
    }
    let bindings = &manifest.route_bindings;
    let resolve = |method: &str| {
        invocations.get(method).cloned().ok_or_else(|| {
            format!(
                "semantic registry has no method descriptor `{method}` for language `{}` provider `{}`",
                language.language_id, language.provider_id
            )
        })
    };
    let optional = |method: &Option<String>| method.as_deref().map(resolve).transpose();
    Ok(crate::protocol::HookRoutes {
        prime: resolve(&bindings.prime)?,
        owner: resolve(&bindings.owner)?,
        lexical: resolve(&bindings.lexical)?,
        query: optional(&bindings.query)?,
        exact_selector_native: optional(&bindings.exact_selector_native)?,
        ingest: resolve(&bindings.ingest)?,
        check_changed: resolve(&bindings.check_changed)?,
        dependency_topology: optional(&bindings.dependency_topology)?,
        dependency_topology_metadata: optional(&bindings.dependency_topology_metadata)?,
        export_index: optional(&bindings.export_index)?,
        guide: optional(&bindings.guide)?,
    })
}

/// Return registered ASP language ids from the provider registry schema.
pub fn registered_language_ids() -> Vec<agent_semantic_config::LanguageId> {
    provider_register()
        .providers
        .iter()
        .map(|registration| {
            agent_semantic_config::LanguageId::new(registration.language_id.as_str())
        })
        .collect()
}

fn normalize_language_provider_manifest(manifest: &mut ProviderManifest) {
    manifest.schema_id = PROVIDER_MANIFEST_SCHEMA_ID.to_string();
    manifest.schema_version = PROVIDER_MANIFEST_SCHEMA_VERSION.to_string();
    manifest.protocol_id = HOOK_PROTOCOL_ID.to_string();
    manifest.protocol_version = HOOK_PROTOCOL_VERSION.to_string();
    manifest.manifest_version = env!("CARGO_PKG_VERSION").to_string();
}

pub(crate) fn provider_register() -> &'static ProviderRegister {
    static REGISTER: std::sync::OnceLock<ProviderRegister> = std::sync::OnceLock::new();
    REGISTER.get_or_init(|| {
        let register: ProviderRegister = serde_json::from_str(provider_register_json())
            .expect("embedded provider register must be valid JSON");
        validate_provider_register(&register)
            .expect("embedded provider register must be internally consistent");
        register
    })
}

/// Materialize deterministic provider argv from a registered closed projection.
pub fn registered_provider_method_projected_argv_v1(
    language_id: &str,
    provider_id: &str,
    method: &str,
    values: &super::ProviderMethodArgumentValuesV1,
) -> Result<Vec<String>, String> {
    use super::{
        ProviderMethodArgumentSlotNameV1, ProviderMethodArgumentTokenV1,
        ProviderMethodArgumentValueTypeV1,
    };

    let unavailable = |detail: &str| {
        format!(
            "reasonKind=provider-native-argument-projection-unavailable languageId={language_id} providerId={provider_id} method={method} detail={detail}"
        )
    };
    let language = provider_register()
        .providers
        .iter()
        .find(|language| language.language_id == language_id)
        .ok_or_else(|| unavailable("language-not-registered"))?;
    if language.provider_id != provider_id {
        return Err(unavailable("provider-identity-drift"));
    }
    let descriptor = language
        .method_descriptors
        .iter()
        .find(|descriptor| descriptor.method == method)
        .ok_or_else(|| unavailable("method-not-registered"))?;
    let projection = descriptor
        .argument_projection
        .as_ref()
        .ok_or_else(|| unavailable("projection-not-declared"))?;
    if projection.schema_version != "1" {
        return Err(unavailable("unsupported-schema-version"));
    }
    projection
        .tokens
        .iter()
        .map(|token| match token {
            ProviderMethodArgumentTokenV1::Literal(literal) => Ok(literal.value.clone()),
            ProviderMethodArgumentTokenV1::Slot(slot) => {
                let value = match (&slot.name, &slot.value_type) {
                    (
                        ProviderMethodArgumentSlotNameV1::Query,
                        ProviderMethodArgumentValueTypeV1::String,
                    ) => values.query.as_ref(),
                    (
                        ProviderMethodArgumentSlotNameV1::Workspace,
                        ProviderMethodArgumentValueTypeV1::Path,
                    ) => values.workspace.as_ref(),
                    (
                        ProviderMethodArgumentSlotNameV1::Presentation,
                        ProviderMethodArgumentValueTypeV1::Presentation,
                    ) => values.presentation.as_ref(),
                    (
                        ProviderMethodArgumentSlotNameV1::Owner,
                        ProviderMethodArgumentValueTypeV1::Path,
                    ) if method.starts_with("search/owner") => values.owner.as_ref(),
                    _ => return Err(unavailable("slot-type-or-method-mismatch")),
                };
                value
                    .filter(|value| !value.is_empty())
                    .cloned()
                    .ok_or_else(|| unavailable(&format!("missing-slot-{}", slot.name.as_str())))
            }
        })
        .collect()
}

fn validate_provider_register(register: &ProviderRegister) -> Result<(), String> {
    let mut language_ids = std::collections::BTreeSet::new();
    for language in &register.providers {
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
        for descriptor in &language.method_descriptors {
            let Some(projection) = descriptor.argument_projection.as_ref() else {
                continue;
            };
            if projection.schema_version != "1" {
                return Err(format!(
                    "ProviderRegistry argument projection schemaVersion must be `1` for language `{}` method `{}`",
                    language.language_id, descriptor.method
                ));
            }
            if descriptor.method == "search/lexical"
                && projection.tokens.iter().any(|token| {
                    matches!(
                        token,
                        ProviderMethodArgumentTokenV1::Slot(slot)
                            if slot.name == ProviderMethodArgumentSlotNameV1::Owner
                    )
                })
            {
                return Err(format!(
                    "ProviderRegistry lexical argument projection declares owner slot for language `{}`",
                    language.language_id
                ));
            }
        }
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderRegister {
    pub(crate) providers: Vec<ProviderRegistration>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderRegistration {
    pub(crate) language_id: String,
    pub(crate) provider_id: String,
    pub(crate) provider: ProviderManifest,
    pub(crate) binary: String,
    methods: Vec<String>,
    pub(crate) method_descriptors: Vec<SemanticMethodDescriptor>,
    pub(crate) query_pack_descriptor: crate::ProviderQueryPackDescriptor,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SemanticMethodDescriptor {
    pub(crate) method: String,
    pub(crate) invocation: crate::protocol::CommandTemplate,
    pub(crate) argument_projection: Option<ProviderMethodArgumentProjectionV1>,
}
