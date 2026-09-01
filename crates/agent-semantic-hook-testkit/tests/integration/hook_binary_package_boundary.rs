const HOOK_MANIFEST: &str = include_str!("../../../agent-semantic-hook/Cargo.toml");
const CLIENT_MANIFEST: &str = include_str!("../../../agent-semantic-client/Cargo.toml");
const HOOK_ENTRY: &str = include_str!("../../../agent-semantic-hook/src/bin/asp_hook.rs");
const PLUGIN_LAUNCHER: &str = include_str!("../../../../asp-codex-plugin/bin/asp-hook-exec");
const TESTKIT_MANIFEST: &str = include_str!("../../Cargo.toml");
const CENTRAL_BUILD_GATE: &str =
    include_str!("../../../../build-support/asp-rust-project-harness-policy/build.rs");

#[test]
fn hook_crate_is_the_only_asp_hook_binary_owner() {
    assert!(HOOK_MANIFEST.contains("name = \"asp-hook\""));
    assert!(!CLIENT_MANIFEST.contains("name = \"asp-hook\""));
}

#[test]
fn executable_entry_is_a_thin_hook_library_adapter() {
    assert!(HOOK_ENTRY.contains("agent_semantic_hook::run_hook_binary_from_env()"));
    for forbidden in [
        "serde_json",
        "tokio",
        "config.toml",
        "reader_probe",
        "agent_semantic_client",
        "agent_semantic_client_db",
    ] {
        assert!(
            !HOOK_ENTRY.contains(forbidden),
            "binary entry crossed the Hook library boundary: {forbidden}"
        );
    }
}

#[test]
fn plugin_launcher_forwards_the_event_without_a_repeated_hook_namespace() {
    assert!(PLUGIN_LAUNCHER.contains("exec \"$hook_bin\" \"$@\""));
    assert!(!PLUGIN_LAUNCHER.contains("exec \"$hook_bin\" hook \"$@\""));
}

#[test]
fn testkit_builds_under_the_central_rust_project_policy() {
    assert!(TESTKIT_MANIFEST.contains("[build-dependencies]"));
    assert!(TESTKIT_MANIFEST.contains("asp-rust-project-harness-policy.workspace = true"));
    assert!(CENTRAL_BUILD_GATE.contains("asp_rust_workspace_build_dag_from_env"));
    assert!(CENTRAL_BUILD_GATE.contains("policyCatalogDigest"));
}
