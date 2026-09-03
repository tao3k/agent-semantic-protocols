//! Provider installation registry branch boundary.

mod core;

pub(crate) use agent_semantic_provider_protocol::ProviderInstallRegistration;
pub(crate) use core::{
    provider_install_registration, provider_install_registration_digest,
    provider_install_registrations, provider_install_registry_digest,
};
