use super::{GenerationRepairKey, active_repairs};

#[test]
fn equivalent_cold_repairs_share_one_runtime_server_key() {
    let key = GenerationRepairKey {
        workspace_identity: "workspace-1".to_owned(),
        project_root: std::path::PathBuf::from("/tmp/workspace-1"),
    };
    let mut active = active_repairs()
        .lock()
        .expect("repair coordinator must be available");

    assert!(active.insert(key.clone()));
    assert!(!active.insert(key.clone()));
    assert!(active.remove(&key));
}
