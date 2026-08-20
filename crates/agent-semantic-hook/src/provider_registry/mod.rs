//! Registry-style language registrations used to derive hook provider manifests.

mod argument_projection;
mod catalog;
mod runtime_binary;

pub use argument_projection::ProviderMethodArgumentValuesV1;
pub(crate) use argument_projection::{
    ProviderMethodArgumentSlotNameV1, ProviderMethodArgumentTokenV1,
    ProviderMethodArgumentValueTypeV1,
};
pub use catalog::registered_provider_method_projected_argv_v1;
pub use catalog::{
    ProviderDevelopmentRegistrationV1, RegisteredProviderCatalogIdentity, RegisteredProviderKind,
    materialize_provider_routes, registered_language_descriptor_digest, registered_language_ids,
    registered_provider_catalog_identities, registered_provider_development_v1,
    registered_provider_id_v1, registered_provider_kind, registered_provider_method_invocation_v1,
    registered_provider_projection_command_binding, registered_query_pack_digest,
    schema_registry_provider_manifests, semantic_registry_digest,
};
#[cfg(test)]
pub(crate) use catalog::{
    REGISTERED_LANGUAGE_ID_STRINGS, language_provider_manifests, schema_registry,
    validate_registered_provider_projection_contracts,
};
#[cfg(test)]
use runtime_binary::executable_identity_matches_v1;
pub use runtime_binary::{
    RegisteredProviderBinaryV1, RuntimeBinaryAdmissionDenialV1, RuntimeBinaryClassificationV1,
    RuntimeBinaryDispatchRequirementsV1, RuntimeBinaryIdentityBindingV1,
    RuntimeBinaryInvocationAuthorityV1, RuntimeBinaryProfileV1, SessionBindingV1,
    classify_runtime_executable_v1, registered_provider_binaries_v1, registered_provider_binary_v1,
    registered_provider_matches_candidate_paths, runtime_binary_dispatch_requirements_v1,
};

#[cfg(test)]
#[path = "../../tests/unit/provider_registry_binary_identity.rs"]
mod provider_registry_binary_identity_tests;
#[cfg(test)]
#[path = "../../tests/unit/provider_registry_candidate_admission.rs"]
mod provider_registry_candidate_admission_tests;
#[cfg(test)]
#[path = "../../tests/unit/provider_registry.rs"]
mod provider_registry_tests;
#[cfg(test)]
#[path = "../../tests/unit/provider_registry/registered_language_projection.rs"]
mod registered_language_projection_tests;
