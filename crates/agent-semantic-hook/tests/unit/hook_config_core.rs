use super::{HookClientRuleMatchConfig, RuleMatch};
use crate::tool_action::{OperationIntent, ToolAction, ToolSurface};

fn configured_projection_rule(
    document_format: agent_semantic_config::HookClientStructuredFormat,
    optional_subcommand_any: Vec<String>,
) -> RuleMatch {
    RuleMatch::try_from(HookClientRuleMatchConfig {
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
    let resident_agents = config.agents.resident_agents;

    let error = match super::CompiledHookRule::try_from_with_agents(rule, &resident_agents) {
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
    assert!(rule.matches_structured_projection(&shell_action("sh .package.name package.json")));
    for command in [
        "sh . package.json",
        "sh '..' package.json",
        "sh '.items[]' package.json",
        "sh .package.name package.json second.json",
        "sh .package.name package.json | sed -n 1p package.json",
    ] {
        assert!(
            !rule.matches_structured_projection(&shell_action(command)),
            "unexpected bounded projector match: {command}"
        );
    }
}

#[test]
fn workspace_regular_file_matching_does_not_use_language_extensions() {
    let rule = RuleMatch::try_from(HookClientRuleMatchConfig {
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
fn optional_subcommand_is_configured_for_toml_projection() {
    let rule = configured_projection_rule(
        agent_semantic_config::HookClientStructuredFormat::Toml,
        vec!["inspect".to_string()],
    );
    assert!(
        rule.matches_structured_projection(&shell_action(
            "sh inspect .workspace.members Cargo.toml"
        ))
    );
    assert!(!rule.matches_structured_projection(&shell_action("sh . Cargo.toml")));
}
