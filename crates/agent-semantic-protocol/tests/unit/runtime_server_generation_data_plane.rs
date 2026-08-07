#[test]
fn cold_generation_open_is_a_read_only_fail_fast_path() {
    let source = include_str!("../../src/server/runtime_server_generation_data_plane.rs");
    assert!(!source.contains("ensure_runtime_generation_ready"));
    assert!(!source.contains("EnsureRuntimeGenerationReady"));
    assert!(source.contains("runtime_server_workspace_session_async"));
    assert!(!source.contains("connect_hook_workspace_session"));
    assert!(!source.contains("runtime_generation_pointer_path"));
    assert!(
        !source.contains("connect_runtime_server_workspace_session"),
        "query data-plane must not repeat ResolvedState/Git/package discovery"
    );
    let exact = include_str!("../../src/command/provider_resident_exact.rs");
    assert!(exact.contains("reasonKind=active-workspace-generation-required"));
    assert!(exact.contains("provider_native_exact_fallback_reason"));
    assert!(
        !source.contains("active-workspace-generation-building"),
        "a public query must not expose the internal Building state"
    );
    assert!(
        !source.contains("retryAfterMs=250"),
        "terminal readiness replaces client retry guidance"
    );
}

#[test]
fn exact_selector_is_routed_as_a_pure_global_runtime_read() {
    let server = include_str!("../../../agent-semantic-client-db/src/workspace_db_ipc_server.rs");
    let start = server
        .find("WorkspaceDbIpcOperation::ReadRuntimeSelector {")
        .expect("selector operation");
    let end = server[start..]
        .find("WorkspaceDbIpcOperation::ReadRuntimeOwner {")
        .map(|offset| start + offset)
        .expect("next read operation");
    let selector = &server[start..end];
    assert!(selector.contains("memory_registry.read_runtime_selector"));
    assert!(!selector.contains("ensure_terminal_ready"));
    assert!(!selector.contains("restore_published_generation"));
    assert!(!selector.contains("schedule_single_flight_discovery"));
}

#[test]
fn read_only_runtime_session_cannot_inherit_a_generation_pointer_cache() {
    let protocol =
        include_str!("../../../agent-semantic-client-db/src/workspace_db_ipc/protocol.rs");
    let start = protocol
        .find("pub fn for_runtime_server_read_only(")
        .expect("read-only global Runtime constructor");
    let end = protocol[start..]
        .find("pub(super) fn runtime_project_root")
        .map(|offset| start + offset)
        .expect("constructor boundary");
    let constructor = &protocol[start..end];
    assert!(constructor.contains("generation_pointer_path: None"));
    assert!(constructor.contains("WorkspaceDbSessionProfile::HookReadOnly"));
    assert!(!constructor.contains("resident_state"));
}
