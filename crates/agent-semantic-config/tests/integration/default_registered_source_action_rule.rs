use agent_semantic_config::default_hook_client_config_file;

#[test]
fn registered_source_rule_uses_read_default_wrapping_plus_language_profiles() {
    let config = default_hook_client_config_file().expect("embedded hook config");
    let rule = config
        .rules
        .iter()
        .find(|rule| rule.id == "route-read-to-asp-languages")
        .expect("registered source rule");
    assert!(rule.matcher.is_none());
    assert!(rule.matcher_policies.is_empty());
    assert_eq!(
        rule.actions,
        [agent_semantic_config::HookClientActionKind::Read]
    );
    assert!(rule.match_config.capability_policy_all.is_empty());
    assert!(rule.profiles_list.contains(&"rust".to_owned()));
    assert!(rule.profiles_list.contains(&"gerbil-scheme".to_owned()));
    assert!(config.capability_policies.is_empty());
}
