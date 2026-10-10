// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Registered provider identities projected into compiled Hook policy.

mod catalog;

#[cfg(test)]
pub(crate) use catalog::provider_register;
pub use catalog::registered_language_ids;
pub use catalog::registered_provider_id;
pub use catalog::semantic_registry_digest;

#[cfg(test)]
#[path = "../../tests/unit/provider_registry.rs"]
mod provider_registry_tests;
