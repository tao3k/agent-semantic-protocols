use super::{
    WorkspaceDbIpcOperation, WorkspaceDbSourceIndexLookupRequest, is_search_data_plane_read,
    is_tree_sitter_data_plane_operation,
};

#[test]
fn tree_sitter_inventory_uses_the_tree_sitter_data_plane_budget() {
    let operation = WorkspaceDbIpcOperation::ReadTreeSitterInventory {
        request: WorkspaceDbSourceIndexLookupRequest {
            project_root: std::path::PathBuf::from("/workspace"),
            indexed_project_root: std::path::PathBuf::from("/workspace"),
            query: ".rs".to_owned(),
            language_id: None,
            limit: u32::MAX,
        },
    };

    assert!(is_tree_sitter_data_plane_operation(&operation));
    assert!(!is_search_data_plane_read(&operation));
}
