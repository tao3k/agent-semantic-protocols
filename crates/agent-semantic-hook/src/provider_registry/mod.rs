//! Registry-style language registrations used to derive hook provider manifests.

mod catalog;

#[cfg(test)]
pub(crate) use catalog::provider_register;
pub use catalog::{
    ProviderDevelopmentRegistration, RegisteredProviderKind, materialize_provider_routes,
    registered_language_ids, registered_provider_id, registered_provider_kind,
    registered_provider_method_invocation, registered_provider_projection_operation,
    semantic_registry_digest,
};

#[cfg(test)]
#[path = "../../tests/unit/provider_registry.rs"]
mod provider_registry_tests;
