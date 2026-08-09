use super::compiled_rule::{CompiledHookRule, RuleMatch};
use crate::tool_action::{OperationIntent, ToolAction, ToolSurface};

fn configured_projection_rule(
    document_format: agent_semantic_config::HookClientStructuredFormat,
    optional_subcommand_any: Vec<String>,
) -> RuleMatch {
    RuleMatch::try_from(agent_semantic_config::HookClientRuleMatchConfig {
        structured_projection: Some(
            agent_semantic_config::HookClientStructuredProjectionMatchConfig {
                binary: "sh".to_string(),
                document_format,
                filter_grammar:
                    agent_semantic_config::HookClientStructuredFilterGrammar::BoundedPathV1,
                optional_subcommand_any,
                option_any: Vec::new(),
                option_value_arity: std::collections::BTreeMap::new(),
            },
        ),
        ..Default::default()
    })
    .expect("compile configured projector rule")
}

#[test]
fn source_expansion_rule_rejects_non_read_effect_contract() {
    let mut config = toml::from_str::<agent_semantic_config::HookClientConfigFile>(
        &crate::hook_config::default_client_config_template(),
    )
    .expect("default hook config should parse");
    let rule_index = config
        .rules
        .iter()
        .position(|rule| rule.id == "deny-raw-registered-source-action")
        .expect("source deny rule should exist");
    let mut rule = config.rules.remove(rule_index);
    rule.match_config.effect_any = vec![agent_semantic_config::HookClientActionKind::Edit];
    let agents = config.agents;

    let error = match CompiledHookRule::try_from_with_agents(
        rule,
        &agents,
        &[],
        &config.action_policies,
        agent_semantic_config::WrapperMatchMode::Enable,
    ) {
        Ok(_) => panic!("source expansion must require a typed read effect"),
        Err(error) => error,
    };
    assert!(
        error.contains("typed read effect"),
        "unexpected compile error: {error}"
    );
}

fn shell_action(command: &str) -> ToolAction {
    ToolAction {
        tool_name: "exec_command".to_string(),
        surface: ToolSurface::CodexShell,
        operation: OperationIntent::ShellCommand,
        command: Some(command.to_string()),
        command_tokens: None,
        paths: Vec::new(),
    }
}

#[test]
fn bounded_projection_model_comes_from_config_and_is_fail_closed() {
    let rule = configured_projection_rule(
        agent_semantic_config::HookClientStructuredFormat::Json,
        Vec::new(),
    );
    assert_eq!(
        rule.structured_projection_source_operands(&shell_action("sh .package.name package.json")),
        Ok(Some(vec!["package.json".to_string()]))
    );
    for command in [
        "sh . package.json",
        "sh '..' package.json",
        "sh '.items[]' package.json",
        "sh .package.name package.json second.json",
        "sh .package.name package.json | sed -n 1p package.json",
    ] {
        assert!(
            rule.structured_projection_source_operands(&shell_action(command))
                .is_err(),
            "unexpected bounded projector match: {command}"
        );
    }
}

#[test]
fn workspace_regular_file_matching_does_not_use_language_extensions() {
    let rule = RuleMatch::try_from(agent_semantic_config::HookClientRuleMatchConfig {
        argv_workspace_regular_file: true,
        ..Default::default()
    })
    .expect("compile workspace file rule");
    let root =
        std::env::temp_dir().join(format!("asp-workspace-regular-file-{}", std::process::id()));
    std::fs::create_dir_all(&root).expect("create workspace");
    std::fs::write(root.join("Cargo.toml"), "[workspace]\n").expect("write TOML fixture");
    assert!(rule.matches_argv_source_path(&root, "Cargo.toml"));
    assert!(!rule.matches_argv_source_path(&root, "missing.toml"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn structured_document_file_matching_excludes_provider_language_sources() {
    let rule = RuleMatch::try_from(agent_semantic_config::HookClientRuleMatchConfig {
        argv_structured_document_file: true,
        ..Default::default()
    })
    .expect("compile structured document rule");
    let root = std::env::temp_dir().join(format!(
        "asp-structured-document-file-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("create workspace");
    std::fs::write(root.join("package.json"), "{}\n").expect("write JSON fixture");
    std::fs::write(root.join("provider.rs"), "fn provider() {}\n").expect("write Rust fixture");
    assert!(rule.matches_argv_source_path(&root, "package.json"));
    assert!(!rule.matches_argv_source_path(&root, "provider.rs"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn optional_subcommand_is_configured_for_toml_projection() {
    let rule = configured_projection_rule(
        agent_semantic_config::HookClientStructuredFormat::Toml,
        vec!["inspect".to_string()],
    );
    assert_eq!(
        rule.structured_projection_source_operands(&shell_action(
            "sh inspect .workspace.members Cargo.toml"
        )),
        Ok(Some(vec!["Cargo.toml".to_string()]))
    );
    assert!(
        rule.structured_projection_source_operands(&shell_action("sh . Cargo.toml"))
            .is_err()
    );
}
#[test]
fn managed_template_serializes_provider_owned_language_extensions() {
    let template = crate::hook_config::default_client_config_template();
    let parsed: toml::Value = toml::from_str(&template).expect("managed hook config TOML");
    let providers = parsed
        .get("languageProviders")
        .and_then(toml::Value::as_array)
        .expect("languageProviders projection");
    for (language_id, provider_id, extension) in [
        ("rust", "rs-harness", ".rs"),
        ("typescript", "ts-harness", ".ts"),
        ("python", "py-harness", ".py"),
        ("julia", "julia-lang-project-harness", ".jl"),
        ("gerbil-scheme", "gerbil-scheme-harness", ".ss"),
    ] {
        let provider = providers
            .iter()
            .find(|provider| {
                provider.get("languageId").and_then(toml::Value::as_str) == Some(language_id)
                    && provider.get("providerId").and_then(toml::Value::as_str) == Some(provider_id)
            })
            .unwrap_or_else(|| panic!("provider projection {language_id}/{provider_id}"));
        assert!(
            provider
                .get("manifestDigest")
                .and_then(toml::Value::as_str)
                .is_some_and(|digest| digest.starts_with("sha256:"))
        );
        assert!(
            provider
                .get("sourceExtensions")
                .and_then(toml::Value::as_array)
                .is_some_and(|extensions| extensions
                    .iter()
                    .any(|candidate| { candidate.as_str() == Some(extension) }))
        );
    }
}
