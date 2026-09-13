// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Provider installation registry branch boundary.

mod core;

pub(crate) use agent_semantic_provider_protocol::ProviderInstallRegistration;
pub(crate) use core::provider_install_registration;
pub(crate) use core::provider_install_registration_digest;
pub(crate) use core::provider_install_registry_digest;
