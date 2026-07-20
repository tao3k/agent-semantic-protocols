use std::fs;
use std::path::{Path, PathBuf};

use agent_semantic_config::{
    CLIENT_HOOK_CONFIG_SCHEMA_ID, HookClientConfigFile, HookClientDecisionMaterializer,
    HookClientExecutionTransport, HookClientResidentAgentConfig,
    default_hook_client_config_template, default_hook_client_config_template_for_source_extensions,
    hook_client_contract_fingerprint, load_asp_project_config_file, load_hook_client_config_file,
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
fn default_template_round_trips_with_third_lint_resident() {
    let root = temp_root("hook-client-template-third-resident");
    let config_path = root.join("hooks").join("config.toml");
    fs::create_dir_all(config_path.parent().expect("config parent")).expect("config dir");

    let fixture = format!(
        r#"{template}

[[agents.residentAgents]]
enabled = true
name = "asp-lint"
role = "asp_lint"
roles = ["subagent", "lint"]
permissions = ["workspace-write"]
codexAgentName = "asp_lint"
lifecycle = "lint-command"
sessionLifetime = "resident"
mainAllowedAspCommandPrefixes = []
commandPrefixes = ["cargo clippy", "cargo fmt", "ruff check"]
"#,
        template = default_hook_client_config_template(),
    );
    fs::write(&config_path, fixture).expect("write config");

    let config = load_hook_client_config_file(&config_path).expect("load config");

    assert_eq!(
        config.schema_id.as_deref(),
        Some(CLIENT_HOOK_CONFIG_SCHEMA_ID)
    );
    assert_eq!(
        config.contract_fingerprint.as_deref(),
        Some(hook_client_contract_fingerprint().as_str())
    );
    assert_eq!(config.agents.resident_agents.len(), 3);

    let asp_explore = resident_agent(&config, "asp-explore");
    assert_eq!(asp_explore.codex_agent_name, "asp_explorer");

    let asp_testing = resident_agent(&config, "asp-testing");
    assert_eq!(asp_testing.codex_agent_name, "asp_testing");

    let asp_lint = resident_agent(&config, "asp-lint");
    assert!(asp_lint.enabled);
    assert_eq!(asp_lint.name, "asp-lint");
    assert_eq!(asp_lint.role, "asp_lint");
    assert_eq!(asp_lint.codex_agent_name, "asp_lint");
    assert_eq!(asp_lint.lifecycle, "lint-command");
    assert_eq!(asp_lint.session_lifetime, "resident");
    assert!(asp_lint.main_allowed_asp_command_prefixes.is_empty());
    assert_eq!(
        asp_lint.command_prefixes,
        ["cargo clippy", "cargo fmt", "ruff check"]
    );

    assert_ne!(asp_explore.name, asp_lint.name);
    assert_ne!(asp_testing.name, asp_lint.name);

    let _ = fs::remove_dir_all(root);
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
    assert_eq!(
        config.contract_fingerprint.as_deref(),
        Some(hook_client_contract_fingerprint().as_str())
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
            .register()
            .as_deref()
            .is_some_and(|guide| guide.contains("asp agent session register guide"))
    );
    assert!(
        config
            .agent_session_guide
            .register()
            .as_deref()
            .is_some_and(|guide| guide.contains("asp agent session bootstrap"))
    );
    assert!(
        config.agent_session_guide.status().as_deref().is_some_and(
            |guide| guide.contains("bootstrap --name <residentChildName-from-hook-decision>")
        )
    );
    assert!(config.agent_session_guide.reuse().is_none());
    assert!(
        config
            .agent_session_messages
            .missing_resident_explore
            .as_deref()
            .is_some_and(|message| message.contains("choose one number"))
    );
    assert!(
        config
            .agent_session_messages
            .missing_resident_explore
            .as_deref()
            .is_some_and(|message| !message.contains("register --guide"))
    );
    assert!(
        config
            .agent_session_messages
            .missing_resident_explore
            .as_deref()
            .is_some_and(|message| !message.contains("agent session reuse"))
    );
    assert!(
        config
            .agent_session_messages
            .main_restricted_without_child
            .as_deref()
            .is_some_and(|message| message.contains("send the blocked ASP command"))
    );
    assert!(
        config
            .agent_session_messages
            .source_access_compact_subagent
            .is_none()
    );
    let invalid_child_message = config
        .agent_session_messages
        .binary_gate_invalid_child
        .as_deref()
        .expect("binary gate invalid child message");
    assert!(invalid_child_message.contains("validation-warning-or-non-routable-child"));
    let legacy_close_delete = ["close", "/", "delete"].concat();
    assert!(!invalid_child_message.contains(&legacy_close_delete));
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
    assert_eq!(asp_testing.codex_agent_name, "asp_testing");
    assert_eq!(config.agents.resident_agents.len(), 2);
    let testing_lane = config
        .execution_lanes
        .lanes
        .get("testing")
        .expect("testing execution lane");
    assert!(testing_lane.enabled);
    assert_eq!(
        testing_lane.transport,
        HookClientExecutionTransport::ResidentAgent
    );
    assert_eq!(testing_lane.resident_name, "asp-testing");
    assert_eq!(testing_lane.receipt_kind, "asp-testing-execution-v1");
    assert_eq!(
        testing_lane.command_prefixes,
        [
            "cargo test",
            "cargo check",
            "cargo build",
            "pytest",
            "uv run pytest",
            "just test"
        ]
    );
    assert_eq!(config.rules.len(), 6);
    assert_eq!(
        config
            .rules
            .iter()
            .map(|rule| rule.id.as_str())
            .collect::<Vec<_>>(),
        [
            "deny-agent-search-json",
            "materialize-prompt-search-strategy",
            "materialize-apply-patch-policy",
            "materialize-source-access-policy",
            "deny-uncontrolled-source-search-commands",
            "deny-uncontrolled-git-source-reads",
        ]
    );
    let prompt_strategy_rule = config
        .rules
        .iter()
        .find(|rule| rule.id == "materialize-prompt-search-strategy")
        .expect("prompt search strategy rule");
    assert!(
        matches!(
            prompt_strategy_rule.decision_materializer,
            Some(HookClientDecisionMaterializer::PromptSearchStrategy)
        ),
        "prompt strategy materializer: {:?}",
        prompt_strategy_rule.decision_materializer
    );
    assert_eq!(
        config.asp_command_intent_policy.control_plane.root_commands,
        [
            "guide",
            "providers",
            "tools",
            "wrap",
            "cache",
            "cloud",
            "hook",
            "agent",
            "install",
            "sync",
            "paths",
            "healthcheck",
            "source-access",
            "ast-patch",
            "graph",
        ]
    );
    assert!(
        config
            .asp_command_intent_policy
            .reasoning
            .root_commands
            .is_empty()
    );
    assert_eq!(
        config.asp_command_intent_policy.reasoning.search_routes,
        [
            "prime",
            "pipe",
            "owner",
            "lexical",
            "deps",
            "dependency",
            "failure",
            "reasoning",
            "ingest",
            "guide",
        ]
    );
    assert_eq!(
        config.asp_command_intent_policy.reasoning.query_flags,
        ["--term"]
    );
    assert_eq!(
        config
            .asp_command_intent_policy
            .exact_evidence
            .selector_kinds,
        ["item"]
    );
    assert!(
        config
            .asp_command_intent_policy
            .invalid_evidence
            .reject_cross_language_selector
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn default_template_contains_no_legacy_asp_facade_rule() {
    let value: toml::Value =
        toml::from_str(&default_hook_client_config_template()).expect("parse default TOML");
    let rules = value
        .get("rules")
        .and_then(toml::Value::as_array)
        .expect("default rules");
    let legacy = rules.iter().find(|rule| {
        rule.get("id").and_then(toml::Value::as_str) == Some("deny-invalid-asp-facade")
            || rule
                .get("decisionMaterializer")
                .and_then(toml::Value::as_str)
                == Some("invalid-asp-facade")
    });
    assert!(
        legacy.is_none(),
        "legacy ASP facade rule remains: {legacy:#?}"
    );
}

#[test]
fn legacy_invalid_asp_facade_materializer_is_rejected() {
    let root = temp_root("legacy-invalid-asp-facade-materializer");
    let config_path = root.join("hooks/config.toml");
    fs::create_dir_all(config_path.parent().expect("config parent")).expect("config dir");
    let legacy_rule = r#"
[[rules]]
id = "legacy-invalid-asp-facade"
priority = 1
decision = "deny"
decisionMaterializer = "invalid-asp-facade"
message = "legacy"
"#;
    fs::write(
        &config_path,
        format!("{}{}", default_hook_client_config_template(), legacy_rule),
    )
    .expect("write legacy config");

    let error = load_hook_client_config_file(&config_path).expect_err("legacy value must fail");
    assert!(
        error.contains("invalid-asp-facade"),
        "unexpected error: {error}"
    );
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
        config.agent_session_guide.register().as_deref(),
        Some("register guide")
    );
    assert_eq!(
        config.agent_session_guide.list().as_deref(),
        Some("list guide")
    );
    assert_eq!(
        config.agent_session_guide.show().as_deref(),
        Some("show guide")
    );
    assert_eq!(
        config.agent_session_guide.reuse().as_deref(),
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
    let rendered = default_hook_client_config_template_for_source_extensions([
        ".ss", "ss", "*.scm", "**/*.sld", "", "  ",
    ]);
    fs::write(&config_path, rendered).expect("write config");

    let config = load_hook_client_config_file(&config_path).expect("load config");
    assert_eq!(
        config
            .rules
            .iter()
            .map(|rule| rule.id.as_str())
            .collect::<Vec<_>>(),
        [
            "deny-agent-search-json",
            "materialize-prompt-search-strategy",
            "materialize-apply-patch-policy",
            "materialize-source-access-policy",
            "deny-uncontrolled-source-search-commands",
            "deny-uncontrolled-git-source-reads",
        ]
    );
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
    assert!(config.contract_fingerprint.is_none());
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
fn invalid_decision_materializer_is_rejected_by_config_layer() {
    let root = temp_root("hook-client-invalid-materializer");
    let config_path = root.join("config.toml");
    fs::write(
        &config_path,
        r#"
schemaId = "agent.semantic-protocols.hook.client-config"
schemaVersion = "1"
protocolId = "agent.semantic-protocols.hook"
protocolVersion = "1"

[[rules]]
id = "deny-source-access"
decision = "deny"
decisionMaterializer = "legacy-source-classifier"
"#,
    )
    .expect("write config");

    let error = load_hook_client_config_file(&config_path).expect_err("invalid materializer");

    assert!(error.contains("legacy-source-classifier"), "{error}");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn decision_materializer_cannot_compete_with_static_routes() {
    let root = temp_root("hook-client-materializer-routes");
    let config_path = root.join("config.toml");
    fs::write(
        &config_path,
        r#"
schemaId = "agent.semantic-protocols.hook.client-config"
schemaVersion = "1"
protocolId = "agent.semantic-protocols.hook"
protocolVersion = "1"

[[rules]]
id = "deny-source-access"
decision = "deny"
decisionMaterializer = "source-access"

[[rules.routes]]
providerId = "rs-harness"
kind = "query"
argv = ["asp", "rust", "query"]
"#,
    )
    .expect("write config");

    let error = load_hook_client_config_file(&config_path).expect_err("ambiguous materializer");

    assert!(
        error.contains("cannot combine decisionMaterializer"),
        "{error}"
    );
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
