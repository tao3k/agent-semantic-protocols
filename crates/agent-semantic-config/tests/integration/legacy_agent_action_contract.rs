use agent_semantic_config::HookClientConfigFile;

const DEFAULT_CONFIG: &str = include_str!("../../templates/hooks/config.toml");

#[test]
fn default_template_uses_agent_action_rule_names() {
    assert!(DEFAULT_CONFIG.contains("[[rules.match.effectRules]]"));
    assert!(!DEFAULT_CONFIG.contains("effectProjections"));
    assert!(!DEFAULT_CONFIG.contains("authorityProjections"));
    toml::from_str::<HookClientConfigFile>(DEFAULT_CONFIG)
        .expect("default hook config should use the AgentAction contract");
}

#[test]
fn removed_effect_projection_key_is_rejected() {
    let legacy = DEFAULT_CONFIG.replacen(
        "[[rules.match.effectRules]]",
        "[[rules.match.effectProjections]]",
        1,
    );
    let error = toml::from_str::<HookClientConfigFile>(&legacy)
        .expect_err("removed effectProjections key must not be accepted");
    assert!(error.to_string().contains("effectProjections"));
}

#[test]
fn removed_authority_projection_key_is_rejected() {
    let legacy = DEFAULT_CONFIG.replacen(
        "[[rules.match.effectRules]]",
        "[[rules.match.authorityProjections]]",
        1,
    );
    let error = toml::from_str::<HookClientConfigFile>(&legacy)
        .expect_err("removed authorityProjections key must not be accepted");
    assert!(error.to_string().contains("authorityProjections"));
}
