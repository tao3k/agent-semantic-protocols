use super::CompiledRecoveryPromptConfig;

#[test]
fn codex_recovery_has_no_legacy_registry_mutation_command() {
    let config = CompiledRecoveryPromptConfig::default();
    let prompt = config.agent_flow_for("codex").expect("Codex recovery flow");
    assert!(prompt.contains("re-enter `asp session --agents choice-plane`"));
    assert!(prompt.contains("native SubagentStart/host receipt"));
    assert!(!prompt.contains("asp agent session register"));
}
