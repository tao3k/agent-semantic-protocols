use super::CompiledRecoveryPromptConfig;

#[test]
fn codex_recovery_has_no_legacy_registry_mutation_command() {
    let config = CompiledRecoveryPromptConfig::default();
    let prompt = config.agent_flow_for("codex").expect("Codex recovery flow");
    assert!(prompt.contains("`collaboration.spawn_agent`"));
    assert!(prompt.contains("`collaboration.list_agents({"));
    assert!(prompt.contains("`collaboration.followup_task`"));
    assert!(prompt.contains("current Host capability snapshot"));
    assert!(prompt.contains("`multi_agent_v2`"));
    assert!(prompt.contains("`reasonKind=codex-multi-agent-v2-capability-unavailable`"));
    assert!(!prompt.contains("under another namespace"));
    assert!(!prompt.contains("multi_agent_v1"));
    assert!(prompt.contains("never emit a tool name that is not callable"));
    assert!(prompt.contains("`reasonKind=host-agent-observation-capability-unavailable`"));
    assert!(prompt.contains("`bootstrap-pending`"));
    assert!(prompt.contains("`state=empty-payload`"));
    assert!(
        prompt.find("collaboration.followup_task") < prompt.find("collaboration.spawn_agent"),
        "followup_task must be presented before spawn_agent"
    );
    assert!(prompt.contains("native Host receipt"));
    assert!(!prompt.contains("asp agent session register"));
}
