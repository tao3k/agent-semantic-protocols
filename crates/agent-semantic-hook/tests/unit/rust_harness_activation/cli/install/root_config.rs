use std::env;

use agent_semantic_hook::parse_hook_activation;

use crate::rust_harness_activation::support::write_fake_provider_binary;

use super::support::{git_project_root, protocol_command};

#[test]
fn cli_install_writes_root_owned_codex_hook_config() {
    let root = git_project_root("install");
    let codex_home = root.join(".codex-home");
    let provider_path = write_fake_provider_binary(&root, "rs-harness");
    let protocol_bin_dir = root.join(".agent-bin");
    let path = env::join_paths([&protocol_bin_dir, &provider_path]).expect("join PATH");
    write_legacy_hook_cache(&root);
    write_incompatible_current_hook_events(&root);
    write_legacy_semantic_agent_protocol_config(&root);

    let output = protocol_command()
        .env("PATH", &path)
        .env("SEMANTIC_AGENT_BIN_DIR", &protocol_bin_dir)
        .env("CODEX_HOME", &codex_home)
        .args([
            "hook",
            "install",
            "--client",
            "codex",
            root.to_str().expect("utf8 temp root"),
        ])
        .output()
        .expect("run agent-semantic-protocol install");

    assert!(
        output.status.success(),
        "install stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("install stdout");
    assert_install_stdout(&stdout);
    assert!(protocol_bin_dir.join("asp").is_file());
    assert_installed_skill(&root);
    assert_no_profile_registry(&root);
    let config =
        std::fs::read_to_string(root.join(".codex/config.toml")).expect("installed config");
    assert_codex_config(&config, &root);
    assert_client_config(&root);
    let parsed_config =
        toml::from_str::<toml::Value>(&config).expect("installed Codex config is valid TOML");
    assert_plugin_entries(&parsed_config);
    assert_codex_user_trust_config(&codex_home, &root);
    assert_codex_asp_explorer(&root, "gpt-5.3-codex-spark");
    assert_legacy_codex_split_subagents_removed(&root);
    assert_installed_activation(&root);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn cli_install_accepts_existing_project_marketplace_source_when_root_matches() {
    let root = git_project_root("install-existing-marketplace-source");
    let codex_home = root.join(".codex-home");
    let provider_path = write_fake_provider_binary(&root, "rs-harness");
    let protocol_bin_dir = root.join(".agent-bin");
    let path = env::join_paths([&protocol_bin_dir, &provider_path]).expect("join PATH");
    write_project_codex_marketplace_source_dot(&root);

    let output = protocol_command()
        .env("PATH", &path)
        .env("SEMANTIC_AGENT_BIN_DIR", &protocol_bin_dir)
        .env("CODEX_HOME", &codex_home)
        .args([
            "hook",
            "install",
            "--client",
            "codex",
            root.to_str().expect("utf8 temp root"),
        ])
        .output()
        .expect("run agent-semantic-protocol install");

    assert!(
        output.status.success(),
        "install stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("install stdout");
    assert!(stdout.contains("pluginMarketplace=asp-project"));
    assert!(stdout.contains("subagent=.codex/agents/asp-explorer.toml"));
    let config =
        std::fs::read_to_string(root.join(".codex/config.toml")).expect("installed config");
    let canonical_root = std::fs::canonicalize(&root).expect("canonical project root");
    assert!(config.contains(&format!("source = \"{}\"", canonical_root.display())));
    assert_codex_asp_explorer_role_config(&config);
    assert!(config.contains("[plugins.\"asp-codex-plugin@asp-project\"]"));
    assert_codex_asp_explorer(&root, "gpt-5.3-codex-spark");
    let _ = std::fs::remove_dir_all(&root);
}

fn write_project_codex_marketplace_source_dot(root: &std::path::Path) {
    let config_path = root.join(".codex/config.toml");
    std::fs::create_dir_all(config_path.parent().expect("project Codex config parent"))
        .expect("create project Codex dir");
    std::fs::write(
        config_path,
        r#"[marketplaces.asp-project]
last_updated = "2026-01-01T00:00:00Z"
source_type = "local"
source = "."
"#,
    )
    .expect("write project Codex marketplace source");
}

fn write_legacy_hook_cache(root: &std::path::Path) {
    let current_cache_dir = root.join(".cache/agent-semantic-protocol/hooks");
    std::fs::create_dir_all(&current_cache_dir).expect("create current hook cache dir");
    std::fs::write(current_cache_dir.join("profiles.json"), r#"{"stale":true}"#)
        .expect("write stale current profile registry");
    std::fs::write(
        current_cache_dir.join("profiles.ts-harness.json"),
        r#"{"stale":true}"#,
    )
    .expect("write stale current provider profile shard");
    let legacy_profiles_dir = root.join(".codex/agent-semantic-hook");
    std::fs::create_dir_all(&legacy_profiles_dir).expect("create legacy profiles dir");
    std::fs::write(
        legacy_profiles_dir.join("profiles.ts-harness.json"),
        r#"{"stale":true}"#,
    )
    .expect("write stale provider profile shard");
    std::fs::write(
        legacy_profiles_dir.join("profiles.json"),
        r#"{"stale":true}"#,
    )
    .expect("write stale provider profile registry");
    std::fs::write(legacy_profiles_dir.join("events.jsonl"), "{}\n")
        .expect("write stale hook event cache");
    std::fs::write(
        legacy_profiles_dir.join("activation.json"),
        r#"{"stale":true}"#,
    )
    .expect("write stale activation");
    let legacy_cache_dir = root.join(".cache/agent-semantic-hook");
    std::fs::create_dir_all(&legacy_cache_dir).expect("create legacy cache dir");
    std::fs::write(legacy_cache_dir.join("profiles.json"), r#"{"stale":true}"#)
        .expect("write stale cache profile registry");
    std::fs::write(legacy_cache_dir.join("events.jsonl"), "{}\n")
        .expect("write stale cache events");
    std::fs::write(
        legacy_cache_dir.join("activation.json"),
        r#"{"stale":true}"#,
    )
    .expect("write stale cache activation");
    let legacy_protocol_cache_dir = root.join(".cache/semantic-agent-protocol/hooks");
    std::fs::create_dir_all(&legacy_protocol_cache_dir).expect("create legacy protocol cache dir");
    std::fs::write(
        legacy_protocol_cache_dir.join("profiles.json"),
        r#"{"stale":true}"#,
    )
    .expect("write stale semantic protocol profile registry");
    std::fs::write(legacy_protocol_cache_dir.join("events.jsonl"), "{}\n")
        .expect("write stale semantic protocol events");
    std::fs::write(
        legacy_protocol_cache_dir.join("activation.json"),
        r#"{"stale":true}"#,
    )
    .expect("write stale semantic protocol activation");
}

fn write_incompatible_current_hook_events(root: &std::path::Path) {
    let current_cache_dir = root.join(".cache/agent-semantic-protocol/hooks");
    std::fs::create_dir_all(&current_cache_dir).expect("create current hook cache dir");
    std::fs::write(
        current_cache_dir.join("events.jsonl"),
        r#"{"schemaId":"agent.semantic-protocols.agent-semantic-hook-event","protocolId":"agent.semantic-protocols.agent-hooks"}"#,
    )
    .expect("write incompatible hook event state");
}

fn assert_install_stdout(stdout: &str) {
    assert!(stdout.contains("[agent-install] client=codex"));
    assert!(stdout.contains("activation="));
    assert!(stdout.contains("agent-semantic-protocol/hooks/activation.json"));
    assert!(stdout.contains("clientConfig=.codex/agent-semantic-protocol/hooks/config.toml"));
    assert!(!stdout.contains("profileCache="));
    assert!(!stdout.contains("agent-semantic-protocol/hooks/profiles.json"));
    assert!(stdout.contains("skill=removed"));
    assert!(stdout.contains("skillContract=removed"));
    assert!(
        stdout.contains("pluginSkill=asp-codex-plugin/skills/agent-semantic-protocols/SKILL.org")
    );
    assert!(stdout.contains(
        "pluginSkillContract=asp-codex-plugin/skills/agent-semantic-protocols/SKILL.contract.org"
    ));
    assert!(stdout.contains("pluginScope=project"));
    assert!(stdout.contains("pluginMarketplace=asp-project"));
    assert!(stdout.contains("pluginMarketplaceConfig=.agents/plugins/marketplace.json"));
    assert!(stdout.contains("projectHookConfig=.codex/config.toml"));
    assert!(stdout.contains("trustConfig="));
    assert!(stdout.contains("subagent=.codex/agents/asp-explorer.toml"));
    assert!(!stdout.contains("subagents="));
    assert!(stdout.contains("binary=asp"));
    assert!(stdout.contains("binaryInstall=installed"));
    assert!(stdout.contains("binaryPath="));
    assert!(stdout.contains("activationRuntime=derived"));
}

#[test]
fn cli_install_writes_codex_custom_subagent_with_requested_model() {
    let root = git_project_root("install-codex-subagent-model");
    let codex_home = root.join(".codex-home");
    let provider_path = write_fake_provider_binary(&root, "rs-harness");
    let protocol_bin_dir = root.join(".agent-bin");
    let path = env::join_paths([&protocol_bin_dir, &provider_path]).expect("join PATH");
    write_stale_codex_subagents(&root);

    let output = protocol_command()
        .env("PATH", &path)
        .env("SEMANTIC_AGENT_BIN_DIR", &protocol_bin_dir)
        .env("CODEX_HOME", &codex_home)
        .args([
            "hook",
            "install",
            "--client",
            "codex",
            "--subagent-model",
            "gpt-5.4-mini",
            root.to_str().expect("utf8 temp root"),
        ])
        .output()
        .expect("run agent-semantic-protocol install");

    assert!(
        output.status.success(),
        "install stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("install stdout");
    assert!(stdout.contains("pluginScope=project"));
    assert!(stdout.contains("subagent=.codex/agents/asp-explorer.toml"));
    assert!(!stdout.contains("subagents="));
    assert!(!stdout.contains(".codex/agents/asp-explorer-selector.toml"));
    assert_codex_asp_explorer(&root, "gpt-5.4-mini");
    assert_legacy_codex_split_subagents_removed(&root);
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn cli_install_rejects_empty_subagent_model_override() {
    let root = git_project_root("install-empty-subagent-model");

    let output = protocol_command()
        .args([
            "hook",
            "install",
            "--client",
            "codex",
            "--subagent-model=",
            root.to_str().expect("utf8 temp root"),
        ])
        .output()
        .expect("run agent-semantic-protocol install");

    assert!(!output.status.success(), "install unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--subagent-model must not be empty"),
        "install stderr: {stderr}"
    );
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn cli_install_rejects_missing_subagent_model_value() {
    let output = protocol_command()
        .args(["hook", "install", "--client", "codex", "--subagent-model"])
        .output()
        .expect("run agent-semantic-protocol install");

    assert!(!output.status.success(), "install unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--subagent-model requires a value"),
        "install stderr: {stderr}"
    );
}

#[test]
fn cli_install_writes_claude_custom_subagent_by_default() {
    let root = git_project_root("install-claude-subagent");
    let codex_home = root.join(".codex-home");
    let provider_path = write_fake_provider_binary(&root, "rs-harness");
    let protocol_bin_dir = root.join(".agent-bin");
    let path = env::join_paths([&protocol_bin_dir, &provider_path]).expect("join PATH");
    write_stale_claude_subagents(&root);

    let output = protocol_command()
        .env("PATH", &path)
        .env("SEMANTIC_AGENT_BIN_DIR", &protocol_bin_dir)
        .env("CODEX_HOME", &codex_home)
        .args([
            "hook",
            "install",
            "--client",
            "claude",
            root.to_str().expect("utf8 temp root"),
        ])
        .output()
        .expect("run agent-semantic-protocol install");

    assert!(
        output.status.success(),
        "install stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("install stdout");
    assert!(stdout.contains("[agent-install] client=claude"));
    assert!(stdout.contains("subagent=.claude/agents/asp-explorer.md"));
    assert!(!stdout.contains("subagents="));
    assert!(!stdout.contains(".claude/agents/asp-explorer-selector.md"));
    assert_claude_asp_explorer(&root, "haiku");
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

fn assert_installed_skill(root: &std::path::Path) {
    assert!(
        !root
            .join(".agents/skills/agent-semantic-protocols/SKILL.org")
            .exists(),
        "Codex plugin install should remove the legacy project skill copy"
    );
    assert!(
        !root
            .join(".agents/skills/agent-semantic-protocols/SKILL.contract.org")
            .exists(),
        "Codex plugin install should remove the legacy project skill contract copy"
    );
    let skill = std::fs::read_to_string(
        root.join("asp-codex-plugin/skills/agent-semantic-protocols/SKILL.org"),
    )
    .expect("installed plugin skill");
    assert!(skill.contains("Generated by =asp hook install="));
    assert!(skill.contains("Do not edit this installed copy"));
    assert!(skill.contains("* Provider Contracts"));
    assert!(skill.contains(":LANGUAGE_ID: rust"));
    assert!(!skill.contains("/.bin/rs-harness` |"));
    assert!(!skill.contains(&root.display().to_string()));
    assert!(skill.contains("Start with =asp <language> guide"));
    assert!(!skill.contains("Start with =asp <language> agent guide"));
    assert!(!skill.contains("=asp typescript="));
    assert!(!skill.contains("=asp python="));
    assert!(!skill.contains("=asp julia="));
    assert!(!skill.contains("Julia participates in language facade parity"));
    assert!(skill.contains("Do not add =--json= during agent exploration."));
    assert!(skill.contains("single-quoted argv literal"));
    assert!(!skill.contains("--query-set 'Start with =asp <language> guide"));
    assert!(
        root.join("asp-codex-plugin/skills/agent-semantic-protocols/SKILL.contract.org")
            .is_file(),
        "plugin skill contract should be installed"
    );
}

fn assert_no_profile_registry(root: &std::path::Path) {
    assert!(
        !root
            .join(".cache/agent-semantic-protocol/hooks/profiles.json")
            .exists()
    );
    assert!(
        !root
            .join(".cache/agent-semantic-protocol/hooks/profiles.ts-harness.json")
            .exists()
    );
    assert!(
        !root
            .join(".codex/agent-semantic-hook/profiles.ts-harness.json")
            .exists()
    );
    assert!(
        !root
            .join(".codex/agent-semantic-hook/profiles.json")
            .exists()
    );
    assert!(
        !root
            .join(".codex/agent-semantic-hook/events.jsonl")
            .exists()
    );
    assert!(
        !root
            .join(".cache/agent-semantic-hook/profiles.json")
            .exists()
    );
    assert!(
        !root
            .join(".cache/agent-semantic-hook/events.jsonl")
            .exists()
    );
    assert!(
        !root
            .join(".cache/semantic-agent-protocol/hooks/profiles.json")
            .exists()
    );
    assert!(
        !root
            .join(".cache/semantic-agent-protocol/hooks/events.jsonl")
            .exists()
    );
    assert!(
        !root
            .join(".cache/semantic-agent-protocol/hooks/activation.json")
            .exists()
    );
    assert!(
        !root
            .join(".cache/agent-semantic-protocol/hooks/events.jsonl")
            .exists()
    );
}

fn assert_codex_config(config: &str, root: &std::path::Path) {
    assert!(config.contains("# BEGIN agent-semantic-protocol agent hooks"));
    assert!(!config.contains("# BEGIN semantic-agent-protocol agent hooks"));
    assert!(!config.contains("# BEGIN agent-semantic-hook agent hooks"));
    assert!(!config.contains("hook_bin="));
    assert!(!config.contains("exec semantic-agent-protocol"));
    assert!(!config.contains("exec agent-semantic-hook"));
    assert!(!config.contains(".codex/agent-semantic-hook/bin/agent-semantic-hook"));
    assert!(config.contains("exec asp hook pre-tool --client codex"));
    assert!(config.contains("exec asp hook user-prompt --client codex"));
    assert!(config.contains("exec asp hook stop --client codex"));
    assert!(!config.contains("asp hook --client codex"));
    assert!(config.matches("[hooks.state.").count() == 0);
    assert!(!config.contains("ts-harness agent hook --client codex"));
    assert!(!config.contains("rs-harness agent hook --client codex"));
    assert!(config.contains("[marketplaces.asp-project]"));
    assert!(config.contains("source_type = \"local\""));
    let canonical_root = std::fs::canonicalize(root).expect("canonical project root");
    assert!(config.contains(&format!("source = \"{}\"", canonical_root.display())));
    assert_codex_asp_explorer_role_config(config);
    assert!(config.contains("[plugins.\"asp-codex-plugin@asp-project\"]"));
    assert!(config.contains("enabled = true"));
}

fn assert_codex_user_trust_config(codex_home: &std::path::Path, root: &std::path::Path) {
    let trust_config =
        std::fs::read_to_string(codex_home.join("config.toml")).expect("Codex trust config");
    let project_config = std::fs::canonicalize(root.join(".codex/config.toml"))
        .expect("canonical project Codex config");
    assert!(trust_config.contains("[projects."));
    assert!(trust_config.contains(&format!("{}:pre_tool_use:0:0", project_config.display())));
    assert!(trust_config.contains(&format!(
        "{}:user_prompt_submit:0:0",
        project_config.display()
    )));
    assert!(trust_config.contains(&format!("{}:stop:0:0", project_config.display())));
    assert!(trust_config.contains("trusted_hash = \"sha256:"));
    toml::from_str::<toml::Value>(&trust_config).expect("Codex trust config is valid TOML");
}

fn assert_codex_asp_explorer_role_config(config: &str) {
    assert!(config.contains("[agents.asp_explorer]"));
    assert!(config.contains("description = \"Read-only ASP search explorer"));
    assert!(config.contains("config_file = \"agents/asp-explorer.toml\""));
    assert!(config.contains(
        "nickname_candidates = [\"ASP owner\", \"ASP rg\", \"ASP selector\", \"ASP search\"]"
    ));
}

fn assert_plugin_entries(config: &toml::Value) {
    assert_eq!(
        config["marketplaces"]["asp-project"]["source_type"].as_str(),
        Some("local")
    );
    assert_eq!(
        config["plugins"]["asp-codex-plugin@asp-project"]["enabled"].as_bool(),
        Some(true)
    );
}

fn assert_claude_asp_explorer(root: &std::path::Path, model: &str) {
    let path = root.join(".claude/agents/asp-explorer.md");
    let agent = std::fs::read_to_string(&path).expect("installed Claude ASP explorer agent");
    assert!(agent.contains("name: asp-explorer"));
    assert!(agent.contains("description: Read-only ASP"));
    assert!(agent.contains(&format!("model: '{model}'")));
    assert!(agent.contains("permissionMode: plan"));
    assert!(agent.contains("maxTurns: 8"));
    assert_asp_explorer_instructions(&agent);
    for stale in [
        "asp-explorer-owner.md",
        "asp-explorer-rg.md",
        "asp-explorer-selector.md",
    ] {
        assert!(
            !root.join(".claude/agents").join(stale).exists(),
            "stale generated Claude subagent was not removed: {stale}"
        );
    }
}

fn assert_codex_asp_explorer(root: &std::path::Path, model: &str) {
    let path = root.join(".codex/agents/asp-explorer.toml");
    let agent = std::fs::read_to_string(&path).expect("installed Codex ASP explorer agent");
    let parsed = toml::from_str::<toml::Value>(&agent).expect("Codex ASP explorer is valid TOML");
    let table = parsed
        .as_table()
        .expect("Codex ASP explorer is a TOML table");
    assert!(
        !table.contains_key("fork_context"),
        "fork_context is a spawn_agent call argument, not a custom-agent TOML key"
    );
    assert!(
        !table.contains_key("fork_turns"),
        "fork_turns is not a supported custom-agent TOML key"
    );
    assert!(agent.contains("name = \"asp_explorer\""));
    assert!(agent.contains("description = \"Read-only ASP"));
    assert!(agent.contains(&format!("model = \"{model}\"")));
    assert!(agent.contains("nickname_candidates = ["));
    assert!(agent.contains("model_reasoning_effort = \"medium\""));
    assert!(agent.contains("sandbox_mode = \"read-only\""));
    assert!(agent.contains("developer_instructions = \"\"\""));
    assert!(agent.contains("fork_context=false"));
    assert!(!agent.contains("fork_turns"), "{agent}");
    assert_asp_explorer_instructions(&agent);
}

fn assert_asp_explorer_instructions(instructions: &str) {
    assert!(instructions.contains("Use ASP provider commands before source reads"));
    assert!(
        instructions.contains("After prime, the immediate next ASP command must be search pipe")
    );
    assert!(instructions.contains("Compress broad prose into 2-4 stable terms before search pipe"));
    assert!(instructions.contains("If search pipe returns nextCommand or an exact query-selector"));
    assert!(instructions.contains("If a hook denies read-before-pipe, repeated-search-pipe"));
    assert!(instructions.contains("Resident search-agent control is owned by the parent"));
    assert!(instructions.contains("Spawn-only controls such as fork_context"));
    assert!(instructions.contains("keep exactly one ASP search agent thread per main task"));
    assert!(instructions.contains("reuse that thread with send_input"));
    assert!(instructions.contains("Only spawn a new ASP search agent when no recorded agent id"));
    assert!(instructions.contains("Do not assume hidden sibling context"));
    assert!(instructions.contains("action=<action id"));
}

fn write_stale_codex_subagents(root: &std::path::Path) {
    let dir = root.join(".codex/agents");
    std::fs::create_dir_all(&dir).expect("create stale Codex agents dir");
    for stale in [
        "asp-explorer.toml",
        "asp-explorer-owner.toml",
        "asp-explorer-rg.toml",
        "asp-explorer-selector.toml",
    ] {
        std::fs::write(dir.join(stale), "name = \"stale\"\n").expect("write stale Codex agent");
    }
}

fn assert_legacy_codex_split_subagents_removed(root: &std::path::Path) {
    for stale in [
        "asp-explorer-owner.toml",
        "asp-explorer-rg.toml",
        "asp-explorer-selector.toml",
    ] {
        assert!(
            !root.join(".codex/agents").join(stale).exists(),
            "legacy Codex split subagent should be removed during plugin install: {stale}"
        );
    }
}

fn write_stale_claude_subagents(root: &std::path::Path) {
    let dir = root.join(".claude/agents");
    std::fs::create_dir_all(&dir).expect("create stale Claude agents dir");
    for stale in [
        "asp-explorer-owner.md",
        "asp-explorer-rg.md",
        "asp-explorer-selector.md",
    ] {
        std::fs::write(dir.join(stale), "---\nname: stale\n---\n")
            .expect("write stale Claude agent");
    }
}

fn write_legacy_semantic_agent_protocol_config(root: &std::path::Path) {
    let codex_dir = root.join(".codex");
    std::fs::create_dir_all(&codex_dir).expect("create legacy codex config dir");
    std::fs::write(
        codex_dir.join("config.toml"),
        r#"[features]
hooks = true
unified_exec = true

# BEGIN semantic-agent-protocol agent hooks
[[hooks.PreToolUse]]
matcher = "Read"

[[hooks.PreToolUse.hooks]]
type = "command"
timeout = 5
statusMessage = "Checking old semantic hook"
command = '''
exec semantic-agent-protocol hook pre-tool --client codex
'''
# END semantic-agent-protocol agent hooks

[agents.asp_explorer]
description = "legacy standalone role"
config_file = "agents/asp-explorer.toml"
"#,
    )
    .expect("write legacy semantic-agent-protocol config");
}

fn assert_client_config(root: &std::path::Path) {
    let client_config =
        std::fs::read_to_string(root.join(".codex/agent-semantic-protocol/hooks/config.toml"))
            .expect("installed client hook config");
    assert!(client_config.contains("schemaId = "));
    assert!(client_config.contains("[experimental.semanticAstPatch]"));
    assert!(client_config.contains("enabled = false"));
    assert!(client_config.contains("[[rules]]"));
    assert!(client_config.contains("id = \"deny-shell-source-argv\""));
    assert!(client_config.contains(
        "toolAny = [\"Bash\", \"shell\", \"functions.exec_command\", \"exec_command\", \"command_execution\"]"
    ));
    assert!(client_config.contains("commandAny = [\"sed\", \"perl\", \"rg\", \"wl\"]"));
    assert!(client_config.contains("argvSourceGlobAny = ["));
    assert!(client_config.contains("\"**/*.rs\""));
    assert!(client_config.contains("argvSourceExcludeFlagAny = ["));
    toml::from_str::<toml::Value>(&client_config).expect("client hook config is valid TOML");
}

fn assert_installed_activation(root: &std::path::Path) {
    assert!(
        !root
            .join(".codex/agent-semantic-hook/bin/agent-semantic-hook")
            .exists()
    );
    let activation =
        std::fs::read_to_string(root.join(".cache/agent-semantic-protocol/hooks/activation.json"))
            .expect("installed activation");
    let registry = parse_hook_activation(&activation).expect("valid installed activation");
    let rust_provider = registry
        .providers
        .iter()
        .find(|provider| provider.language_id == "rust")
        .expect("rust provider activation");
    assert_eq!(
        rust_provider.routes.fzf.argv,
        [
            "rs-harness",
            "search",
            "fzf",
            "{query}",
            "owner",
            "tests",
            "--view",
            "seeds",
            "{projectRoot}"
        ]
    );
}
