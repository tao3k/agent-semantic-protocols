use agent_semantic_config::{HookClientActionKind, default_hook_client_config_file};

#[test]
fn default_registered_source_rule_projects_normalized_inner_read() {
    let config = default_hook_client_config_file().expect("embedded hook config");
    let rule = config
        .rules
        .iter()
        .find(|rule| rule.id == "deny-raw-registered-source-action")
        .expect("registered source rule");

    assert!(
        rule.match_config
            .command_wrappers
            .iter()
            .any(|wrapper| wrapper.executable == "rtk")
    );
    assert!(
        rule.match_config
            .effect_projections
            .iter()
            .any(|projection| {
                projection.argv_prefix == ["read"]
                    && projection.effect == HookClientActionKind::Read
            })
    );
    assert_eq!(rule.match_config.effect_any, [HookClientActionKind::Read]);
}
