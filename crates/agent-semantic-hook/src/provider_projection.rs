// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Hook-owned provider policy projections used by source classification.

use serde::Deserialize;
use serde::Serialize;

use crate::protocol::HookPolicy;

#[derive(Clone, Debug)]
/// In-memory view of one compiled Hook policy and provider projection.
pub struct HookRuntime {
    pub project_root: String,
    pub policy_providers: Vec<HookProviderProjection>,
}

/// Provider-owned language and routing facts compiled for Hook matching.
///
/// This deliberately excludes execution activation. A document provider or a
/// language provider whose harness is not active still participates in Hook
/// policy without pretending that an executable provider was activated.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct HookProviderProjection {
    pub language_id: agent_semantic_config::LanguageId,
    pub provider_id: agent_semantic_config::ProviderId,
    pub package_roots: Vec<String>,
    pub source_extensions: Vec<String>,
    pub config_files: Vec<String>,
    pub policy: HookPolicy,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SourceSelectorKind {
    ExactPath,
    Pattern,
}

#[derive(Clone, Debug)]
pub(crate) struct ProviderSelectorMatch {
    pub provider: HookProviderProjection,
    pub kind: SourceSelectorKind,
}
