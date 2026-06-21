use std::env;

use crate::rust_harness_activation::support::write_fake_provider_binary;

use super::support::{codex_plugin_install_args, git_project_root, protocol_command};

#[test]
fn cli_install_prefers_git_toplevel_cache_over_prj_cache_home() {
    let root = git_project_root("install-prj-cache-home");
    let codex_home = root.join(".codex-home");
    let provider_path = write_fake_provider_binary(&root, "rs-harness");
    let protocol_bin_dir = root.join(".agent-bin");
    let prj_cache_home = root.join(".project-cache");
    let path = env::join_paths([&protocol_bin_dir, &provider_path]).expect("join PATH");
    let output = protocol_command()
        .env("PATH", &path)
        .env("SEMANTIC_AGENT_BIN_DIR", &protocol_bin_dir)
        .env("PRJ_CACHE_HOME", &prj_cache_home)
        .env("CODEX_HOME", &codex_home)
        .args(codex_plugin_install_args(&root))
        .output()
        .expect("run agent-semantic-protocol install");
    assert!(
        output.status.success(),
        "install stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("install stdout");
    assert!(
        root.join(".cache")
            .join("agent-semantic-protocol/hooks/activation.json")
            .is_file()
    );
    assert!(!stdout.contains("profileCache="));
    assert!(
        !root
            .join(".cache")
            .join("agent-semantic-protocol/hooks/profiles.json")
            .exists()
    );
    assert!(
        !prj_cache_home
            .join("agent-semantic-protocol/hooks/profiles.json")
            .exists()
    );
}

#[test]
fn cli_install_preserves_existing_client_hook_config() {
    let root = git_project_root("install-preserves-client-config");
    let codex_home = root.join(".codex-home");
    let provider_path = write_fake_provider_binary(&root, "rs-harness");
    let protocol_bin_dir = root.join(".agent-bin");
    let path = env::join_paths([&protocol_bin_dir, &provider_path]).expect("join PATH");
    let client_config_path = root.join(".codex/agent-semantic-protocol/hooks/config.toml");
    std::fs::create_dir_all(client_config_path.parent().expect("config parent"))
        .expect("create client config dir");
    let custom_config = r#"schemaId = "agent.semantic-protocols.hook.client-config"
schemaVersion = "1"
protocolId = "agent.semantic-protocols.hook"
protocolVersion = "1"

[[rules]]
id = "custom-rule"
decision = "deny"
"#;
    std::fs::write(&client_config_path, custom_config).expect("write custom config");
    let output = protocol_command()
        .env("PATH", &path)
        .env("SEMANTIC_AGENT_BIN_DIR", &protocol_bin_dir)
        .env("CODEX_HOME", &codex_home)
        .args(codex_plugin_install_args(&root))
        .output()
        .expect("run agent-semantic-protocol install");
    assert!(
        output.status.success(),
        "install stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(&client_config_path).expect("read client config"),
        custom_config
    );
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn cli_install_refreshes_generated_client_hook_config_from_activation_extensions() {
    let root = git_project_root("install-refreshes-generated-client-config");
    let codex_home = root.join(".codex-home");
    let provider_path = write_fake_provider_binary(&root, "gslph");
    let protocol_bin_dir = root.join(".agent-bin");
    let path = env::join_paths([&protocol_bin_dir, &provider_path]).expect("join PATH");
    let client_config_path = root.join(".codex/agent-semantic-protocol/hooks/config.toml");
    std::fs::create_dir_all(client_config_path.parent().expect("config parent"))
        .expect("create client config dir");
    std::fs::write(
        &client_config_path,
        r#"# Semantic agent client hook config.
schemaId = "agent.semantic-protocols.hook.client-config"
schemaVersion = "1"
protocolId = "agent.semantic-protocols.hook"
protocolVersion = "1"

[[rules]]
id = "deny-shell-source-argv"
decision = "deny"

[rules.match]
tool = "Bash"
commandAny = ["sed", "perl", "rg", "wl"]
argvSourceGlobAny = [
  "*.ss", "**/*.ss",
  "*.scm", "**/*.scm",
]
"#,
    )
    .expect("write stale generated config");

    let output = protocol_command()
        .env("PATH", &path)
        .env("SEMANTIC_AGENT_BIN_DIR", &protocol_bin_dir)
        .env("CODEX_HOME", &codex_home)
        .args(codex_plugin_install_args(&root))
        .output()
        .expect("run agent-semantic-protocol install");
    assert!(
        output.status.success(),
        "install stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let client_config = std::fs::read_to_string(&client_config_path).expect("read client config");
    assert!(client_config.contains("\"*.ss\", \"**/*.ss\""));
    assert!(client_config.contains("\"*.ssi\", \"**/*.ssi\""));
    assert!(client_config.contains("\"*.scm\", \"**/*.scm\""));
    assert!(client_config.contains("\"*.sld\", \"**/*.sld\""));
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn cli_install_preserves_top_level_flags_and_writes_project_plugin_entries() {
    let root = git_project_root("install-unified-exec-feature");
    let codex_home = root.join(".codex-home");
    let provider_path = write_fake_provider_binary(&root, "rs-harness");
    std::fs::create_dir_all(root.join(".codex")).expect("create .codex");
    let config_path = root.join(".codex/config.toml");
    std::fs::write(
        &config_path,
        "hooks = false\nunified_exec = true\n\n[features]\nmulti_agent = true\n",
    )
    .expect("write transitional config");
    std::fs::create_dir_all(&codex_home).expect("create codex home");
    std::fs::write(
        codex_home.join("config.toml"),
        "[hooks.state.\"stale:pre_tool_use:0:0\"]\ntrusted_hash = \"sha256:old\"\n",
    )
    .expect("write stale user trust state");

    let output = protocol_command()
        .env("PATH", &provider_path)
        .env("CODEX_HOME", &codex_home)
        .args(codex_plugin_install_args(&root))
        .output()
        .expect("run agent-semantic-protocol install");

    assert!(
        output.status.success(),
        "install stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let config = std::fs::read_to_string(&config_path).expect("installed config");
    let parsed_config =
        toml::from_str::<toml::Value>(&config).expect("installed Codex config is valid TOML");
    assert_eq!(
        parsed_config
            .get("unified_exec")
            .and_then(toml::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        parsed_config.get("hooks").and_then(toml::Value::as_bool),
        Some(false)
    );
    let features = parsed_config
        .get("features")
        .and_then(toml::Value::as_table)
        .expect("features table");
    assert_eq!(
        features.get("multi_agent").and_then(toml::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        parsed_config["marketplaces"]["asp-project"]["source_type"].as_str(),
        Some("local")
    );
    assert_eq!(
        parsed_config["plugins"]["asp-codex-plugin@asp-project"]["enabled"].as_bool(),
        Some(true)
    );
    let user_config =
        std::fs::read_to_string(codex_home.join("config.toml")).expect("user trust config");
    assert!(user_config.contains("sha256:old"));
    assert!(!user_config.contains("agent-semantic-protocol trusted hook state"));
    let _ = std::fs::remove_dir_all(&root);
}
