#[test]
fn cold_generation_open_is_a_read_only_fail_fast_path() {
    let source = include_str!("../../src/server/runtime_server_generation_data_plane.rs");
    assert!(!source.contains("ensure_runtime_generation_ready"));
    assert!(!source.contains("EnsureRuntimeGenerationReady"));
    assert!(source.contains("connect_hook_workspace_session"));
    assert!(
        !source.contains("connect_runtime_server_workspace_session"),
        "query data-plane must not repeat ResolvedState/Git/package discovery"
    );
    assert_eq!(
        source
            .matches("reasonKind=active-workspace-generation-required")
            .count(),
        2,
        "general and exact cold opens must fail closed without starting a writer"
    );
    assert!(
        !source.contains("active-workspace-generation-building"),
        "a public query must not expose the internal Building state"
    );
    assert!(
        !source.contains("retryAfterMs=250"),
        "terminal readiness replaces client retry guidance"
    );
}
