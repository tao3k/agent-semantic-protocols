use std::env;

use agent_semantic_hook::parse_hook_activation;

use crate::rust_harness_activation::support::write_fake_provider_binary;

use super::support::{
    codex_plugin_install_args, codex_plugin_install_args_with_subagent_model, git_project_root,
    protocol_command,
};

#[test]
fn cli_install_writes_root_owned_codex_hook_config() {
    let root = git_project_root("install");
    let codex_home = root.join(".codex-home");
    let asp_state_home = root.join(".asp-state-home");
    let provider_path = write_fake_provider_binary(&root, "rs-harness");
    let protocol_bin_dir = root.join(".agent-bin");
    let path = env::join_paths([&protocol_bin_dir, &provider_path]).expect("join PATH");

    let output = protocol_command()
        .env("PATH", &path)
        .env("SEMANTIC_AGENT_BIN_DIR", &protocol_bin_dir)
        .env("CODEX_HOME", &codex_home)
        .env("ASP_STATE_HOME", &asp_state_home)
        .args(codex_plugin_install_args(&root))
        .output()
        .expect("run agent-semantic-protocol install");

    assert!(
        output.status.success(),
        "install stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("install stdout");
    assert_install_stdout(&stdout);
    assert!(
        !asp_state_home.join("hooks/config.toml").exists(),
        "install must not generate user hook config"
    );
    assert!(protocol_bin_dir.join("asp").is_file());
    assert_installed_codex_plugin_skill(&root);
    let config =
        std::fs::read_to_string(root.join(".codex/config.toml")).expect("installed config");
    assert_codex_config(&config, &root);
    assert_codex_user_asp_explorer_role_config(&codex_home);
    assert_agent_config(&root);
    let parsed_config =
        toml::from_str::<toml::Value>(&config).expect("installed Codex config is valid TOML");
    assert_plugin_entries(&parsed_config);
    assert_no_codex_user_trust_config(&codex_home);
    assert_codex_asp_explorer(&codex_home, "gpt-5.3-codex-spark");
    assert_installed_activation(&root);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn cli_install_accepts_existing_project_marketplace_source_when_root_matches() {
    let root = git_project_root("install-existing-marketplace-source");
    let codex_home = root.join(".codex-home");
    let asp_state_home = root.join(".asp-state-home");
    let provider_path = write_fake_provider_binary(&root, "rs-harness");
    let protocol_bin_dir = root.join(".agent-bin");
    let path = env::join_paths([&protocol_bin_dir, &provider_path]).expect("join PATH");
    write_project_codex_marketplace_source_dot(&root);

    let output = protocol_command()
        .env("PATH", &path)
        .env("SEMANTIC_AGENT_BIN_DIR", &protocol_bin_dir)
        .env("CODEX_HOME", &codex_home)
        .env("ASP_STATE_HOME", &asp_state_home)
        .args(codex_plugin_install_args(&root))
        .output()
        .expect("run agent-semantic-protocol install");

    assert!(
        output.status.success(),
        "install stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("install stdout");
    assert!(stdout.contains("pluginMarketplace=asp-project"));
    assert!(stdout.contains("codexAgentConfig=.codex-home/config.toml"));
    assert!(stdout.contains("subagent="));
    assert!(stdout.contains("agents/asp-explorer"));
    let config =
        std::fs::read_to_string(root.join(".codex/config.toml")).expect("installed config");
    let canonical_root = std::fs::canonicalize(&root).expect("canonical project root");
    assert!(config.contains("source = \".\""));
    assert!(!config.contains(&format!("source = \"{}\"", canonical_root.display())));
    assert!(!config.contains("last_updated ="));
    assert!(!config.contains("[agents.asp_explorer]"));
    assert_codex_user_asp_explorer_role_config(&codex_home);
    assert!(config.contains("[plugins.\"asp-codex-plugin@asp-project\"]"));
    assert_codex_asp_explorer(&codex_home, "gpt-5.3-codex-spark");
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

fn assert_install_stdout(stdout: &str) {
    assert!(stdout.contains("[plugin-install] client=codex"));
    assert!(stdout.contains("activation="));
    assert!(stdout.contains("userConfigStatus=missing"));
    assert!(
        stdout.contains(
            "pluginSkill=.codex/plugins/cache/asp-project/asp-codex-plugin/0.1.0/skills/agent-semantic-protocols/SKILL.org"
        )
    );
    assert!(stdout.contains("pluginCache=.codex/plugins/cache/asp-project/asp-codex-plugin/0.1.0"));
    assert!(stdout.contains("pluginScope=project"));
    assert!(stdout.contains("pluginMarketplace=asp-project"));
    assert!(stdout.contains("config=.codex/config.toml"));
    assert!(stdout.contains("projectConfig=.codex/config.toml"));
    assert!(stdout.contains("codexAgentConfig=.codex-home/config.toml"));
    assert!(stdout.contains("subagent="));
    assert!(stdout.contains("agents/asp-explorer"));
    assert!(stdout.contains("binary=asp"));
    assert!(stdout.contains("binaryInstall=installed"));
    assert!(stdout.contains("binaryPath="));
    assert!(stdout.contains("activationRuntime=derived"));
}

#[test]
fn cli_install_writes_codex_custom_subagent_with_requested_model() {
    let root = git_project_root("install-codex-subagent-model");
    let codex_home = root.join(".codex-home");
    let asp_state_home = root.join(".asp-state-home");
    let provider_path = write_fake_provider_binary(&root, "rs-harness");
    let protocol_bin_dir = root.join(".agent-bin");
    let path = env::join_paths([&protocol_bin_dir, &provider_path]).expect("join PATH");

    let output = protocol_command()
        .env("PATH", &path)
        .env("SEMANTIC_AGENT_BIN_DIR", &protocol_bin_dir)
        .env("CODEX_HOME", &codex_home)
        .env("ASP_STATE_HOME", &asp_state_home)
        .args(codex_plugin_install_args_with_subagent_model(
            &root,
            "gpt-5.4-mini",
        ))
        .output()
        .expect("run agent-semantic-protocol install");

    assert!(
        output.status.success(),
        "install stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("install stdout");
    assert!(stdout.contains("pluginScope=project"));
    assert!(stdout.contains("codexAgentConfig=.codex-home/config.toml"));
    assert!(stdout.contains("subagent="));
    assert!(stdout.contains("agents/asp-explorer"));
    assert_codex_user_asp_explorer_role_config(&codex_home);
    assert_codex_asp_explorer(&codex_home, "gpt-5.4-mini");
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn cli_install_rejects_empty_subagent_model_override() {
    let root = git_project_root("install-empty-subagent-model");

    let output = protocol_command()
        .args([
            "install",
            "plugin",
            "--codex",
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
        .args(["install", "plugin", "--codex", "--subagent-model"])
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

    let output = protocol_command()
        .env("PATH", &path)
        .env("SEMANTIC_AGENT_BIN_DIR", &protocol_bin_dir)
        .env("CODEX_HOME", &codex_home)
        .args([
            "install",
            "hook",
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
    assert_claude_asp_explorer(&root, "haiku");
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

fn assert_installed_codex_plugin_skill(root: &std::path::Path) {
    assert!(
        root.join(".codex/plugins/cache/asp-project/asp-codex-plugin/0.1.0/skills/agent-semantic-protocols/SKILL.org")
            .exists(),
        "Codex plugin installation should materialize the installed cache SKILL.org"
    );
}

fn assert_codex_config(config: &str, root: &std::path::Path) {
    assert!(config.contains("[marketplaces.asp-project]"));
    assert!(config.contains("source_type = \"local\""));
    let canonical_root = std::fs::canonicalize(root).expect("canonical project root");
    assert!(config.contains("source = \".\""));
    assert!(!config.contains(&format!("source = \"{}\"", canonical_root.display())));
    assert!(!config.contains("last_updated ="));
    assert!(!config.contains("[agents.asp_explorer]"));
    assert!(config.contains("[plugins.\"asp-codex-plugin@asp-project\"]"));
    assert!(config.contains("enabled = true"));
}

fn assert_no_codex_user_trust_config(codex_home: &std::path::Path) {
    let config_path = codex_home.join("config.toml");
    if !config_path.exists() {
        return;
    }
    let config = std::fs::read_to_string(config_path).expect("read Codex user config");
    assert!(
        !config.contains("[hooks.state."),
        "Codex plugin installation should not write hook trust state"
    );
    assert!(!config.contains("agent-semantic-protocol trusted hook state"));
}

fn assert_codex_user_asp_explorer_role_config(codex_home: &std::path::Path) {
    let config_path = codex_home.join("config.toml");
    let config = std::fs::read_to_string(config_path).expect("read Codex user config");
    assert!(config.contains("[agents.asp_explorer]"));
    assert!(config.contains("description = "));
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
        config["marketplaces"]["asp-project"]["source"].as_str(),
        Some(".")
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
    assert!(agent.contains("description:"));
    assert!(agent.contains(&format!("model: '{model}'")));
    assert!(agent.contains("permissionMode: plan"));
    assert!(agent.contains("maxTurns: 8"));
    assert_asp_explorer_instructions(&agent);
}

fn assert_codex_asp_explorer(codex_home: &std::path::Path, _model: &str) {
    let path = codex_home
        .parent()
        .expect("Codex home parent")
        .join(".asp-state-home")
        .join("agents")
        .join("asp-explorer_codex.toml");
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
    assert!(table.contains_key("description"));
    assert!(agent.contains("nickname_candidates = ["));
    assert!(agent.contains("session_lifetime = \"resident\""));
    assert!(agent.contains("model_reasoning_effort = \"medium\""));
    assert!(agent.contains("sandbox_mode = \"read-only\""));
    assert!(agent.contains("developer_instructions = \"\"\""));
    assert!(!agent.contains("fork_turns"), "{agent}");
    assert_asp_explorer_instructions(&agent);
}

fn assert_asp_explorer_instructions(instructions: &str) {
    let lower = instructions.to_ascii_lowercase();
    assert!(lower.contains("asp"));
    assert!(lower.contains("search"));
    assert!(lower.contains("query"));
    assert!(lower.contains("source"));
    assert!(instructions.contains("Own cheap but turn-expensive search work"));
    assert!(instructions.contains("search pipe, search owner, frontier ranking"));
    assert!(instructions.contains("owner/item discovery"));
    assert!(instructions.contains("[asp-search-subagent]"));
    assert!(instructions.contains("owner=<owner path>"));
    assert!(instructions.contains("read=<parser-owned selector>"));
    assert!(instructions.contains("item=<symbol or item identity, or ->"));
    assert!(instructions.contains("next=<exact asp query command for the parent to run>"));
    assert!(instructions.contains("must not run that final exact read yourself"));
    assert!(instructions.contains("Do not return source bodies, snippets, line-range selectors"));
    assert!(!instructions.contains("confidence is high"));
    assert!(!instructions.contains("missing=<missing facts"));
    assert!(!instructions.contains("risk=<risk"));
}

fn assert_agent_config(root: &std::path::Path) {
    let agent_config =
        std::fs::read_to_string(root.join(".agents/asp.toml")).expect("installed agent config");
    assert!(agent_config.contains("[skills.agent-semantic-protocols]"));
    assert!(!agent_config.contains("[hook.agentOrgArtifacts]"));
    assert!(!agent_config.contains("aspOrg"));
    assert!(!agent_config.contains("orgArtifacts"));
    toml::from_str::<toml::Value>(&agent_config).expect("agent config is valid TOML");
}

fn assert_installed_activation(root: &std::path::Path) {
    let activation =
        std::fs::read_to_string(installed_activation_path(root)).expect("installed activation");
    let registry = parse_hook_activation(&activation).expect("valid installed activation");
    let rust_provider = registry
        .providers
        .iter()
        .find(|provider| provider.language_id == "rust")
        .expect("rust provider activation");
    assert_eq!(
        rust_provider.routes.lexical.argv,
        [
            "rs-harness",
            "search",
            "lexical",
            "{query}",
            "owner",
            "tests",
            "--workspace",
            "{projectRoot}",
            "--view",
            "seeds"
        ]
    );
}

fn installed_activation_path(root: &std::path::Path) -> std::path::PathBuf {
    let mut matches = Vec::new();
    collect_activation_paths(&root.join(".asp-state-home"), &mut matches);
    matches.sort();
    assert_eq!(matches.len(), 1, "activation paths: {matches:?}");
    matches.remove(0)
}

fn collect_activation_paths(dir: &std::path::Path, matches: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_activation_paths(&path, matches);
        } else if path.ends_with("state/activation.json") {
            matches.push(path);
        }
    }
}
