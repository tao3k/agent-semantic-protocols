//! Language-neutral result contract for the resident search data plane.

pub const RESIDENT_SEARCH_RESULT_SCHEMA_ID: &str =
    "agent.semantic-protocols.resident-search-result";
pub const RESIDENT_SEARCH_RESULT_SCHEMA_VERSION: &str = "1";

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResidentSearchProjectionTier {
    ShallowNavigation,
    OwnerLocalDynamic,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum ResidentSearchReadyState {
    Ready,
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
