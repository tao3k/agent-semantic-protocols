use super::CompiledRecoveryPromptConfig;

#[test]
fn codex_recovery_has_no_legacy_registry_mutation_command() {
    let config = CompiledRecoveryPromptConfig::default();
    let prompt = config.agent_flow_for("codex").expect("Codex recovery flow");
    assert!(prompt.contains("`collaboration.spawn_agent`"));
    assert!(prompt.contains("`collaboration.list_agents`"));
    assert!(prompt.contains("native Host receipt"));
    assert!(!prompt.contains("asp agent session register"));
}
