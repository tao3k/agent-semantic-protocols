use agent_semantic_config::{HookClientActionKind, default_hook_client_config_file};

#[test]
fn registered_source_capability_rule_keeps_opaque_shell_access_separate_from_read() {
    let config = default_hook_client_config_file().expect("embedded hook config");
    let rule = config
        .rules
        .iter()
        .find(|rule| rule.id == "deny-raw-registered-source-action")
        .expect("registered source rule");

    assert_eq!(
        rule.match_config.capability_policy_all,
        ["opaque-shell-source-access", "registered-language-source"]
    );
    let opaque_shell_source_access = config
        .capability_policies
        .iter()
        .find(|policy| policy.id == "opaque-shell-source-access")
        .expect("opaque shell source-access policy");
    assert_eq!(
        opaque_shell_source_access.action_any,
        [HookClientActionKind::Execute]
    );
}
