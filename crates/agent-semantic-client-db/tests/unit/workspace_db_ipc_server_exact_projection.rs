#[test]
fn provider_owner_demand_starts_and_awaits_the_resident_runtime() {
    let source = include_str!("../../src/workspace_db_ipc_server_exact_projection.rs");
    let start = source.find(".provider_runtime(").expect("runtime start");
    let await_ready = source
        .find(".provider_runtime_await_ready(")
        .expect("runtime await-ready");
    let owner = source.find(".provider_owner(").expect("provider owner");
    assert!(start < await_ready && await_ready < owner);
}

#[test]
fn provider_owner_projection_publishes_the_runtime_owned_cache_before_returning() {
    let source = include_str!("../../src/workspace_db_ipc_server_exact_projection.rs");
    let publication = source
        .find(".publish_provider_owner(")
        .expect("Runtime-owned provider owner publication");
    let projection = source
        .find("WorkspaceDbIpcResult::ProviderOwnerProjection")
        .expect("owner projection result");
    assert!(publication < projection);
    assert!(source.contains("runtime-server-provider-owner-publication-failed"));
}

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
