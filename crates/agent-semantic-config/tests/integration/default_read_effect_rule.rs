use agent_semantic_config::{
    HookClientActionKind, WrapperMatchMode, default_hook_client_config_file,
};

#[test]
fn default_registered_source_rule_infers_normalized_inner_read_through_effect_rule() {
    let config = default_hook_client_config_file().expect("embedded hook config");
    let rule = config
        .rules
        .iter()
        .find(|rule| rule.id == "deny-raw-registered-source-action")
        .expect("registered source rule");

    assert_eq!(config.wrapper_match, WrapperMatchMode::Enable);
    assert!(
        rule.match_config.effect_rules.iter().any(|rule| {
            rule.argv_prefix == ["read"] && rule.effect == HookClientActionKind::Read
        })
    );
    assert_eq!(rule.match_config.effect_any, [HookClientActionKind::Read]);
}
