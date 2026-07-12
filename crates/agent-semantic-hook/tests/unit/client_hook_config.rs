use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use agent_semantic_hook::{
    ClientHookConfig, DecisionKind, HookClassificationRequest, classify_hook_with_config,
    load_client_config, load_client_config_for_project, render_platform_response,
};
use serde_json::json;

use crate::classifier::registry;

#[test]
fn argv_source_glob_rule_matches_source_argument_after_flags() {
    let root = temp_root("argv-source-glob");
    let config_path = root.join("config.toml");
    fs::write(
        &config_path,
        r#"
schemaId = "agent.semantic-protocols.hook.client-config"
schemaVersion = "1"
protocolId = "agent.semantic-protocols.hook"
protocolVersion = "1"

[[rules]]
id = "deny-wl-source-argv"
decision = "deny"
message = "matched configured argv source"

[rules.match]
tool = "Bash"
commandAny = ["wl"]
argvSourceGlobAny = ["*.ts"]
argvSourceExcludeFlagAny = ["--output"]
"#,
    )
    .expect("write config");
    let config = load_client_config(&config_path).expect("load client config");
    let registry = registry();

    let decision = classify_hook_with_config(HookClassificationRequest {
        registry: &registry,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &json!({
            "tool_name": "Bash",
            "tool_input": {"command": "wl --flag2 flag3 *.ts"}
        }),
    });

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(
        decision
            .fields
            .get("configRuleId")
            .and_then(|id| id.as_str()),
        Some("deny-wl-source-argv")
    );

    for command in [
        "wl --flag2 flag3 README",
        "wl --output *.ts README",
        "wl --output=*.ts README",
    ] {
        let decision = classify_hook_with_config(HookClassificationRequest {
            registry: &registry,
            config: &config,
            platform: "codex",
            event: "pre-tool",
            payload: &json!({
                "tool_name": "Bash",
                "tool_input": {"command": command}
            }),
        });

        assert_eq!(decision.decision, DecisionKind::Allow, "{command}");
    }

    let decision = classify_hook_with_config(HookClassificationRequest {
        registry: &registry,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &json!({
            "tool_name": "Bash",
            "tool_input": {"command": "wl --output ignored.txt source.ts"}
        }),
    });

    assert_eq!(decision.decision, DecisionKind::Deny);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn builtin_source_argv_rule_matches_command_names_not_harness_subcommands() {
    let root = temp_root("builtin-source-argv-command-name");
    let config = ClientHookConfig::default();
    let registry = registry();

    let asp_rg_decision = classify_hook_with_config(HookClassificationRequest {
        registry: &registry,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &json!({
            "tool_name": "Bash",
            "tool_input": {"command": "asp rg -query 'HookDecision' src/cli/agent-hooks.ts"}
        }),
    });

    assert_eq!(asp_rg_decision.decision, DecisionKind::Allow);

    let direct_rg_decision = classify_hook_with_config(HookClassificationRequest {
        registry: &registry,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &json!({
            "session_id": "session-ABC_123",
            "tool_name": "Bash",
            "tool_input": {"command": "rg HookDecision src/cli/agent-hooks.ts"}
        }),
    });

    assert_eq!(direct_rg_decision.decision, DecisionKind::Deny);
    assert_eq!(
        direct_rg_decision
            .fields
            .get("configRuleId")
            .and_then(|id| id.as_str()),
        None
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn deny_decision_mentions_agent_org_artifact_entry_when_inactive() {
    let root = temp_root("agent-org-artifacts-inactive");
    let config_path = root.join("config.toml");
    fs::write(&config_path, agent_org_artifacts_config(&root, true)).expect("write config");
    let config = load_client_config(&config_path).expect("load client config");
    let mut registry = registry();
    registry.project_root = root.display().to_string();

    let decision = classify_hook_with_config(HookClassificationRequest {
        registry: &registry,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &json!({
            "tool_name": "Bash",
            "tool_input": {"command": "rg HookDecision crates/agent-semantic-hook/src/hook_config.rs"},
            "session_id": "session-ABC_123"
        }),
    });

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert!(decision.message.contains("ASP Org Artifact Entry:"));
    let entry_skill_path = org_state_skill_path(&root);
    assert!(decision.message.contains(&format!(
        "Read @{} before continuing",
        entry_skill_path.display()
    )));
    let artifacts_path = org_artifacts_root(&root);
    assert!(
        decision
            .message
            .contains(&artifacts_path.display().to_string()),
        "{}",
        decision.message
    );
    assert_eq!(
        decision
            .fields
            .get("agentOrgArtifactsStatus")
            .and_then(|status| status.as_str()),
        Some("missing-contract-bound-artifact")
    );
    assert_eq!(
        decision
            .fields
            .get("agentOrgArtifactsPath")
            .and_then(|path| path.as_str()),
        Some(artifacts_path.to_str().expect("utf8 artifacts path"))
    );
    assert_eq!(
        decision
            .fields
            .get("agentOrgArtifactsEntrySkillPath")
            .and_then(|path| path.as_str()),
        Some(entry_skill_path.to_str().expect("utf8 entry skill path"))
    );
    let plans_path = artifacts_path.join("flow").join("plans");
    let expected_capture_command_prefix = format!(
        "asp org capture --contract agent.plan.v1 --title 'Agent session plan' --target-file {}/agent-plan-session-abc-123-",
        plans_path.display()
    );
    let expected_capture_command_suffix = ".org --no-confirm";
    let capture_command = decision
        .fields
        .get("agentOrgCaptureContractCommand")
        .and_then(|command| command.as_str())
        .expect("capture command field");
    assert!(
        capture_command.starts_with(&expected_capture_command_prefix),
        "{capture_command}"
    );
    assert!(
        capture_command.ends_with(expected_capture_command_suffix),
        "{capture_command}"
    );
    assert!(
        decision.message.contains(capture_command),
        "{}",
        decision.message
    );
    assert!(
        !capture_command.contains("current-agent-task.org"),
        "{capture_command}"
    );
    assert!(
        !capture_command.contains("agent.task.v1"),
        "{capture_command}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn deny_decision_mentions_agent_org_artifact_entry_by_default() {
    let root = temp_root("agent-org-artifacts-default-enabled");
    let config_path = root.join("config.toml");
    fs::write(&config_path, agent_org_artifacts_default_config(&root)).expect("write config");
    let config = load_client_config(&config_path).expect("load client config");
    let mut registry = registry();
    registry.project_root = root.display().to_string();

    let decision = classify_hook_with_config(HookClassificationRequest {
        registry: &registry,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &json!({
            "tool_name": "Bash",
            "tool_input": {"command": "rg HookDecision crates/agent-semantic-hook/src/hook_config.rs"}
        }),
    });

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert!(decision.message.contains("ASP Org Artifact Entry:"));
    assert_eq!(
        decision
            .fields
            .get("agentOrgArtifactsStatus")
            .and_then(|status| status.as_str()),
        Some("missing-contract-bound-artifact")
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn deny_decision_mentions_agent_org_artifact_entry_when_recent_org_has_no_contract_binding() {
    let root = temp_root("agent-org-artifacts-simple-org-not-active");
    let artifact = org_artifacts_root(&root)
        .join("flow")
        .join("plans")
        .join("current.org");
    fs::create_dir_all(artifact.parent().expect("artifact parent")).expect("artifact dir");
    fs::write(
        &artifact,
        "* Plan\n** Goal\nDo the work.\n** Checklist\n- [ ] One item\n** Evidence\n| Command | Result |\n",
    )
    .expect("write simple org artifact");
    let config_path = root.join("config.toml");
    fs::write(&config_path, agent_org_artifacts_config(&root, true)).expect("write config");
    let config = load_client_config(&config_path).expect("load client config");
    let mut registry = registry();
    registry.project_root = root.display().to_string();

    let decision = classify_hook_with_config(HookClassificationRequest {
        registry: &registry,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &json!({
            "tool_name": "Bash",
            "tool_input": {"command": "rg HookDecision crates/agent-semantic-hook/src/hook_config.rs"}
        }),
    });

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert!(decision.message.contains("ASP Org Artifact Entry:"));
    assert_eq!(
        decision
            .fields
            .get("agentOrgArtifactsStatus")
            .and_then(|status| status.as_str()),
        Some("missing-contract-bound-artifact")
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn deny_decision_skips_agent_org_artifact_entry_when_recent_contract_bound_org_exists() {
    let root = temp_root("agent-org-artifacts-contract-bound-active");
    let artifact = org_artifacts_root(&root)
        .join("flow")
        .join("plans")
        .join("current.org");
    fs::create_dir_all(artifact.parent().expect("artifact parent")).expect("artifact dir");
    fs::write(&artifact, contract_bound_org("Current")).expect("write contract-bound org artifact");
    let config_path = root.join("config.toml");
    fs::write(&config_path, agent_org_artifacts_config(&root, true)).expect("write config");
    let config = load_client_config(&config_path).expect("load client config");
    let mut registry = registry();
    registry.project_root = root.display().to_string();

    let decision = classify_hook_with_config(HookClassificationRequest {
        registry: &registry,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &json!({
            "tool_name": "Bash",
            "tool_input": {"command": "rg HookDecision crates/agent-semantic-hook/src/hook_config.rs"}
        }),
    });

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert!(!decision.message.contains("ASP Org Artifact Entry:"));
    assert!(!decision.fields.contains_key("agentOrgArtifactsStatus"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn deny_decision_mentions_agent_org_artifact_entry_when_recent_org_archive_exists() {
    let root = temp_root("agent-org-artifacts-org-archive-not-active");
    let artifact = org_artifacts_root(&root)
        .join("flow")
        .join("plans")
        .join("current.org_archive");
    fs::create_dir_all(artifact.parent().expect("artifact parent")).expect("artifact dir");
    fs::write(&artifact, "* Current\n").expect("write org_archive artifact");
    let config_path = root.join("config.toml");
    fs::write(&config_path, agent_org_artifacts_config(&root, true)).expect("write config");
    let config = load_client_config(&config_path).expect("load client config");
    let mut registry = registry();
    registry.project_root = root.display().to_string();

    let decision = classify_hook_with_config(HookClassificationRequest {
        registry: &registry,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &json!({
            "tool_name": "Bash",
            "tool_input": {"command": "rg HookDecision crates/agent-semantic-hook/src/hook_config.rs"}
        }),
    });

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert!(decision.message.contains("ASP Org Artifact Entry:"));
    assert_eq!(
        decision
            .fields
            .get("agentOrgArtifactsStatus")
            .and_then(|status| status.as_str()),
        Some("missing-contract-bound-artifact")
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn deny_decision_ignores_recent_org_archive_under_archive_dir() {
    let root = temp_root("agent-org-artifacts-archive-dir-ignored");
    let artifact = org_artifacts_root(&root)
        .join("archive")
        .join("current.org_archive");
    fs::create_dir_all(artifact.parent().expect("artifact parent")).expect("artifact dir");
    fs::write(&artifact, "* Archived\n").expect("write archived org artifact");
    let config_path = root.join("config.toml");
    fs::write(&config_path, agent_org_artifacts_config(&root, true)).expect("write config");
    let config = load_client_config(&config_path).expect("load client config");
    let mut registry = registry();
    registry.project_root = root.display().to_string();

    let decision = classify_hook_with_config(HookClassificationRequest {
        registry: &registry,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &json!({
            "tool_name": "Bash",
            "tool_input": {"command": "rg HookDecision crates/agent-semantic-hook/src/hook_config.rs"}
        }),
    });

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert!(decision.message.contains("ASP Org Artifact Entry:"));
    assert_eq!(
        decision
            .fields
            .get("agentOrgArtifactsStatus")
            .and_then(|status| status.as_str()),
        Some("missing-contract-bound-artifact")
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn deny_decision_skips_agent_org_artifact_entry_when_disabled() {
    let root = temp_root("agent-org-artifacts-disabled");
    let config_path = root.join("config.toml");
    fs::write(&config_path, agent_org_artifacts_config(&root, false)).expect("write config");
    let config = load_client_config(&config_path).expect("load client config");
    let mut registry = registry();
    registry.project_root = root.display().to_string();

    let decision = classify_hook_with_config(HookClassificationRequest {
        registry: &registry,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &json!({
            "tool_name": "Bash",
            "tool_input": {"command": "rg HookDecision crates/agent-semantic-hook/src/hook_config.rs"}
        }),
    });

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert!(!decision.message.contains("ASP Org Artifact Entry:"));
    assert!(!decision.fields.contains_key("agentOrgArtifactsStatus"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn deny_decision_warns_when_done_org_artifacts_should_be_archived() {
    let root = temp_root("agent-org-artifacts-archive-warning");
    write_org_artifact_set(&root, 10, &["flow/plans/done-01.org"]);
    let config_path = root.join("config.toml");
    fs::write(&config_path, agent_org_artifacts_config(&root, true)).expect("write config");
    let config = load_client_config(&config_path).expect("load client config");
    let mut registry = registry();
    registry.project_root = root.display().to_string();

    let decision = classify_hook_with_config(HookClassificationRequest {
        registry: &registry,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &json!({
            "tool_name": "Bash",
            "tool_input": {"command": "rg HookDecision crates/agent-semantic-hook/src/hook_config.rs"}
        }),
    });

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert!(
        !decision.message.contains("ASP Org Archive Warning:"),
        "{}",
        decision.message
    );
    assert_eq!(
        decision
            .fields
            .get("agentOrgArtifactsArchiveWarning")
            .and_then(|status| status.as_str()),
        Some("unarchived-done")
    );
    assert_eq!(
        decision
            .fields
            .get("agentOrgArtifactsActiveOrgFileCount")
            .and_then(|count| count.as_u64()),
        Some(11)
    );
    let artifacts_path = org_artifacts_root(&root);
    let expected_recall_command = format!(
        "asp org recall plans --artifacts-root {} --archive-dir archives",
        artifacts_path.display()
    );
    let expected_command = format!(
        "asp org query --kind task --field todo=DONE --exclude-dir archives --workspace {} --content",
        artifacts_path.display()
    );
    let expected_archive_command = format!(
        "asp org archive done --artifacts-root {} --archive-dir archives",
        artifacts_path.display()
    );
    assert_eq!(
        decision
            .fields
            .get("agentOrgArtifactsPath")
            .and_then(|path| path.as_str()),
        Some(artifacts_path.to_str().expect("utf8 artifacts path"))
    );
    assert_eq!(
        decision
            .fields
            .get("agentOrgArtifactsArchiveQueryCommand")
            .and_then(|command| command.as_str()),
        Some(expected_command.as_str())
    );
    assert_eq!(
        decision
            .fields
            .get("agentOrgArtifactsRecallPlansCommand")
            .and_then(|command| command.as_str()),
        Some(expected_recall_command.as_str())
    );
    assert_eq!(
        decision
            .fields
            .get("agentOrgArtifactsArchiveCommand")
            .and_then(|command| command.as_str()),
        Some(expected_archive_command.as_str())
    );
    let files = decision
        .fields
        .get("agentOrgArtifactsUnarchivedDoneFiles")
        .and_then(|value| value.as_array())
        .expect("done files");
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].as_str(), Some("flow/plans/done-01.org"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn allow_decision_renders_agent_org_archive_warning() {
    let root = temp_root("agent-org-artifacts-archive-warning-allow");
    write_org_artifact_set(&root, 10, &["flow/plans/done-01.org"]);
    let config_path = root.join("config.toml");
    fs::write(&config_path, agent_org_artifacts_config(&root, true)).expect("write config");
    let config = load_client_config(&config_path).expect("load client config");
    let mut registry = registry();
    registry.project_root = root.display().to_string();

    let decision = classify_hook_with_config(HookClassificationRequest {
        registry: &registry,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &json!({
            "tool_name": "Bash",
            "tool_input": {"command": "true"}
        }),
    });

    assert_eq!(decision.decision, DecisionKind::Allow);
    assert!(!decision.message.contains("ASP Org Archive Warning:"));
    assert_eq!(
        decision
            .fields
            .get("agentOrgArtifactsArchiveWarning")
            .and_then(|status| status.as_str()),
        Some("unarchived-done")
    );
    let rendered = render_platform_response(&decision).expect("render decision");
    assert!(
        rendered
            .get("systemMessage")
            .and_then(|message| message.as_str())
            .is_none_or(|message| !message.contains("ASP Org Archive Warning:")),
        "{rendered}"
    );

    let session_start = classify_hook_with_config(HookClassificationRequest {
        registry: &registry,
        config: &config,
        platform: "codex",
        event: "session-start",
        payload: &json!({}),
    });

    assert_eq!(session_start.decision, DecisionKind::Allow);
    assert!(session_start.message.contains("ASP Org Archive Warning:"));
    let rendered_session_start =
        render_platform_response(&session_start).expect("render session-start decision");
    assert!(
        rendered_session_start
            .get("systemMessage")
            .and_then(|message| message.as_str())
            .is_some_and(|message| message.contains("ASP Org Archive Warning:")),
        "{rendered_session_start}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn deny_decision_skips_archive_warning_below_threshold_or_inside_archives() {
    let root = temp_root("agent-org-artifacts-archive-warning-skipped");
    write_org_artifact_set(&root, 9, &["flow/plans/done-01.org"]);
    let archived = org_artifacts_root(&root)
        .join("archives")
        .join("flow")
        .join("plans")
        .join("done-archived.org");
    fs::create_dir_all(archived.parent().expect("archived parent")).expect("archive dir");
    fs::write(&archived, "* DONE Archived\n").expect("write archived done");
    let config_path = root.join("config.toml");
    fs::write(&config_path, agent_org_artifacts_config(&root, true)).expect("write config");
    let config = load_client_config(&config_path).expect("load client config");
    let mut registry = registry();
    registry.project_root = root.display().to_string();

    let decision = classify_hook_with_config(HookClassificationRequest {
        registry: &registry,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &json!({
            "tool_name": "Bash",
            "tool_input": {"command": "rg HookDecision crates/agent-semantic-hook/src/hook_config.rs"}
        }),
    });

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert!(!decision.message.contains("ASP Org Archive Warning:"));
    assert!(
        !decision
            .fields
            .contains_key("agentOrgArtifactsArchiveWarning")
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn client_config_disables_agent_org_archive_warning() {
    let root = temp_root("client-config-disables-agent-org-archive-warning");
    write_org_artifact_set(&root, 10, &["flow/plans/done-01.org"]);
    let config_path = root.join("config.toml");
    let mut config_text = agent_org_artifacts_config(&root, true);
    config_text.push_str(
        r#"
[agentOrgArtifacts.archiveWarning]
enabled = false
"#,
    );
    fs::write(&config_path, config_text).expect("write config");
    let config = load_client_config_for_project(&config_path, &root).expect("load client config");
    let mut registry = registry();
    registry.project_root = root.display().to_string();

    let decision = classify_hook_with_config(HookClassificationRequest {
        registry: &registry,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &json!({
            "tool_name": "Bash",
            "tool_input": {"command": "rg HookDecision crates/agent-semantic-hook/src/hook_config.rs"}
        }),
    });

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert!(!decision.message.contains("ASP Org Archive Warning:"));
    assert!(
        !decision
            .fields
            .contains_key("agentOrgArtifactsArchiveWarning")
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn client_config_disables_agent_org_artifact_recovery() {
    let root = temp_root("client-config-disables-agent-org-artifacts");
    let config_path = root.join("config.toml");
    fs::write(&config_path, agent_org_artifacts_config(&root, false)).expect("write config");
    let config = load_client_config_for_project(&config_path, &root).expect("load client config");
    let mut registry = registry();
    registry.project_root = root.display().to_string();

    let decision = classify_hook_with_config(HookClassificationRequest {
        registry: &registry,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &json!({
            "tool_name": "Bash",
            "tool_input": {"command": "rg HookDecision crates/agent-semantic-hook/src/hook_config.rs"}
        }),
    });

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert!(!decision.message.contains("ASP Org Artifact Entry:"));
    assert!(!decision.fields.contains_key("agentOrgArtifactsStatus"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn project_config_rejects_agent_org_artifacts_overlay() {
    let root = temp_root("project-config-rejects-agent-org-artifacts-overlay");
    let config_path = root.join("config.toml");
    fs::write(&config_path, agent_org_artifacts_default_config(&root)).expect("write config");
    let agent_config_path = root.join(".agents").join("asp.toml");
    fs::create_dir_all(agent_config_path.parent().expect("agent config parent"))
        .expect("agent config dir");
    fs::write(
        &agent_config_path,
        r#"
[hook.agentOrgArtifacts]
enabled = false
"#,
    )
    .expect("write agent config");

    let err = load_client_config_for_project(&config_path, &root).expect_err("reject agent config");
    assert!(err.contains("agentOrgArtifacts"), "{err}");
    assert!(err.contains("unknown field"), "{err}");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn command_contains_any_rejects_empty_patterns() {
    let root = temp_root("command-contains-empty-pattern");
    let config_path = root.join("config.toml");
    fs::write(
        &config_path,
        r#"
schemaId = "agent.semantic-protocols.hook.client-config"
schemaVersion = "1"
protocolId = "agent.semantic-protocols.hook"
protocolVersion = "1"

[[rules]]
id = "deny-empty-command-contains"
decision = "deny"

[rules.match]
tool = "Bash"
commandContainsAny = [""]
"#,
    )
    .expect("write config");

    let error = load_client_config(&config_path).expect_err("reject empty commandContainsAny");
    assert!(
        error.contains("rules[].match.commandContainsAny[] must not be empty"),
        "{error}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn command_contains_any_matches_ascii_case_insensitively() {
    let root = temp_root("command-contains-case-insensitive");
    let config_path = root.join("config.toml");
    fs::write(
        &config_path,
        r#"
schemaId = "agent.semantic-protocols.hook.client-config"
schemaVersion = "1"
protocolId = "agent.semantic-protocols.hook"
protocolVersion = "1"

[[rules]]
id = "deny-case-insensitive-command-contains"
decision = "deny"

[rules.match]
tool = "Bash"
commandContainsAny = ["HOOKDECISION"]
"#,
    )
    .expect("write config");
    let config = load_client_config(&config_path).expect("load client config");
    let registry = registry();

    let decision = classify_hook_with_config(HookClassificationRequest {
        registry: &registry,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &json!({
            "tool_name": "Bash",
            "tool_input": {
                "command": "rg hookdecision src/cli/agent-hooks.ts"
            }
        }),
    });

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(
        decision
            .fields
            .get("configRuleId")
            .and_then(|id| id.as_str()),
        Some("deny-case-insensitive-command-contains")
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn argv_prefix_any_matches_a_nested_command_stage_without_matching_nearby_forms() {
    let root = temp_root("argv-prefix");
    let config_path = root.join("config.toml");
    fs::write(
        &config_path,
        r#"
schemaId = "agent.semantic-protocols.hook.client-config"
schemaVersion = "1"
protocolId = "agent.semantic-protocols.hook"
protocolVersion = "1"

[[rules]]
id = "deny-recursive-force-remove"
decision = "deny"

[rules.match]
tool = "Bash"
argvPrefixAny = [["rm", "-rf"]]
"#,
    )
    .expect("write config");
    let config = load_client_config(&config_path).expect("load client config");
    let registry = registry();

    let denied = classify_hook_with_config(HookClassificationRequest {
        registry: &registry,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &json!({
            "tool_name": "Bash",
            "tool_input": {
                "command": "printf warmup && rm -rf ./generated"
            }
        }),
    });
    assert_eq!(denied.decision, DecisionKind::Deny);
    assert_eq!(
        denied.fields.get("configRuleId").and_then(|id| id.as_str()),
        Some("deny-recursive-force-remove")
    );

    let allowed = classify_hook_with_config(HookClassificationRequest {
        registry: &registry,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &json!({
            "tool_name": "Bash",
            "tool_input": {
                "command": "rm -r ./generated"
            }
        }),
    });
    assert_eq!(allowed.decision, DecisionKind::Allow);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn argv_prefix_any_rejects_empty_patterns() {
    let root = temp_root("argv-prefix-empty");
    let config_path = root.join("config.toml");
    fs::write(
        &config_path,
        r#"
schemaId = "agent.semantic-protocols.hook.client-config"
schemaVersion = "1"
protocolId = "agent.semantic-protocols.hook"
protocolVersion = "1"

[[rules]]
id = "invalid-empty-prefix"
decision = "deny"

[rules.match]
argvPrefixAny = [[]]
"#,
    )
    .expect("write config");

    let error = load_client_config(&config_path).expect_err("empty prefix must be rejected");
    assert!(
        error.contains("rules[].match.argvPrefixAny[0] must not be empty"),
        "{error}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn configurable_hook_default_rule_classification_stays_fast() {
    let root = temp_root("default-source-argv-perf");
    let config = ClientHookConfig::default();
    let registry = registry();
    let payloads = [
        json!({
            "tool_name": "Bash",
            "tool_input": {"command": "rg HookDecision src/cli/agent-hooks.ts"}
        }),
        json!({
            "tool_name": "Bash",
            "tool_input": {"command": "sed -n '1,40p' src/cli/agent-hooks.ts"}
        }),
        json!({
            "tool_name": "Bash",
            "tool_input": {"command": "wl --output src/cli/agent-hooks.ts README.md"}
        }),
        json!({
            "tool_name": "Bash",
            "tool_input": {"command": "asp rg -query 'HookDecision' src/cli/agent-hooks.ts"}
        }),
    ];
    let samples = 4;
    let iterations = 20_000;
    let mut best_elapsed = Duration::MAX;
    let mut best_denied = 0usize;

    for _ in 0..samples {
        let start = Instant::now();
        let mut denied = 0usize;
        for index in 0..iterations {
            let decision = classify_hook_with_config(HookClassificationRequest {
                registry: &registry,
                config: &config,
                platform: "codex",
                event: "pre-tool",
                payload: &payloads[index % payloads.len()],
            });
            if decision.decision == DecisionKind::Deny {
                denied += 1;
            }
        }
        let elapsed = start.elapsed();
        if elapsed < best_elapsed {
            best_elapsed = elapsed;
            best_denied = denied;
        }
    }

    let per_decision = best_elapsed.as_nanos() / iterations as u128;
    eprintln!(
        "configurable_hook_default_rule_perf samples={samples} iterations={iterations} best_elapsed_ms={} best_ns_per_decision={per_decision}",
        best_elapsed.as_millis()
    );

    assert_eq!(best_denied, iterations / 4);
    assert!(
        best_elapsed < Duration::from_millis(5_000),
        "configurable hook classification regressed: {best_elapsed:?} for {iterations} iterations"
    );

    let _ = fs::remove_dir_all(root);
}

fn temp_root(label: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("agent-semantic-hook-{label}-{nonce}"));
    fs::create_dir_all(&root).expect("create temp root");
    canonical(&root)
}

fn canonical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn org_artifacts_root(root: &Path) -> PathBuf {
    state_home(root)
        .join("projects")
        .join("by-id")
        .join("repo-test")
        .join("workspaces")
        .join("workspace-test")
        .join("artifacts")
        .join("org")
}

fn org_state_skill_path(root: &Path) -> PathBuf {
    state_home(root)
        .join("org")
        .join("templates")
        .join("ASP_ORG_SKILL.org")
}

fn state_home(root: &Path) -> PathBuf {
    root.join("home").join(".agent-semantic-protocols")
}

fn write_org_artifact_set(root: &Path, count: usize, done_files: &[&str]) {
    let artifacts_root = org_artifacts_root(root);
    for index in 0..count {
        let path = artifacts_root
            .join("flow")
            .join("plans")
            .join(format!("active-{index:02}.org"));
        fs::create_dir_all(path.parent().expect("artifact parent")).expect("artifact dir");
        fs::write(&path, format!("* TODO Active {index}\n")).expect("write active org");
    }
    for relative in done_files {
        let path = artifacts_root.join(relative);
        fs::create_dir_all(path.parent().expect("done parent")).expect("done dir");
        fs::write(&path, "* DONE Ready to archive\n").expect("write done org");
    }
}

fn contract_bound_org(title: &str) -> String {
    format!(
        r#"* TODO {title} :agent:
:PROPERTIES:
:CONTRACT_ORG: agent.task.v1
:END:
** Goal
Keep recoverable ASP Org state.
** Acceptance
- [X] Hook accepts contract-bound artifacts.
** Progress
- [X] Fixture written.
** Evidence
- cargo test -p agent-semantic-hook client_hook_config
"#
    )
}

fn agent_org_artifacts_config(root: &Path, enabled: bool) -> String {
    let artifacts_path = org_artifacts_root(root);
    let entry_skill_path = org_state_skill_path(root);
    format!(
        r#"
schemaId = "agent.semantic-protocols.hook.client-config"
schemaVersion = "1"
protocolId = "agent.semantic-protocols.hook"
protocolVersion = "1"

[agentOrgArtifacts]
enabled = {enabled}
inactiveAfterMinutes = 30
artifactsPath = "{}"
entrySkillPath = "{}"

[[rules]]
id = "deny-rg"
enabled = true
event = "pre-tool"
priority = 80
decision = "deny"
reasonKind = "bulk-source-dump"
message = "matched configured rg"

[rules.match]
tool = "Bash"
commandAny = ["rg"]
"#,
        artifacts_path.display().to_string().replace('\\', "\\\\"),
        entry_skill_path.display().to_string().replace('\\', "\\\\")
    )
}

fn agent_org_artifacts_default_config(root: &Path) -> String {
    let artifacts_path = org_artifacts_root(root);
    let entry_skill_path = org_state_skill_path(root);
    format!(
        r#"
schemaId = "agent.semantic-protocols.hook.client-config"
schemaVersion = "1"
protocolId = "agent.semantic-protocols.hook"
protocolVersion = "1"

[agentOrgArtifacts]
artifactsPath = "{}"
entrySkillPath = "{}"

[[rules]]
id = "deny-rg"
enabled = true
event = "pre-tool"
priority = 80
decision = "deny"
reasonKind = "bulk-source-dump"
message = "matched configured rg"

[rules.match]
tool = "Bash"
commandAny = ["rg"]
"#,
        artifacts_path.display().to_string().replace('\\', "\\\\"),
        entry_skill_path.display().to_string().replace('\\', "\\\\")
    )
}
