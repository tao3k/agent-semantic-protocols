use agent_semantic_config::{HookClientActionKind, HookClientConfigFile, WrapperMatchMode};

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

    assert_eq!(dispatch.agent.as_str(), "testing");
    assert_eq!(dispatch.receipt_kind.as_str(), "asp-testing-execution-v1");
    assert!(
        rule.message
            .as_deref()
            .is_some_and(|message| message.contains("ASP Testing"))
    );
}

#[test]
fn default_template_uses_one_top_level_wrapper_match_mode() {
    let config =
        toml::from_str::<HookClientConfigFile>(include_str!("../../templates/hooks/config.toml"))
            .expect("default hook config template should parse");

    let action_rule = config
        .rules
        .iter()
        .find(|rule| rule.id == "deny-raw-registered-source-action")
        .expect("action-first source deny rule should exist");
    assert_eq!(config.wrapper_match, WrapperMatchMode::Enable);
    assert_eq!(
        action_rule.match_config.action_any,
        vec![HookClientActionKind::Execute]
    );
    assert_eq!(
        action_rule.match_config.effect_any,
        vec![HookClientActionKind::Read]
    );
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
fn wrapper_match_rfc_records_parser_owned_snapshot_contract() {
    let rfc = include_str!(
        "../../../../docs/10-19-rfcs/10.15-agent-hook-interception-protocol/10.15.40-parser-owned-wrapper-match-snapshots.org"
    );

    assert!(rfc.contains("wrapper_match"));
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
