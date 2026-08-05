//! Runtime binary identities and admission requirements derived from the provider registry.

use super::catalog::{
    RegisteredProviderKind, registered_provider_kind, schema_registry,
    schema_registry_provider_manifests,
};

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

pub(super) fn executable_identity_matches_v1(executable: &str, registered_binary: &str) -> bool {
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
