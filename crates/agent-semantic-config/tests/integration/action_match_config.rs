use agent_semantic_config::{HookClientActionKind, HookClientConfigFile, HookClientMatcherPolicy};

#[test]
fn git_source_read_rule_dispatches_to_testing_resident() {
    let config =
        toml::from_str::<HookClientConfigFile>(include_str!("../../templates/hooks/config.toml"))
            .expect("default hook config template should parse");
    let rule = config
        .rules
        .iter()
        .find(|rule| rule.id == "deny-uncontrolled-git-source-reads")
        .expect("git source read rule should exist");
    let dispatch = rule
        .dispatch
        .as_ref()
        .expect("git source read rule should declare a resident dispatch");

    assert_eq!(dispatch.role.as_str(), "testing");
    assert_eq!(dispatch.receipt_kind.as_str(), "asp-testing-execution-v1");
    assert!(
        rule.message
            .as_deref()
            .is_some_and(|message| message.contains("asp session --agents choice-plane"))
    );
}

#[test]
fn source_deny_rules_have_one_explore_role_dispatch_for_the_choice_plane() {
    let config =
        toml::from_str::<HookClientConfigFile>(include_str!("../../templates/hooks/config.toml"))
            .expect("default hook config template should parse");

    for rule_id in [
        "deny-uncontrolled-source-search-commands",
        "deny-uncontrolled-source-materialization-commands",
    ] {
        let rule = config
            .rules
            .iter()
            .find(|rule| rule.id == rule_id)
            .unwrap_or_else(|| panic!("{rule_id} should exist"));
        let dispatch = rule
            .dispatch
            .as_ref()
            .unwrap_or_else(|| panic!("{rule_id} should declare one dispatch"));

        assert_eq!(dispatch.role.as_str(), "explore");
        assert_eq!(dispatch.receipt_kind.as_str(), "asp-explore-search-v1");
    }
}

#[test]
fn default_template_uses_rule_local_matcher_policies() {
    let config =
        toml::from_str::<HookClientConfigFile>(include_str!("../../templates/hooks/config.toml"))
            .expect("default hook config template should parse");

    let wrapped_rule = config
        .rules
        .iter()
        .find(|rule| rule.id == "resident-testing-dispatch")
        .expect("wrapped command rule");
    assert_eq!(
        wrapped_rule.matcher_policies,
        [HookClientMatcherPolicy::WrappedCommand]
    );
    let native_read_rule = config
        .rules
        .iter()
        .find(|rule| rule.id == "route-read-to-asp-languages")
        .expect("native Read route rule");
    assert!(native_read_rule.matcher_policies.is_empty());

    let action_rule = config
        .rules
        .iter()
        .find(|rule| rule.id == "deny-raw-registered-source-action")
        .expect("action-first source deny rule should exist");
    assert_eq!(
        action_rule.match_config.action_policy_all,
        ["raw-shell-read", "registered-language-source"]
    );
    let raw_shell_read = config
        .action_policies
        .iter()
        .find(|policy| policy.id == "raw-shell-read")
        .expect("raw-shell-read action policy");
    assert_eq!(
        raw_shell_read.action_any,
        vec![HookClientActionKind::Execute]
    );
    assert_eq!(raw_shell_read.effect_any, vec![HookClientActionKind::Read]);
    assert!(action_rule.match_config.effect_rules.iter().any(|rule| {
        rule.argv_prefix == ["git", "mv"] && rule.effect == HookClientActionKind::Edit
    }));
    assert!(
        action_rule.match_config.effect_rules.iter().any(|rule| {
            rule.argv_prefix == ["cat"] && rule.effect == HookClientActionKind::Read
        })
    );
}

#[test]
fn matcher_policy_rfc_records_parser_owned_snapshot_contract() {
    let rfc = include_str!(
        "../../../../docs/10-19-rfcs/10.15-agent-hook-interception-protocol/10.15.40-parser-owned-wrapper-match-snapshots.org"
    );

    assert!(rfc.contains("matcherPolicies"));
    assert!(rfc.contains("wrapped_command"));
    assert!(rfc.contains("commandWrappers"));
    assert!(rfc.contains("Git snapshot"));
}

#[test]
fn typed_action_rule_shape_probe() {
    let config =
        toml::from_str::<HookClientConfigFile>(include_str!("../../templates/hooks/config.toml"))
            .expect("default hook config template should parse");
    let rule = config
        .rules
        .iter()
        .find(|rule| rule.id == "deny-raw-registered-source-action")
        .expect("typed action rule should exist");
    eprintln!("{rule:#?}");
}
