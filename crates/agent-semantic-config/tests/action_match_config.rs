use agent_semantic_config::{HookClientActionKind, HookClientConfigFile};

#[test]
fn git_source_read_rule_dispatches_to_testing_resident() {
    let config =
        toml::from_str::<HookClientConfigFile>(include_str!("../templates/hooks/config.toml"))
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

    assert_eq!(dispatch.resident_name, "asp-testing");
    assert_eq!(dispatch.receipt_kind, "asp-testing-execution-v1");
    assert!(
        rule.message
            .as_deref()
            .is_some_and(|message| message.contains("ASP Testing"))
    );
}

#[test]
fn default_template_parses_typed_action_and_wrapper_match_fields() {
    let config =
        toml::from_str::<HookClientConfigFile>(include_str!("../templates/hooks/config.toml"))
            .expect("default hook config template should parse");

    let action_rule = config
        .rules
        .iter()
        .find(|rule| rule.id == "deny-raw-registered-source-action")
        .expect("action-first source deny rule should exist");
    assert_eq!(action_rule.match_config.command_wrappers.len(), 1);
    assert_eq!(action_rule.match_config.invocation_shape_any.len(), 3);
    assert_eq!(action_rule.match_config.wrapper_match_any.len(), 3);
    assert_eq!(action_rule.match_config.flag_presence_any.len(), 2);
    assert_eq!(
        action_rule.match_config.action_any,
        vec![HookClientActionKind::Read, HookClientActionKind::Execute]
    );
    assert_eq!(
        action_rule.match_config.effect_any,
        vec![HookClientActionKind::Read]
    );
    assert!(
        action_rule
            .match_config
            .effect_projections
            .iter()
            .any(|projection| {
                projection.argv_prefix == ["git", "mv"]
                    && projection.effect == HookClientActionKind::Edit
            })
    );
    assert!(
        action_rule
            .match_config
            .effect_projections
            .iter()
            .any(|projection| {
                projection.argv_prefix == ["cat"] && projection.effect == HookClientActionKind::Read
            })
    );
}

#[test]
fn invocation_rfc_records_bash_producer_and_hook_consumer_ownership() {
    let rfc = include_str!(
        "../../../docs/10-19-rfcs/10.05-cli-first-harness-ux/10.05.10-search-query-surface/09-invocation-shape-and-wrapper-match.org"
    );

    assert!(rfc.contains("The Bash parser crate owns shell syntax"));
    assert!(rfc.contains("The hook crate MUST consume those facts"));
    assert!(rfc.contains("Rust matcher code MUST NOT contain wrapper names"));
    assert!(rfc.contains("~effect=read|unknown~"));
    assert!(rfc.contains("~effect=edit~ MUST NOT"));
}

#[test]
fn typed_action_rule_shape_probe() {
    let config =
        toml::from_str::<HookClientConfigFile>(include_str!("../templates/hooks/config.toml"))
            .expect("default hook config template should parse");
    let rule = config
        .rules
        .iter()
        .find(|rule| rule.id == "deny-raw-registered-source-action")
        .expect("typed action rule should exist");
    eprintln!("{rule:#?}");
}
