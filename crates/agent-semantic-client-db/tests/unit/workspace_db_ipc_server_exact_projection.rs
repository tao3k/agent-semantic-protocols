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
