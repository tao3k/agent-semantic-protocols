use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use agent_semantic_hook::{
    DecisionKind, HookClassificationRequest, classify_hook_with_config,
    default_client_config_template, load_client_config, load_client_config_for_project,
    render_platform_response,
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
fn default_source_argv_rule_matches_command_names_not_harness_subcommands() {
    let root = temp_root("default-source-argv-command-name");
    let config_path = root.join("config.toml");
    fs::write(&config_path, default_client_config_template()).expect("write config");
    let config = load_client_config(&config_path).expect("load client config");
    let registry = registry();

    let asp_rg_decision = classify_hook_with_config(HookClassificationRequest {
        registry: &registry,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &json!({
            "tool_name": "Bash",
            "tool_input": {"command": "asp rg -query 'HookDecision' crates/agent-semantic-hook/src/hook_config.rs"}
        }),
    });

    assert_eq!(asp_rg_decision.decision, DecisionKind::Allow);

    let direct_rg_decision = classify_hook_with_config(HookClassificationRequest {
        registry: &registry,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &json!({
            "tool_name": "Bash",
            "tool_input": {"command": "rg HookDecision crates/agent-semantic-hook/src/hook_config.rs"}
        }),
    });

    assert_eq!(direct_rg_decision.decision, DecisionKind::Deny);
    assert_eq!(
        direct_rg_decision
            .fields
            .get("configRuleId")
            .and_then(|id| id.as_str()),
        Some("deny-shell-source-argv")
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn deny_decision_mentions_agent_org_artifact_entry_when_inactive() {
    let root = temp_root("agent-org-artifacts-inactive");
    let config_path = root.join("config.toml");
    fs::write(&config_path, agent_org_artifacts_config(true)).expect("write config");
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
    assert!(
        decision.message.contains(
            "Read @.cache/agent-semantic-protocol/org/skills/ASP_ORG.org before continuing"
        )
    );
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
        Some(".cache/agent-semantic-protocol/org/skills/ASP_ORG.org")
    );
    let sample_task_path = artifacts_path.join("current-agent-task.org");
    let expected_capture_command = format!(
        "asp org capture --contract agent.task.v1 --title 'Current agent task' --target-file {} --no-confirm",
        sample_task_path.display()
    );
    assert_eq!(
        decision
            .fields
            .get("agentOrgCaptureContractCommand")
            .and_then(|command| command.as_str()),
        Some(expected_capture_command.as_str())
    );
    assert!(
        decision.message.contains(&expected_capture_command),
        "{}",
        decision.message
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn deny_decision_mentions_agent_org_artifact_entry_by_default() {
    let root = temp_root("agent-org-artifacts-default-enabled");
    let config_path = root.join("config.toml");
    fs::write(&config_path, agent_org_artifacts_default_config()).expect("write config");
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
    let artifact = root
        .join(".cache")
        .join("agent-semantic-protocol")
        .join("artifacts")
        .join("org")
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
    fs::write(&config_path, agent_org_artifacts_config(true)).expect("write config");
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
    let artifact = root
        .join(".cache")
        .join("agent-semantic-protocol")
        .join("artifacts")
        .join("org")
        .join("flow")
        .join("plans")
        .join("current.org");
    fs::create_dir_all(artifact.parent().expect("artifact parent")).expect("artifact dir");
    fs::write(&artifact, contract_bound_org("Current")).expect("write contract-bound org artifact");
    let config_path = root.join("config.toml");
    fs::write(&config_path, agent_org_artifacts_config(true)).expect("write config");
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
    let artifact = root
        .join(".cache")
        .join("agent-semantic-protocol")
        .join("artifacts")
        .join("org")
        .join("flow")
        .join("plans")
        .join("current.org_archive");
    fs::create_dir_all(artifact.parent().expect("artifact parent")).expect("artifact dir");
    fs::write(&artifact, "* Current\n").expect("write org_archive artifact");
    let config_path = root.join("config.toml");
    fs::write(&config_path, agent_org_artifacts_config(true)).expect("write config");
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
    let artifact = root
        .join(".cache")
        .join("agent-semantic-protocol")
        .join("artifacts")
        .join("org")
        .join("archive")
        .join("current.org_archive");
    fs::create_dir_all(artifact.parent().expect("artifact parent")).expect("artifact dir");
    fs::write(&artifact, "* Archived\n").expect("write archived org artifact");
    let config_path = root.join("config.toml");
    fs::write(&config_path, agent_org_artifacts_config(true)).expect("write config");
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
    fs::write(&config_path, agent_org_artifacts_config(false)).expect("write config");
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
    fs::write(&config_path, agent_org_artifacts_config(true)).expect("write config");
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
        decision.message.contains("ASP Org Archive Warning:"),
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
    let expected_command = format!(
        "asp org query --kind task --field todo=DONE --exclude-dir archives --workspace {} --content",
        artifacts_path.display()
    );
    assert!(
        decision.message.contains(&expected_command),
        "{}",
        decision.message
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
    fs::write(
        &config_path,
        r#"
schemaId = "agent.semantic-protocols.hook.client-config"
schemaVersion = "1"
protocolId = "agent.semantic-protocols.hook"
protocolVersion = "1"

[agentOrgArtifacts]
enabled = true
"#,
    )
    .expect("write config");
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
    assert!(decision.message.contains("ASP Org Archive Warning:"));
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
            .is_some_and(|message| message.contains("ASP Org Archive Warning:")),
        "{rendered}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn deny_decision_skips_archive_warning_below_threshold_or_inside_archives() {
    let root = temp_root("agent-org-artifacts-archive-warning-skipped");
    write_org_artifact_set(&root, 9, &["flow/plans/done-01.org"]);
    let archived = root
        .join(".cache")
        .join("agent-semantic-protocol")
        .join("artifacts")
        .join("org")
        .join("archives")
        .join("flow")
        .join("plans")
        .join("done-archived.org");
    fs::create_dir_all(archived.parent().expect("archived parent")).expect("archive dir");
    fs::write(&archived, "* DONE Archived\n").expect("write archived done");
    let config_path = root.join("config.toml");
    fs::write(&config_path, agent_org_artifacts_config(true)).expect("write config");
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
fn agent_config_disables_agent_org_archive_warning() {
    let root = temp_root("agent-config-disables-agent-org-archive-warning");
    write_org_artifact_set(&root, 10, &["flow/plans/done-01.org"]);
    let config_path = root.join("config.toml");
    fs::write(&config_path, agent_org_artifacts_config(true)).expect("write config");
    let agent_config_path = root.join(".agents").join("asp.toml");
    fs::create_dir_all(agent_config_path.parent().expect("agent config parent"))
        .expect("agent config dir");
    fs::write(
        &agent_config_path,
        r#"
[hook.agentOrgArtifacts.archiveWarning]
enabled = false
"#,
    )
    .expect("write agent config");
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
fn agent_config_disables_agent_org_artifact_recovery() {
    let root = temp_root("agent-config-disables-agent-org-artifacts");
    let config_path = root.join("config.toml");
    fs::write(&config_path, agent_org_artifacts_default_config()).expect("write config");
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
                "command": "rg hookdecision crates/agent-semantic-hook/src/hook_config.rs"
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
fn configurable_hook_default_rule_classification_stays_fast() {
    let root = temp_root("default-source-argv-perf");
    let config_path = root.join("config.toml");
    fs::write(&config_path, default_client_config_template()).expect("write config");
    let config = load_client_config(&config_path).expect("load client config");
    let registry = registry();
    let payloads = [
        json!({
            "tool_name": "Bash",
            "tool_input": {"command": "rg HookDecision crates/agent-semantic-hook/src/hook_config.rs"}
        }),
        json!({
            "tool_name": "Bash",
            "tool_input": {"command": "sed -n '1,40p' crates/agent-semantic-hook/src/hook_config.rs"}
        }),
        json!({
            "tool_name": "Bash",
            "tool_input": {"command": "wl --output crates/agent-semantic-hook/src/hook_config.rs README.md"}
        }),
        json!({
            "tool_name": "Bash",
            "tool_input": {"command": "asp rg -query 'HookDecision' crates/agent-semantic-hook/src/hook_config.rs"}
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

    assert_eq!(best_denied, iterations * 3 / 4);
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
    root.join(".cache")
        .join("agent-semantic-protocol")
        .join("artifacts")
        .join("org")
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

fn agent_org_artifacts_config(enabled: bool) -> String {
    format!(
        r#"
schemaId = "agent.semantic-protocols.hook.client-config"
schemaVersion = "1"
protocolId = "agent.semantic-protocols.hook"
protocolVersion = "1"

[agentOrgArtifacts]
enabled = {enabled}
inactiveAfterMinutes = 30
artifactsPath = ".cache/agent-semantic-protocol/artifacts/org"
entrySkillPath = ".cache/agent-semantic-protocol/org/skills/ASP_ORG.org"

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
"#
    )
}

fn agent_org_artifacts_default_config() -> &'static str {
    r#"
schemaId = "agent.semantic-protocols.hook.client-config"
schemaVersion = "1"
protocolId = "agent.semantic-protocols.hook"
protocolVersion = "1"

[agentOrgArtifacts]

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
"#
}
