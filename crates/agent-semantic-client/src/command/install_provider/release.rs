// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Shared release metadata for provider installation.

/// Resolved immutable release specification for one provider installation.
#[derive(Clone, Debug)]
pub(super) struct ProviderReleaseSpec {
    pub(super) language_id: String,
    pub(super) provider_id: String,
    pub(super) binary: String,
    pub(super) repo: String,
    pub(super) release_version: String,
    pub(super) download_base_url: String,
    pub(super) archive_prefix: String,
    pub(super) archive_binary: String,
    pub(super) require_native_binary: bool,
    pub(super) supported_targets: Vec<String>,
    pub(super) sha256_by_target: std::collections::BTreeMap<String, String>,
}
