use agent_semantic_config::{HookClientMatcherPolicy, default_hook_client_config_file};

#[test]
fn registered_source_rule_is_wrapped_read_action_plus_language_profiles() {
    let config = default_hook_client_config_file().expect("embedded hook config");
    let rule = config
        .rules
        .iter()
        .find(|rule| rule.id == "route-read-to-asp-languages")
        .expect("registered source rule");
    assert!(rule.matcher.is_none());
    assert_eq!(
        rule.matcher_policies,
        [HookClientMatcherPolicy::WrappedCommand]
    );
    assert_eq!(
        rule.actions,
        [agent_semantic_config::HookClientActionKind::Read]
    );
    assert!(rule.match_config.capability_policy_all.is_empty());
    assert!(rule.profiles_list.contains(&"rust".to_owned()));
    assert!(rule.profiles_list.contains(&"gerbil-scheme".to_owned()));
    assert!(config.capability_policies.is_empty());
}
