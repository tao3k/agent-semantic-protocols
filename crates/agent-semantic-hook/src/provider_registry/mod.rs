// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Registry-style language registrations used to derive hook provider manifests.

mod catalog;

pub use catalog::ProviderDevelopmentRegistration;
pub use catalog::RegisteredProviderKind;
pub use catalog::materialize_provider_routes;
#[cfg(test)]
pub(crate) use catalog::provider_register;
pub use catalog::registered_language_ids;
pub use catalog::registered_provider_id;
pub use catalog::registered_provider_kind;
pub use catalog::registered_provider_method_invocation;
pub use catalog::registered_provider_projection_operation;
pub use catalog::semantic_registry_digest;

#[cfg(test)]
#[path = "../../tests/unit/provider_registry.rs"]
mod provider_registry_tests;
