// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_config::HookClientActionKind;
use agent_semantic_config::HookClientCapabilityPolicyConfig;
use agent_semantic_config::HookClientHostInvocationKind;
use agent_semantic_config::HookClientRuleConfig;

#[test]
fn native_matcher_is_the_only_public_rule_host_axis() {
    let rule = toml::from_str::<HookClientRuleConfig>(
        r#"
id = "mcp-read"
decision = "deny"
matcher = "mcp__filesystem__read_file"
"#,
    )
    .expect("parse native matcher rule DSL");
    assert_eq!(rule.matcher.as_deref(), Some("mcp__filesystem__read_file"));

    let duplicate_host_axis = toml::from_str::<HookClientRuleConfig>(
        r#"
id = "duplicate-mcp-read"
decision = "deny"
matcher = "mcp__filesystem__read_file"
hostInvocations = ["mcp"]
"#,
    );
    assert!(
        duplicate_host_axis.is_err(),
        "hostInvocations must not duplicate the native matcher"
    );
}

#[test]
fn capability_policy_separates_host_and_semantic_axes() {
    let policy = toml::from_str::<HookClientCapabilityPolicyConfig>(
        r#"
id = "raw-host-search"
hostInvocationAny = ["execute"]
semanticCapabilityAny = ["search"]
"#,
    )
    .expect("parse split capability policy axes");
    assert_eq!(
        policy.host_invocation_any,
        [HookClientHostInvocationKind::Execute]
    );
    assert_eq!(
        policy.semantic_capability_any,
        [HookClientActionKind::Search]
    );

    let legacy = toml::from_str::<HookClientCapabilityPolicyConfig>(
        r#"
id = "legacy-conflated-action"
actionAny = ["execute"]
"#,
    );
    assert!(legacy.is_err(), "legacy actionAny must fail closed");
}

#[test]
fn hook_config_schema_exposes_only_the_public_rule_axes() {
    let schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../schemas/semantic-agent-hook-client-config.v1.schema.json"
    ))
    .expect("parse hook config schema");
    let rule_properties = schema["$defs"]["rule"]["properties"]
        .as_object()
        .expect("rule properties");
    for public_axis in ["matcher", "profilesList", "matcherPolicies"] {
        assert!(
            rule_properties.contains_key(public_axis),
            "missing public rule axis {public_axis}"
        );
    }

    let match_properties = schema["$defs"]["ruleMatch"]["properties"]
        .as_object()
        .expect("rule match properties");
    assert!(!match_properties.contains_key("actionAny"));
    assert!(!match_properties.contains_key("hostInvocationAny"));

    let capability_policy_properties = schema["$defs"]["capabilityPolicy"]["properties"]
        .as_object()
        .expect("capability policy properties");
    assert!(capability_policy_properties.contains_key("hostInvocationAny"));
    assert!(capability_policy_properties.contains_key("semanticCapabilityAny"));
    assert!(!capability_policy_properties.contains_key("actionAny"));
}

#[test]
fn language_route_materializes_default_wrapped_read_action_in_internal_ir() {
    let mut config = agent_semantic_config::default_hook_client_config_file()
        .expect("parse canonical hook config");
    let public_rule = config
        .rules
        .iter_mut()
        .find(|rule| rule.id == "route-read-to-asp-languages")
        .expect("language route rule");
    assert_eq!(public_rule.matcher.as_deref(), Some("Bash"));
    assert!(public_rule.matcher_policies.is_empty());
    assert_eq!(
        public_rule.profiles_list,
        ["rust", "typescript", "python", "julia", "gerbil-scheme"]
    );
    config
        .materialize_profile_rule_ir()
        .expect("compile profile rule IR");
    let compiled_rule = config
        .rules
        .iter()
        .find(|rule| rule.id == "route-read-to-asp-languages")
        .expect("compiled language route rule");
    assert_eq!(
        compiled_rule.match_config.native_matcher_any,
        ["Bash"],
        "the Bash read route declares only the Bash Host action"
    );
    assert!(compiled_rule.matcher_policies.is_empty());
    assert!(compiled_rule.match_config.host_invocation_any.is_empty());
    assert_eq!(
        compiled_rule.actions,
        [agent_semantic_config::HookClientActionKind::Read]
    );
    assert!(compiled_rule.match_config.capability_policy_all.is_empty());
    assert!(
        compiled_rule
            .match_config
            .profile_extension_any
            .iter()
            .any(|extension| extension == "rs")
    );
}

#[test]
fn unknown_profile_reference_fails_closed() {
    let mut config = agent_semantic_config::default_hook_client_config_file()
        .expect("parse canonical hook config");
    config
        .rules
        .iter_mut()
        .find(|rule| rule.id == "route-read-to-asp-languages")
        .expect("language route rule")
        .profiles_list
        .push("missing-language".to_owned());
    let error = config
        .validate()
        .expect_err("unknown profile must fail closed");
    assert!(error.contains("unknown profile"), "error={error}");
}

#[test]
fn matched_language_dispatch_rejects_profiles_without_provider_routes() {
    let mut config = agent_semantic_config::default_hook_client_config_file()
        .expect("parse canonical hook config");
    config
        .rules
        .iter_mut()
        .find(|rule| rule.id == "route-read-to-asp-languages")
        .expect("language route rule")
        .profiles_list
        .push("markdown".to_owned());
    let error = config
        .validate()
        .expect_err("unregistered lazy provider must fail closed");
    assert!(
        error.contains("selects unregistered lazy provider md/asp-md"),
        "error={error}"
    );
}

#[test]
fn unknown_repository_command_set_reference_fails_closed() {
    let mut config = agent_semantic_config::default_hook_client_config_file()
        .expect("parse canonical hook config");
    config
        .rules
        .iter_mut()
        .find(|rule| rule.id == "git-history-inspection-dispatch")
        .expect("Git history dispatch")
        .match_config
        .command_set_any
        .push("missing-command-set".to_owned());
    let error = config
        .validate()
        .expect_err("unknown command set must fail closed");
    assert!(error.contains("missing command set"), "error={error}");
}

#[test]
fn cross_provider_extension_collision_fails_closed() {
    let mut config = agent_semantic_config::default_hook_client_config_file()
        .expect("parse canonical hook config");
    config
        .profiles
        .get_mut("typescript")
        .expect("TypeScript profile")
        .extension_any
        .push("rs".to_owned());
    let error = config
        .validate()
        .expect_err("cross-provider extension collision must fail closed");
    assert!(error.contains("maps extension"), "error={error}");
}
