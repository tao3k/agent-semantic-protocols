#[derive(Clone)]
pub(super) struct TursoSourceIndexLookupRequestScope {
    pub(super) project_root: String,
    pub(super) schema_id: String,
    pub(super) schema_version: String,
}

#[derive(Clone)]
pub(super) struct TursoSourceIndexLookupScope {
    pub(super) project_root: String,
    pub(super) schema_id: String,
    pub(super) schema_version: String,
    pub(super) generation_id: String,
    pub(super) source_snapshot_json: String,
}

#[derive(serde::Deserialize)]
pub(super) struct TursoSourceIndexCanonicalSelectorFact {
    pub(super) selector_id: String,
    pub(super) symbol: Option<String>,
    pub(super) kind: Option<String>,
    pub(super) source: String,
    pub(super) projection_record: agent_semantic_content_identity::ExactSelectorProjectionRecordV1,
    pub(super) query_keys: Vec<String>,
}

#[derive(Clone, Copy)]
pub(super) enum TursoSourceIndexCandidateScope<'a> {
    Resolved(&'a TursoSourceIndexLookupScope),
    Requested(&'a TursoSourceIndexLookupRequestScope),
}
