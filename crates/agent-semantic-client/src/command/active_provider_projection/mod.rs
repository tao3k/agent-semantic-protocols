// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Installed provider artifact branch boundary.

mod core;

pub(crate) use core::RuntimeActiveProviderProjection;
pub(crate) use core::active_runtime_bundle_digest;
pub(crate) use core::load_runtime_active_provider_projection;
pub(crate) use core::provider_languages_for_generation_demand;
pub(crate) use core::runtime_source_index_provider_projection;
