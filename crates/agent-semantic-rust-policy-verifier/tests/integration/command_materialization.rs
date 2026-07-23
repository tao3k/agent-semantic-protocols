use agent_semantic_rust_policy_verifier::prepare_command;

use super::fixtures::registry;

#[test]
fn prepare_command_materializes_the_selected_package() {
    let rendered = prepare_command(&registry(), "demo");
    assert!(rendered.contains("--package demo"), "rendered={rendered}");
    assert!(!rendered.contains("{package}"));
}
