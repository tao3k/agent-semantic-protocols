// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Language-neutral result contract for the resident search data plane.

pub const RESIDENT_SEARCH_RESULT_SCHEMA_ID: &str =
    "agent.semantic-protocols.resident-search-result";
pub const RESIDENT_SEARCH_RESULT_SCHEMA_VERSION: &str = "1";
pub const RUNTIME_PROVIDER_SEARCH_RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.runtime-provider-search-receipt";
pub const RUNTIME_PROVIDER_SEARCH_RECEIPT_SCHEMA_VERSION: &str = "1";

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResidentSearchProjectionTier {
    ShallowNavigation,
    OwnerLocalDynamic,
}

impl ResidentSearchProjectionTier {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ShallowNavigation => "shallow-navigation",
            Self::OwnerLocalDynamic => "owner-local-dynamic",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum ResidentSearchReadyState {
    Ready,
}

impl ResidentSearchReadyState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResidentSearchHit {
    pub owner_path: String,
    pub owner_content_digest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language_id: Option<String>,
    pub projection_tier: ResidentSearchProjectionTier,
    pub line_count: u32,
    pub query_keys: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selector: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<u32>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResidentSearchWorkCounters {
    pub database_read_count: u64,
    pub filesystem_read_count: u64,
    pub provider_process_count: u64,
    pub socket_operation_count: u64,
    pub scheduler_task_count: u64,
}

impl ResidentSearchWorkCounters {
    pub fn validate_zero_io(self) -> Result<(), String> {
        if self == Self::default() {
            Ok(())
        } else {
            Err("resident search performed request-time external work".to_owned())
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResidentSearchReadyResult {
    pub schema_id: String,
    pub schema_version: String,
    pub state: ResidentSearchReadyState,
    pub generation_digest: String,
    pub root_digest: String,
    pub provider_digest: String,
    pub index_artifact_digest: String,
    pub hits: Vec<ResidentSearchHit>,
    pub work_counters: ResidentSearchWorkCounters,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeProviderSearchReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub operation_id: String,
    pub status: String,
    pub language_id: String,
    pub generation_digest: String,
    pub root_digest: String,
    pub provider_digest: String,
    pub index_artifact_digest: String,
    pub candidate_count: usize,
    pub selector_projection_budget: usize,
    pub projected_owner_count: usize,
    pub selectors: Vec<String>,
    pub owner_paths: Vec<String>,
    pub resident_read_elapsed_micros: u64,
    pub service_elapsed_micros: u64,
    pub elapsed_micros: u64,
    pub work_counters: ResidentSearchWorkCounters,
}

impl RuntimeProviderSearchReceipt {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != RUNTIME_PROVIDER_SEARCH_RECEIPT_SCHEMA_ID
            || self.schema_version != RUNTIME_PROVIDER_SEARCH_RECEIPT_SCHEMA_VERSION
            || self.operation_id.is_empty()
            || self.language_id.is_empty()
            || !matches!(self.status.as_str(), "matches" | "no-matches")
            || !self.generation_digest.starts_with("blake3-256:")
            || self.root_digest.is_empty()
            || self.provider_digest.is_empty()
            || self.index_artifact_digest.is_empty()
            || self.selector_projection_budget == 0
            || self.projected_owner_count > self.selector_projection_budget
            || self.elapsed_micros
                != self
                    .resident_read_elapsed_micros
                    .saturating_add(self.service_elapsed_micros)
        {
            return Err("runtime provider search receipt identity is invalid".to_owned());
        }
        self.work_counters.validate_zero_io()
    }
}

impl ResidentSearchReadyResult {
    pub fn new(
        generation_digest: String,
        source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
        index_artifact_digest: String,
        hits: Vec<ResidentSearchHit>,
    ) -> Result<Self, String> {
        let result = Self {
            schema_id: RESIDENT_SEARCH_RESULT_SCHEMA_ID.to_owned(),
            schema_version: RESIDENT_SEARCH_RESULT_SCHEMA_VERSION.to_owned(),
            state: ResidentSearchReadyState::Ready,
            generation_digest,
            root_digest: source_snapshot.root_digest.clone(),
            provider_digest: source_snapshot.provider_digest.clone(),
            index_artifact_digest,
            hits,
            work_counters: ResidentSearchWorkCounters::default(),
        };
        result.validate()?;
        Ok(result)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != RESIDENT_SEARCH_RESULT_SCHEMA_ID
            || self.schema_version != RESIDENT_SEARCH_RESULT_SCHEMA_VERSION
            || self.state != ResidentSearchReadyState::Ready
            || !self.generation_digest.starts_with("blake3-256:")
            || self.root_digest.is_empty()
            || self.provider_digest.is_empty()
            || self.index_artifact_digest.is_empty()
        {
            return Err("resident search result identity is invalid".to_owned());
        }
        self.work_counters.validate_zero_io()
    }
}
