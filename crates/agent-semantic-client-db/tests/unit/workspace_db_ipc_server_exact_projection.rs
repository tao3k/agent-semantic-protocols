#[test]
fn provider_owner_rechecks_runtime_owned_generation_before_resident_read() {
    let source = include_str!("../../src/workspace_db_ipc_server_exact_projection.rs");
    let admission = source
        .find("require_or_submit_terminal_generation_for_read_with_provider")
        .expect("provider owner must reuse Runtime-owned query-demand admission");
    let owner_read = source
        .find("read_projection_owner(workspace_identity")
        .expect("provider owner must read only after admission");
    assert!(admission < owner_read);
    assert!(source.contains("active-workspace-generation-required"));
    assert!(source.contains("runtime-server-resident-owner-missing"));
}
