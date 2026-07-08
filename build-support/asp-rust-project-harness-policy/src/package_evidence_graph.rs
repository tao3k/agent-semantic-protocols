//! Package-level evidence graph receipts for ASP Rust harness policy checks.

use serde::Serialize;

use crate::evidence::{
    AspRustProjectHarnessEvidenceGraphInput, AspRustProjectHarnessEvidenceGraphSummary,
    summarize_client_db_evidence_graph,
};

/// Request for building an ASP package-level evidence graph receipt.
#[derive(Clone, Debug)]
pub struct AspRustProjectHarnessPackageEvidenceGraphRequest<'a> {
    pub package_name: String,
    pub evidence_graph: &'a AspRustProjectHarnessEvidenceGraphInput,
}

/// Receipt that ties a Rust package policy crate to the ASP evidence graph.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AspRustProjectHarnessPackageEvidenceGraphReceipt {
    pub package_name: String,
    pub evidence_graph_summary: AspRustProjectHarnessEvidenceGraphSummary,
}

/// Builds a package evidence graph receipt without writing artifacts.
pub fn build_package_evidence_graph_receipt(
    request: AspRustProjectHarnessPackageEvidenceGraphRequest<'_>,
) -> AspRustProjectHarnessPackageEvidenceGraphReceipt {
    AspRustProjectHarnessPackageEvidenceGraphReceipt {
        package_name: request.package_name,
        evidence_graph_summary: summarize_client_db_evidence_graph(request.evidence_graph),
    }
}
