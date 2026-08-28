use agent_semantic_config::default_hook_client_config_file;

#[test]
fn registered_source_rule_is_bash_unknown_access_plus_language_profiles() {
    let config = default_hook_client_config_file().expect("embedded hook config");
    let rule = config
        .rules
        .iter()
        .find(|rule| rule.id == "route-unresolved-source-access-to-asp-languages")
        .expect("registered source rule");
    assert_eq!(rule.matcher.as_deref(), Some("Bash"));
    assert_eq!(
        rule.match_config.capability_policy_all,
        ["registered-source-access"]
    );
    assert!(rule.profiles_list.contains(&"rust".to_owned()));
    assert!(rule.profiles_list.contains(&"gerbil-scheme".to_owned()));
    assert!(
        config
            .capability_policies
            .iter()
            .all(|policy| policy.id != "opaque-registered-source-operation")
    );
}
