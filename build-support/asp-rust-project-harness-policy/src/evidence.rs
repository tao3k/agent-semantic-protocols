//! `agent-semantic-client-db` evidence graph summaries for the ASP policy crate.

use std::path::PathBuf;

use serde::Serialize;

/// Lightweight projection of a client-db evidence graph for build-support policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AspRustProjectHarnessEvidenceGraphInput {
    pub generation_id: String,
    pub project_root: PathBuf,
    pub node_count: usize,
    pub edge_count: usize,
}

/// Compact, package-neutral summary of a client-db evidence graph.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AspRustProjectHarnessEvidenceGraphSummary {
    pub schema_id: &'static str,
    pub schema_version: &'static str,
    pub generation_id: String,
    pub project_root: std::path::PathBuf,
    pub node_count: usize,
    pub edge_count: usize,
}

/// Builds the policy-crate evidence summary from the client-db graph owner.
pub fn summarize_client_db_evidence_graph(
    graph: &AspRustProjectHarnessEvidenceGraphInput,
) -> AspRustProjectHarnessEvidenceGraphSummary {
    AspRustProjectHarnessEvidenceGraphSummary {
        schema_id: "asp.rust-project-harness.evidence-graph-summary",
        schema_version: "1",
        generation_id: graph.generation_id.clone(),
        project_root: graph.project_root.clone(),
        node_count: graph.node_count,
        edge_count: graph.edge_count,
    }
}
