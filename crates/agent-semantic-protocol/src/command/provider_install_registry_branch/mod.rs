//! Provider installation registry branch boundary.

mod core;

pub(crate) use agent_semantic_provider_protocol::{
    ProviderInstallArtifactDomain, ProviderInstallRegistration,
};
pub(crate) use core::{
    provider_install_registration, provider_install_registration_digest,
    provider_install_registrations,
};
