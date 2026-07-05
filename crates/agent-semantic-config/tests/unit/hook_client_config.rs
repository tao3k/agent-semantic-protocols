use std::fs;
use std::path::{Path, PathBuf};

use super::{
    CLIENT_HOOK_CONFIG_SCHEMA_ID, HookClientConfigFile, HookClientResidentAgentConfig,
    default_hook_client_config_template, default_hook_client_config_template_for_source_extensions,
    load_asp_project_config_file, load_hook_client_config_file,
};

fn resident_agent<'a>(
    config: &'a HookClientConfigFile,
    name: &str,
) -> &'a HookClientResidentAgentConfig {
    config
        .agents
        .resident_agents
        .iter()
        .find(|agent| agent.name == name)
        .expect("resident agent")
}

#[test]
fn default_template_round_trips_through_config_parser() {
    let root = temp_root("hook-client-template");
    let config_path = root.join("hooks").join("config.toml");
    fs::create_dir_all(config_path.parent().expect("config parent")).expect("config dir");
    fs::write(&config_path, default_hook_client_config_template()).expect("write config");

    let config = load_hook_client_config_file(&config_path).expect("load config");

    assert_eq!(
        config.schema_id.as_deref(),
        Some(CLIENT_HOOK_CONFIG_SCHEMA_ID)
    );
    assert!(config.experimental.is_empty());
    assert!(config.agent_org_artifacts.is_none());
    assert!(config.recovery_prompt.template.is_none());
    assert!(config.recovery_prompt.codex_agent_flow.is_none());
    assert!(config.recovery_prompt.claude_agent_flow.is_none());
    assert!(config.recovery_prompt.default_agent_flow.is_none());
    assert!(
        config
            .agent_session_guide
            .register
            .as_deref()
            .is_some_and(|guide| guide.contains("asp agent session register guide"))
    );
    assert!(
        config
            .agent_session_guide
            .register
            .as_deref()
            .is_some_and(|guide| guide.contains("asp agent session lifecycle audit --json"))
    );
    assert!(
        config
            .agent_session_guide
            .status
            .as_deref()
            .is_some_and(|guide| guide.contains("nextAction=start-resident-child-and-register"))
    );
    assert!(
        config
            .agent_session_messages
            .missing_resident_explore
            .as_deref()
            .is_some_and(|message| message.contains("asp agent session register --guide"))
    );
    assert!(
        config
            .agent_session_messages
            .main_restricted_without_child
            .as_deref()
            .is_some_and(|message| message.contains("Retry the blocked ASP command"))
    );
    let source_access_compact_subagent = config
        .agent_session_messages
        .source_access_compact_subagent
        .as_deref()
        .expect("source access compact subagent message");
    assert!(
        source_access_compact_subagent
            .contains("compact `[asp-search-subagent]` graph-route receipt")
    );
    assert!(source_access_compact_subagent.contains("schema/intent/route/state/evidence/next"));
    assert!(source_access_compact_subagent.contains("Do not return source bodies"));
    let invalid_child_message = config
        .agent_session_messages
        .binary_gate_invalid_child
        .as_deref()
        .expect("binary gate invalid child message");
    assert!(invalid_child_message.contains("validation-warning-or-non-routable-child"));
    assert!(
        invalid_child_message.contains("requiredAction=parent-follow-up-existing-child-with-model")
    );
    assert!(invalid_child_message.contains("parent agent sends a Codex thread follow-up"));
    assert!(invalid_child_message.contains("<requiredModel-from-validationReason>"));
    assert!(
        invalid_child_message.contains("configSwitchPurpose=Use the config switch command only")
    );
    assert!(!invalid_child_message.contains("destroy-invalid-child-and-create-configured-child"));
    let asp_explore = resident_agent(&config, "asp-explore");
    assert!(asp_explore.enabled);
    assert_eq!(asp_explore.name, "asp-explore");
    assert_eq!(asp_explore.codex_agent_name, "asp_explorer");
    assert_eq!(
        asp_explore.main_allowed_asp_command_prefixes,
        [
            "help",
            "--help",
            "-h",
            "agent session",
            "org recall",
            "org capture"
        ]
    );
    let asp_testing = resident_agent(&config, "asp-testing");
    assert!(asp_testing.enabled);
    assert_eq!(asp_testing.name, "asp-testing");
    assert_eq!(asp_testing.codex_agent_name, "asp_testing");
    assert_eq!(
        asp_testing.command_prefixes,
        [
            "cargo test",
            "cargo check",
            "cargo build",
            "pytest",
            "uv run pytest",
            "just test"
        ]
    );
    assert!(config.rules.is_empty());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn client_config_loads_recovery_prompt_template() {
    let root = temp_root("hook-client-recovery-prompt");
    let config_path = root.join("config.toml");
    fs::write(
        &config_path,
        r#"
schemaId = "agent.semantic-protocols.hook.client-config"
schemaVersion = "1"
protocolId = "agent.semantic-protocols.hook"
protocolVersion = "1"

[recoveryPrompt]
template = "reason={reason}\nflow={agent_flow}\nroutes={routes}"
codexAgentFlow = "codex flow from config"
claudeAgentFlow = "claude flow from config"
defaultAgentFlow = "default flow from config"

[agentSessionGuide]
register = "register guide"
list = "list guide"
show = "show guide"
reuse = "reuse guide"

[[agents.residentAgents]]
name = "asp-explore"
role = "asp_explorer"
codexAgentName = "asp_explorer"
lifecycle = "asp-command"
mainAllowedAspCommandPrefixes = ["help", "agent session", "org recall", "org capture"]
"#,
    )
    .expect("write config");

    let config = load_hook_client_config_file(&config_path).expect("load config");

    assert_eq!(
        config.recovery_prompt.template.as_deref(),
        Some("reason={reason}\nflow={agent_flow}\nroutes={routes}")
    );
    assert_eq!(
        config.recovery_prompt.codex_agent_flow.as_deref(),
        Some("codex flow from config")
    );
    assert_eq!(
        config.recovery_prompt.claude_agent_flow.as_deref(),
        Some("claude flow from config")
    );
    assert_eq!(
        config.recovery_prompt.default_agent_flow.as_deref(),
        Some("default flow from config")
    );
    assert_eq!(
        config.agent_session_guide.register.as_deref(),
        Some("register guide")
    );
    assert_eq!(
        config.agent_session_guide.list.as_deref(),
        Some("list guide")
    );
    assert_eq!(
        config.agent_session_guide.show.as_deref(),
        Some("show guide")
    );
    assert_eq!(
        config.agent_session_guide.reuse.as_deref(),
        Some("reuse guide")
    );
    let asp_explore = resident_agent(&config, "asp-explore");
    assert!(asp_explore.enabled);
    assert_eq!(asp_explore.name, "asp-explore");
    assert_eq!(asp_explore.codex_agent_name, "asp_explorer");
    assert_eq!(
        asp_explore.main_allowed_asp_command_prefixes,
        ["help", "agent session", "org recall", "org capture"]
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn client_config_rejects_legacy_flat_subagent_receipt_message() {
    let root = temp_root("hook-client-legacy-subagent-message");
    let config_path = root.join("config.toml");
    fs::write(
        &config_path,
        r#"
schemaId = "agent.semantic-protocols.hook.client-config"
schemaVersion = "1"
protocolId = "agent.semantic-protocols.hook"
protocolVersion = "1"

[agentSessionMessages]
sourceAccessCompactSubagent = "Use ASP query/search routes and return selector-only `[asp-search-subagent]` evidence with owner/read/next."
"#,
    )
    .expect("write config");

    let error = load_hook_client_config_file(&config_path).expect_err("legacy message rejected");

    assert!(
        error.contains("legacy flat subagent receipt contract"),
        "{error}"
    );
    assert!(
        error.contains("schema/intent/route/state/evidence/next"),
        "{error}"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn project_config_rejects_hook_agent_org_artifacts() {
    let root = temp_root("asp-project-config-agent-org-artifacts");
    let config_path = root.join(".agents").join("asp.toml");
    fs::create_dir_all(config_path.parent().expect("config parent")).expect("config dir");
    fs::write(
        &config_path,
        r#"
[skills.agent-semantic-protocols]
template = "SKILL.org"

[hook.agentOrgArtifacts]
enabled = false
inactiveAfterMinutes = 45
artifactsPath = "/tmp/asp-state/projects/by-id/repo-test/workspaces/workspace-test/artifacts/org"
entrySkillPath = "/tmp/asp-state/org/templates/ASP_ORG_SKILL.org"
"#,
    )
    .expect("write asp config");

    let err = load_asp_project_config_file(&config_path).expect_err("reject asp config");
    assert!(err.contains("agentOrgArtifacts"), "{err}");
    assert!(err.contains("unknown field"), "{err}");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn template_source_extensions_do_not_generate_user_rules() {
    let root = temp_root("hook-client-template-extensions");
    let config_path = root.join("hooks").join("config.toml");
    fs::create_dir_all(config_path.parent().expect("config parent")).expect("config dir");
    fs::write(
        &config_path,
        default_hook_client_config_template_for_source_extensions([
            ".ss", "ss", "*.scm", "**/*.sld", "", "  ",
        ]),
    )
    .expect("write config");

    let config = load_hook_client_config_file(&config_path).expect("load config");
    assert!(config.rules.is_empty());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn missing_config_loads_empty_defaults() {
    let root = temp_root("hook-client-missing");
    let config = load_hook_client_config_file(&root.join("missing.toml")).expect("missing config");

    assert!(config.rules.is_empty());
    assert!(config.experimental.is_empty());
    assert!(config.agent_org_artifacts.is_none());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn existing_config_uses_figment_metadata_defaults() {
    let root = temp_root("hook-client-metadata-defaults");
    let config_path = root.join("config.toml");
    fs::write(
        &config_path,
        r#"
[[rules]]
id = "deny-rust-read"
decision = "deny"
"#,
    )
    .expect("write config");

    let config = load_hook_client_config_file(&config_path).expect("load config");

    assert_eq!(
        config.schema_id.as_deref(),
        Some(CLIENT_HOOK_CONFIG_SCHEMA_ID)
    );
    assert_eq!(config.schema_version.as_deref(), Some("1"));
    assert_eq!(
        config.protocol_id.as_deref(),
        Some("agent.semantic-protocols.hook")
    );
    assert_eq!(config.protocol_version.as_deref(), Some("1"));
    assert!(config.agent_org_artifacts.is_none());
    assert_eq!(config.rules.len(), 1);
    assert_eq!(config.rules[0].id, "deny-rust-read");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn agent_org_artifacts_config_requires_state_core_paths_when_block_present() {
    let root = temp_root("hook-client-agent-org-artifacts-defaults");
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

    let error = load_hook_client_config_file(&config_path).expect_err("missing State Core paths");
    assert!(
        error.contains("artifactsPath"),
        "expected artifactsPath error, got {error}"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn agent_org_artifacts_rejects_empty_paths_and_zero_minutes() {
    let root = temp_root("hook-client-agent-org-artifacts-invalid");
    let config_path = root.join("config.toml");
    fs::write(
        &config_path,
        r#"
schemaId = "agent.semantic-protocols.hook.client-config"
schemaVersion = "1"
protocolId = "agent.semantic-protocols.hook"
protocolVersion = "1"

[agentOrgArtifacts]
inactiveAfterMinutes = 0
artifactsPath = ""
entrySkillPath = ""

[agentOrgArtifacts.archiveWarning]
activeOrgFileThreshold = 0
archivesDir = ""
maxReportedFiles = 0
"#,
    )
    .expect("write config");

    let error =
        load_hook_client_config_file(&config_path).expect_err("reject invalid agent org artifacts");
    assert!(
        error.contains("agentOrgArtifacts.inactiveAfterMinutes must be greater than 0"),
        "{error}"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn invalid_route_kind_is_rejected_by_config_layer() {
    let root = temp_root("hook-client-invalid-route");
    let config_path = root.join("config.toml");
    fs::write(
        &config_path,
        r#"
schemaId = "agent.semantic-protocols.hook.client-config"
schemaVersion = "1"
protocolId = "agent.semantic-protocols.hook"
protocolVersion = "1"

[[rules]]
id = "deny-rust-read"
decision = "deny"

[[rules.routes]]
providerId = "rs-harness"
kind = "route-text"
argv = ["asp", "rust"]
"#,
    )
    .expect("write config");

    let error = load_hook_client_config_file(&config_path).expect_err("invalid route kind");

    assert!(error.contains("route-text"), "{error}");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn argv_source_match_fields_round_trip_through_config_parser() {
    let root = temp_root("hook-client-argv-source");
    let config_path = root.join("config.toml");
    fs::write(
        &config_path,
        r#"
schemaId = "agent.semantic-protocols.hook.client-config"
schemaVersion = "1"
protocolId = "agent.semantic-protocols.hook"
protocolVersion = "1"

[[rules]]
id = "deny-argv-source"
decision = "deny"

[rules.match]
commandAny = ["wl"]
argvSourceAny = ["src/main.ts"]
argvSourceGlobAny = ["*.ts"]
argvSourceExcludeFlagAny = ["--output"]
"#,
    )
    .expect("write config");

    let config = load_hook_client_config_file(&config_path).expect("load config");
    let rule = config.rules.first().expect("config rule");

    assert_eq!(rule.match_config.argv_source_any, ["src/main.ts"]);
    assert_eq!(rule.match_config.argv_source_glob_any, ["*.ts"]);
    assert_eq!(rule.match_config.argv_source_exclude_flag_any, ["--output"]);
    let _ = fs::remove_dir_all(root);
}

fn temp_root(label: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("agent-semantic-config-{label}-{nonce}"));
    fs::create_dir_all(&root).expect("create temp root");
    canonical(&root)
}

fn canonical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}
