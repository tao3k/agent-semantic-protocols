use std::path::PathBuf;

use asp_rust_project_harness_policy::AspRustProjectHarnessEvidenceGraphInput;
use asp_rust_project_harness_policy::workspace_evidence_graph::{
    AspRustProjectHarnessWorkspaceEvidenceGraphEdgeKind,
    AspRustProjectHarnessWorkspaceEvidenceGraphNodeKind,
    AspRustProjectHarnessWorkspaceEvidenceGraphRequest, build_asp_workspace_evidence_graph_receipt,
    build_workspace_evidence_graph_receipt,
};

#[test]
fn workspace_receipt_projects_member_crates_and_client_db_graph() {
    let graph = AspRustProjectHarnessEvidenceGraphInput {
        generation_id: "gen-workspace".to_string(),
        project_root: PathBuf::from("/tmp/asp"),
        node_count: 0,
        edge_count: 0,
    };

    let receipt = build_workspace_evidence_graph_receipt(
        AspRustProjectHarnessWorkspaceEvidenceGraphRequest {
            workspace_label: "agent-semantic-protocols".to_string(),
            workspace_root: PathBuf::from("/tmp/asp"),
            member_crate_names: vec!["agent-semantic-client-db".to_string()],
            client_db_evidence_graph: &graph,
        },
    );

    assert_eq!(receipt.summary.member_crate_count, 1);
    assert_eq!(receipt.summary.evidence_graph_node_count, 3);
    assert_eq!(receipt.summary.evidence_graph_edge_count, 2);
    assert!(
        receipt
            .nodes
            .iter()
            .any(|node| node.kind == AspRustProjectHarnessWorkspaceEvidenceGraphNodeKind::Workspace)
    );
    assert!(
        receipt
            .edges
            .iter()
            .any(|edge| edge.kind == AspRustProjectHarnessWorkspaceEvidenceGraphEdgeKind::Contains)
    );
}

#[test]
fn default_workspace_receipt_uses_central_member_policy_registry() {
    let graph = AspRustProjectHarnessEvidenceGraphInput {
        generation_id: "gen-default-workspace".to_string(),
        project_root: PathBuf::from("/tmp/asp"),
        node_count: 0,
        edge_count: 0,
    };

    let receipt = build_asp_workspace_evidence_graph_receipt(PathBuf::from("/tmp/asp"), &graph);

    assert_eq!(receipt.summary.member_crate_count, 6);
    assert!(
        receipt
            .members
            .iter()
            .any(|member| member.package_name == "agent-semantic-client-db")
    );
}
