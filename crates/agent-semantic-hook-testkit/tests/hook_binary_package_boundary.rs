const HOOK_MANIFEST: &str = include_str!("../../agent-semantic-hook/Cargo.toml");
const CLIENT_MANIFEST: &str = include_str!("../../agent-semantic-client/Cargo.toml");
const HOOK_ENTRY: &str = include_str!("../../agent-semantic-hook/src/bin/asp_hook.rs");

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
