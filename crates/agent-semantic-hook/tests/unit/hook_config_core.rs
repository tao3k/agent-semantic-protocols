use super::compiled_rule::RuleMatch;
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
                max_slice_items: 64,
                optional_subcommand_any,
                option_any: Vec::new(),
                option_value_arity: std::collections::BTreeMap::new(),
            },
        ),
        ..Default::default()
    })
    .expect("compile configured projector rule")
}

fn shell_action(command: &str) -> ToolAction {
    ToolAction {
        tool_name: "exec_command".to_string(),
        host_payload: serde_json::json!({ "command": command }),
        invocation_source: None,
        host_action: crate::action_ir::HostInvocationKind::Unknown,
        surface: ToolSurface::CodexShell,
        operation: OperationIntent::ShellCommand,
        command: Some(command.to_string()),
        command_tokens: None,
        leading_shell_stage: true,
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
fn managed_template_serializes_declarative_language_profiles() {
    let template = crate::hook_config::default_client_config_template();
    let parsed: toml::Value = toml::from_str(&template).expect("managed hook config TOML");
    let profiles = parsed
        .get("profiles")
        .and_then(toml::Value::as_table)
        .expect("declarative language profiles");
    for (language_id, provider_id, extension) in [
        ("rust", "asp-rust", "rs"),
        ("typescript", "asp-typescript", "ts"),
        ("python", "asp-python", "py"),
        ("julia", "asp-julia", "jl"),
        ("gerbil-scheme", "asp-gerbil-scheme", "ss"),
    ] {
        let profile = profiles
            .get(language_id)
            .unwrap_or_else(|| panic!("language profile {language_id}"));
        assert_eq!(
            profile.get("providerId").and_then(toml::Value::as_str),
            Some(provider_id)
        );
        assert!(
            profile
                .get("extensionAny")
                .and_then(toml::Value::as_array)
                .is_some_and(|extensions| extensions
                    .iter()
                    .any(|candidate| { candidate.as_str() == Some(extension) }))
        );
    }
}
