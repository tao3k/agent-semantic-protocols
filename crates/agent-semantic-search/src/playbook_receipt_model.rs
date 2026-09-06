//! Public V1 Search Playbook receipt model.

use agent_semantic_search_projection::RuntimeProviderSearchReceipt;
use serde::Deserialize;
use serde::Serialize;

use crate::NativeSyntaxDiagnostic;
use crate::NativeSyntaxProjection;
use crate::NativeSyntaxRelation;
use crate::ResidentGraphSearchStage;

pub const SEARCH_PLAYBOOK_RECEIPT_SCHEMA_ID: &str = "asp.search.playbook-receipt";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchPlaybookReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub workspace_identity: String,
    pub language_id: String,
    pub provider_digest: String,
    pub index_artifact_digest: String,
    pub generation_digest: String,
    pub source_root_digest: String,
    pub operation_id: String,
    pub candidate_count: usize,
    pub resident_read_elapsed_micros: u64,
    pub service_elapsed_micros: u64,
    pub elapsed_micros: u64,
    pub work_counters: agent_semantic_search_projection::ResidentSearchWorkCounters,
    pub query: String,
    pub intent: String,
    pub state: String,
    pub plan: SearchPlaybookPlan,
    pub evidence: SearchPlaybookEvidence,
    pub decision: SearchPlaybookDecision,
    pub metrics: SearchPlaybookMetrics,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchPlaybookPlan {
    pub stages: Vec<SearchPlaybookStage>,
    pub coverage: String,
    pub max_owners: u32,
    pub deadline_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchPlaybookStage {
    pub family: String,
    pub capability_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchPlaybookEvidence {
    pub source_acquisition: SearchPlaybookSourceAcquisitionEvidence,
    pub native_syntax: SearchPlaybookNativeSyntaxEvidence,
    pub indexed_lexical: SearchPlaybookLexicalEvidence,
    pub byte_evidence: SearchPlaybookByteEvidence,
    pub resident_graph: SearchPlaybookGraphEvidence,
    pub ripgrep: SearchPlaybookRgEvidence,
    pub python_graph: SearchPlaybookPythonGraphEvidence,
    pub correlation: SearchPlaybookCorrelation,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchPlaybookSourceAcquisitionEvidence {
    pub state: String,
    pub stage_artifact_digest: String,
    pub generation_digest: String,
    pub admitted_owner_count: usize,
    pub complete: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchPlaybookNativeSyntaxEvidence {
    pub state: String,
    pub stage_artifact_digest: String,
    pub projections: Vec<NativeSyntaxProjection>,
    pub relations: Vec<NativeSyntaxRelation>,
    pub diagnostics: Vec<NativeSyntaxDiagnostic>,
    pub proper_byte_ranges: bool,
    pub non_empty_query_keys: bool,
    pub relations_bound_to_owners: bool,
    pub complete: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchPlaybookLexicalEvidence {
    pub state: String,
    pub backend: String,
    pub generation_digest: String,
    pub index_artifact_digest: String,
    pub candidate_owner_ids: Vec<String>,
    pub indexed_owner_count: usize,
    pub admitted_owner_count: usize,
    pub complete: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchPlaybookGraphEvidence {
    pub state: String,
    pub backend: String,
    pub generation_digest: String,
    pub result_digest: String,
    pub entry_owner_ids: Vec<String>,
    pub entry_node_ids: Vec<String>,
    pub candidate_owner_ids: Vec<String>,
    pub closure_state: String,
    pub unresolved_frontier_count: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchPlaybookByteEvidence {
    pub state: String,
    pub mode: String,
    pub generation_digest: String,
    pub candidate_owner_ids: Vec<String>,
    pub coverage_complete: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchPlaybookRgEvidence {
    pub state: String,
    pub reason_kind: String,
    pub mode: String,
    pub generation_digest: String,
    pub coverage_input_digest: Option<String>,
    pub candidate_owner_ids: Vec<String>,
    pub process_count: u8,
    pub complete: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchPlaybookPythonGraphEvidence {
    pub state: String,
    pub reason_kind: String,
    pub backend: String,
    pub generation_digest: String,
    pub projection_digest: Option<String>,
    pub candidate_owner_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchPlaybookCorrelation {
    pub lexical_graph_overlap_count: usize,
    pub lexical_graph_jaccard_permille: usize,
    pub graph_marginal_candidate_count: usize,
    pub candidate_union_count: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchPlaybookDecision {
    pub chosen_path: String,
    pub explanation: String,
    pub residual_uncertainty: Vec<String>,
    pub selectors: Vec<String>,
    pub owner_paths: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchPlaybookMetrics {
    pub total_elapsed_micros: u64,
    pub stage_elapsed_micros: SearchPlaybookStageMetrics,
    pub command_count: u8,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchPlaybookStageMetrics {
    pub native_syntax: u64,
    pub indexed_lexical: u64,
    pub byte_evidence: u64,
    pub resident_graph: u64,
    pub ripgrep: u64,
    pub python_graph: u64,
}

pub struct SearchPlaybookReceiptInput {
    pub workspace_identity: String,
    pub query: String,
    pub intent: String,
    pub coverage: String,
    pub max_owners: u32,
    pub deadline_ms: u64,
    pub indexed_owner_count: usize,
    pub indexed_lexical_executed: bool,
    pub resident_graph_executed: bool,
    pub byte_evidence_executed: bool,
    pub source_acquisition_stage_artifact_digest: String,
    pub native_syntax_state: String,
    pub native_syntax_stage_artifact_digest: String,
    pub native_syntax_projections: Vec<NativeSyntaxProjection>,
    pub native_syntax_relations: Vec<NativeSyntaxRelation>,
    pub native_syntax_diagnostics: Vec<NativeSyntaxDiagnostic>,
    pub native_syntax_elapsed_micros: u64,
    pub runtime: RuntimeProviderSearchReceipt,
    pub graph: ResidentGraphSearchStage,
    pub cold_rg: Option<SearchPlaybookColdRgExecution>,
    pub python_graph: Option<SearchPlaybookPythonGraphExecution>,
}

pub struct SearchPlaybookColdRgExecution {
    pub generation_digest: String,
    pub coverage_input_digest: String,
    pub candidate_owner_ids: Vec<String>,
    pub elapsed_micros: u64,
    pub process_count: u8,
}

pub struct SearchPlaybookPythonGraphExecution {
    pub generation_digest: String,
    pub projection_digest: String,
    pub candidate_owner_ids: Vec<String>,
    pub elapsed_micros: u64,
}
