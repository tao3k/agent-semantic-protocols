#[test]
fn search_generation_open_is_a_socket_free_sectioned_read_path() {
    let source = include_str!("../../src/server/runtime_server_generation_data_plane.rs");
    assert!(source.contains("WorkspaceSearchGenerationDataPlaneClient"));
    assert!(source.contains("read_source_index"));
    assert!(source.contains("read_graph_facts"));
    assert!(source.contains("read_owner"));
    assert!(!source.contains("WorkspaceDbIpcSession"));
    assert!(!source.contains("WorkspaceGenerationDataPlaneClient::open_state"));
    assert!(!source.contains("ensure_runtime_generation"));
    assert!(!source.contains("runtime_search_generation_authority"));
}

#[test]
fn exact_generation_open_is_a_socket_free_immutable_read_path() {
    let source = include_str!("../../src/server/runtime_server_generation_data_plane.rs");
    assert!(!source.contains("ensure_runtime_generation_ready"));
    assert!(!source.contains("EnsureRuntimeGenerationReady"));
    let exact_owner = source
        .split("pub(crate) async fn runtime_server_workspace_exact_projection_async")
        .nth(1)
        .expect("exact projection owner");
    assert!(exact_owner.contains("runtime_server_workspace_generation_pointer_async"));
    assert!(exact_owner.contains("WorkspaceExactProjectionDataPlaneClient::open_state"));
    assert!(!exact_owner.contains("generation_session(project_root)"));
    assert!(!exact_owner.contains("read_runtime_selector(language_id"));
    assert!(!source.contains("connect_hook_workspace_session"));
    assert!(
        !source.contains("connect_runtime_server_workspace_session"),
        "query data-plane must not repeat ResolvedState/Git/package discovery"
    );
    let exact = include_str!("../../src/command/provider_resident_exact.rs");
    assert!(!exact.contains("provider_native_exact_fallback_reason"));
    assert!(!exact.contains("run_provider_command"));
    assert!(!exact.contains("provider_native_exact_fallback_reason"));
    assert!(
        !source.contains("active-workspace-generation-building"),
        "a public query must not expose the internal Building state"
    );
    assert!(
        !source.contains("retryAfterMs=250"),
        "terminal readiness replaces client retry guidance"
    );
}
