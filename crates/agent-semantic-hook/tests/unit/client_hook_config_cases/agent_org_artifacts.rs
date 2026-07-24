use super::common::{
    DecisionKind, HookClassificationRequest, agent_org_artifacts_config,
    agent_org_artifacts_default_config, classify_hook_with_config, contract_bound_org, fs, json,
    load_client_config, load_client_config_for_project, org_artifacts_root, org_state_skill_path,
    registry, render_platform_response, temp_root, write_org_artifact_set,
};

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
