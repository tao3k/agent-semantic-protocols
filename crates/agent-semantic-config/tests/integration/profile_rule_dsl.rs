use agent_semantic_config::{
    HookClientActionKind, HookClientDecisionMaterializer, HookClientLanguageProviderConfig,
};

#[test]
fn language_route_is_public_dsl_and_materializes_only_in_internal_ir() {
    let mut config = agent_semantic_config::default_hook_client_config_file()
        .expect("parse canonical hook config");
    config
        .language_providers
        .push(HookClientLanguageProviderConfig {
            language_id: "rust".to_owned(),
            provider_id: "asp-rust".to_owned(),
            manifest_digest: "test-rust-provider".to_owned(),
            source_extensions: vec![".rs".to_owned()],
        });
    let public_rule = config
        .rules
        .iter()
        .find(|rule| rule.id == "route-read-to-asp-languages")
        .expect("language route rule");
    assert_eq!(public_rule.actions, [HookClientActionKind::Read]);
    assert_eq!(
        public_rule.profiles_list,
        ["rust", "typescript", "python", "julia", "gerbil-scheme"]
    );
    assert_eq!(public_rule.decision_materializer, None);

    config
        .materialize_profile_rule_ir()
        .expect("compile profile rule IR");
    let compiled_rule = config
        .rules
        .iter()
        .find(|rule| rule.id == "route-read-to-asp-languages")
        .expect("compiled language route rule");
    assert_eq!(
        compiled_rule.decision_materializer,
        Some(HookClientDecisionMaterializer::SourceAccess)
    );
    assert!(
        compiled_rule
            .match_config
            .action_any
            .contains(&HookClientActionKind::Read)
    );
    assert!(compiled_rule.match_config.action_policy_all.is_empty());
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
