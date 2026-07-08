//! Workspace-level evidence graph receipts for ASP Rust policy crates.

use std::path::PathBuf;

use serde::Serialize;

use crate::evidence::AspRustProjectHarnessEvidenceGraphInput;
use crate::member_policy::asp_workspace_member_policies;
use crate::package_evidence_graph::{
    AspRustProjectHarnessPackageEvidenceGraphReceipt,
    AspRustProjectHarnessPackageEvidenceGraphRequest, build_package_evidence_graph_receipt,
};

/// Request for projecting ASP client-db evidence into a workspace graph.
#[derive(Clone, Debug)]
pub struct AspRustProjectHarnessWorkspaceEvidenceGraphRequest<'a> {
    pub workspace_label: String,
    pub workspace_root: PathBuf,
    pub member_crate_names: Vec<String>,
    pub client_db_evidence_graph: &'a AspRustProjectHarnessEvidenceGraphInput,
}

/// Agent-facing workspace evidence graph receipt owned by the ASP policy crate.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AspRustProjectHarnessWorkspaceEvidenceGraphReceipt {
    pub schema_id: &'static str,
    pub schema_version: &'static str,
    pub workspace_label: String,
    pub workspace_root: PathBuf,
    pub summary: AspRustProjectHarnessWorkspaceEvidenceGraphSummaryReceipt,
    pub members: Vec<AspRustProjectHarnessPackageEvidenceGraphReceipt>,
    pub nodes: Vec<AspRustProjectHarnessWorkspaceEvidenceGraphNodeReceipt>,
    pub edges: Vec<AspRustProjectHarnessWorkspaceEvidenceGraphEdgeReceipt>,
}

/// Aggregated counts for the ASP workspace evidence graph.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AspRustProjectHarnessWorkspaceEvidenceGraphSummaryReceipt {
    pub member_crate_count: usize,
    pub client_db_graph_node_count: usize,
    pub client_db_graph_edge_count: usize,
    pub evidence_graph_node_count: usize,
    pub evidence_graph_edge_count: usize,
}

/// One node in the ASP workspace evidence graph receipt.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AspRustProjectHarnessWorkspaceEvidenceGraphNodeReceipt {
    pub id: String,
    pub kind: AspRustProjectHarnessWorkspaceEvidenceGraphNodeKind,
    pub label: String,
}

/// Node kind in the ASP workspace evidence graph receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AspRustProjectHarnessWorkspaceEvidenceGraphNodeKind {
    Workspace,
    MemberCrate,
    ClientDbEvidenceGraph,
}

/// One directed edge in the ASP workspace evidence graph receipt.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AspRustProjectHarnessWorkspaceEvidenceGraphEdgeReceipt {
    pub source: String,
    pub target: String,
    pub kind: AspRustProjectHarnessWorkspaceEvidenceGraphEdgeKind,
}

/// Edge kind in the ASP workspace evidence graph receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AspRustProjectHarnessWorkspaceEvidenceGraphEdgeKind {
    Contains,
    ProjectsClientDbEvidence,
}

/// Builds an ASP workspace evidence graph receipt without writing artifacts.
pub fn build_workspace_evidence_graph_receipt(
    request: AspRustProjectHarnessWorkspaceEvidenceGraphRequest<'_>,
) -> AspRustProjectHarnessWorkspaceEvidenceGraphReceipt {
    let workspace_node_id = format!("workspace:{}", request.workspace_label);
    let client_db_graph_node_id = format!(
        "client-db-evidence-graph:{}",
        request.client_db_evidence_graph.generation_id
    );

    let mut nodes = Vec::with_capacity(request.member_crate_names.len() + 2);
    nodes.push(AspRustProjectHarnessWorkspaceEvidenceGraphNodeReceipt {
        id: workspace_node_id.clone(),
        kind: AspRustProjectHarnessWorkspaceEvidenceGraphNodeKind::Workspace,
        label: request.workspace_label.clone(),
    });
    nodes.push(AspRustProjectHarnessWorkspaceEvidenceGraphNodeReceipt {
        id: client_db_graph_node_id.clone(),
        kind: AspRustProjectHarnessWorkspaceEvidenceGraphNodeKind::ClientDbEvidenceGraph,
        label: request.client_db_evidence_graph.generation_id.clone(),
    });

    let mut edges = vec![AspRustProjectHarnessWorkspaceEvidenceGraphEdgeReceipt {
        source: workspace_node_id.clone(),
        target: client_db_graph_node_id,
        kind: AspRustProjectHarnessWorkspaceEvidenceGraphEdgeKind::ProjectsClientDbEvidence,
    }];

    let mut members = Vec::with_capacity(request.member_crate_names.len());
    for member_crate_name in request.member_crate_names {
        let member_node_id = format!("member-crate:{member_crate_name}");
        nodes.push(AspRustProjectHarnessWorkspaceEvidenceGraphNodeReceipt {
            id: member_node_id.clone(),
            kind: AspRustProjectHarnessWorkspaceEvidenceGraphNodeKind::MemberCrate,
            label: member_crate_name.clone(),
        });
        edges.push(AspRustProjectHarnessWorkspaceEvidenceGraphEdgeReceipt {
            source: workspace_node_id.clone(),
            target: member_node_id,
            kind: AspRustProjectHarnessWorkspaceEvidenceGraphEdgeKind::Contains,
        });
        members.push(build_package_evidence_graph_receipt(
            AspRustProjectHarnessPackageEvidenceGraphRequest {
                package_name: member_crate_name,
                evidence_graph: request.client_db_evidence_graph,
            },
        ));
    }

    AspRustProjectHarnessWorkspaceEvidenceGraphReceipt {
        schema_id: "asp.rust-project-harness.workspace-evidence-graph",
        schema_version: "1",
        workspace_label: request.workspace_label,
        workspace_root: request.workspace_root,
        summary: AspRustProjectHarnessWorkspaceEvidenceGraphSummaryReceipt {
            member_crate_count: members.len(),
            client_db_graph_node_count: request.client_db_evidence_graph.node_count,
            client_db_graph_edge_count: request.client_db_evidence_graph.edge_count,
            evidence_graph_node_count: nodes.len(),
            evidence_graph_edge_count: edges.len(),
        },
        members,
        nodes,
        edges,
    }
}

/// Builds the default ASP workspace evidence graph from the central policy registry.
pub fn build_asp_workspace_evidence_graph_receipt(
    workspace_root: PathBuf,
    client_db_evidence_graph: &AspRustProjectHarnessEvidenceGraphInput,
) -> AspRustProjectHarnessWorkspaceEvidenceGraphReceipt {
    build_workspace_evidence_graph_receipt(AspRustProjectHarnessWorkspaceEvidenceGraphRequest {
        workspace_label: "agent-semantic-protocols".to_string(),
        workspace_root,
        member_crate_names: asp_workspace_member_policies()
            .iter()
            .map(|policy| policy.package_name.to_string())
            .collect(),
        client_db_evidence_graph,
    })
}
