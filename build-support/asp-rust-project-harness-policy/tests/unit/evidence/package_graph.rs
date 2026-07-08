use std::path::PathBuf;

use asp_rust_project_harness_policy::{
    AspRustProjectHarnessEvidenceGraphInput, AspRustProjectHarnessPackageEvidenceGraphRequest,
    build_package_evidence_graph_receipt,
};

#[test]
fn package_receipt_summarizes_client_db_evidence_graph() {
    let graph = AspRustProjectHarnessEvidenceGraphInput {
        generation_id: "gen-test".to_string(),
        project_root: PathBuf::from("/tmp/asp"),
        node_count: 0,
        edge_count: 0,
    };

    let receipt =
        build_package_evidence_graph_receipt(AspRustProjectHarnessPackageEvidenceGraphRequest {
            package_name: "agent-semantic-client-db".to_string(),
            evidence_graph: &graph,
        });

    assert_eq!(receipt.package_name, "agent-semantic-client-db");
    assert_eq!(receipt.evidence_graph_summary.generation_id, "gen-test");
    assert_eq!(receipt.evidence_graph_summary.node_count, 0);
    assert_eq!(receipt.evidence_graph_summary.edge_count, 0);
}
