use sha2::{Digest, Sha256};

use crate::rust_harness_activation::support::write_state_home_provider_binary;

use super::support::{
    codex_plugin_install_args, git_project_root, protocol_command, write_fake_codex_cli_in_dir,
    write_stable_runtime_asp_launcher,
};

fn write_managed_config_sidecar(path: &std::path::Path, bytes: &[u8]) {
    let sidecar = path.with_file_name(format!(
        "{}.managed.sha256",
        path.file_name()
            .and_then(|name| name.to_str())
            .expect("config file name")
    ));
    let digest = Sha256::digest(bytes);
    let hex = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    std::fs::write(sidecar, hex).expect("write managed config sidecar");
}

#[test]
fn cli_install_materializes_activation_under_state_core_not_prj_cache() {
    let root = git_project_root("install-prj-cache-home");
    let codex_home = root.join(".codex-home");
    let asp_state_home = root.join(".asp-state-home");
    write_state_home_provider_binary(&asp_state_home, "rust", "asp-rust", "asp-rust");
    let config_path = root.join(".agents").join("asp.toml");
    std::fs::create_dir_all(config_path.parent().expect("agent config parent"))
        .expect("create agent config parent");
    std::fs::write(
        &config_path,
        r#"[providers.typescript]
enabled = false

[providers.python]
enabled = false

[providers.julia]
enabled = false

[providers.gerbil-scheme]
enabled = false

[providers.org]
enabled = false

[providers.md]
enabled = false
"#,
    )
    .expect("write .agents/asp.toml");
    let protocol_bin_dir = write_stable_runtime_asp_launcher(&asp_state_home);
    write_fake_codex_cli_in_dir(&protocol_bin_dir);
    let prj_cache_home = root.join(".project-cache");
    let output = protocol_command()
        .env("PATH", &protocol_bin_dir)
        .env("SEMANTIC_AGENT_BIN_DIR", &protocol_bin_dir)
        .env("PRJ_CACHE_HOME", &prj_cache_home)
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
    let mut activation_paths = Vec::new();
    collect_activation_paths(&asp_state_home, &mut activation_paths);
    assert_eq!(
        activation_paths.len(),
        1,
        "activation paths: {activation_paths:?}"
    );
    assert!(activation_paths[0].starts_with(&asp_state_home));
    assert!(!root.join(".cache").exists());
    assert!(!prj_cache_home.exists());
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

#[test]
fn cli_install_refreshes_drifted_managed_client_hook_config() {
    let root = git_project_root("install-preserves-client-config");
    let codex_home = root.join(".codex-home");
    let asp_state_home = root.join(".asp-state-home");
    write_state_home_provider_binary(&asp_state_home, "rust", "asp-rust", "asp-rust");
    let protocol_bin_dir = write_stable_runtime_asp_launcher(&asp_state_home);
    write_fake_codex_cli_in_dir(&protocol_bin_dir);
    let client_config_path = asp_state_home.join("hooks/config.toml");
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
    write_managed_config_sidecar(&client_config_path, custom_config.as_bytes());
    let output = protocol_command()
        .env("PATH", &protocol_bin_dir)
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
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("userConfigStatus=migrated-managed"),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(&client_config_path).expect("read client config"),
        agent_semantic_hook::default_client_config_template()
    );
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn cli_install_refreshes_legacy_managed_hook_config() {
    let root = git_project_root("install-preserves-user-hook-config");
    let codex_home = root.join(".codex-home");
    let asp_state_home = root.join(".asp-state-home");
    write_state_home_provider_binary(
        &asp_state_home,
        "gerbil-scheme",
        "asp-gerbil-scheme",
        "asp-gerbil-scheme",
    );
    let protocol_bin_dir = write_stable_runtime_asp_launcher(&asp_state_home);
    let client_config_path = asp_state_home.join("hooks/config.toml");
    std::fs::create_dir_all(client_config_path.parent().expect("config parent"))
        .expect("create client config dir");
    write_fake_codex_cli_in_dir(&protocol_bin_dir);
    let legacy_config = r#"# Semantic agent client hook config.
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
"#;
    std::fs::write(&client_config_path, legacy_config).expect("write existing generated config");
    write_managed_config_sidecar(&client_config_path, legacy_config.as_bytes());

    let output = protocol_command()
        .env("PATH", &protocol_bin_dir)
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
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("userConfigStatus=migrated-managed"),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let client_config = std::fs::read_to_string(&client_config_path).expect("read client config");
    assert_eq!(
        client_config,
        agent_semantic_hook::default_client_config_template()
    );
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn cli_install_preserves_top_level_flags_without_forging_hook_trust() {
    let root = git_project_root("install-unified-exec-feature");
    let codex_home = root.join(".codex-home");
    let asp_state_home = root.join(".asp-state-home");
    write_state_home_provider_binary(&asp_state_home, "rust", "asp-rust", "asp-rust");
    let protocol_bin_dir = write_stable_runtime_asp_launcher(&asp_state_home);
    write_fake_codex_cli_in_dir(&protocol_bin_dir);
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
        .env("PATH", &protocol_bin_dir)
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
    let config = std::fs::read_to_string(&config_path).expect("installed config");
    let parsed_config =
        toml::from_str::<toml::Value>(&config).expect("installed Codex config is valid TOML");
    let features = parsed_config
        .get("features")
        .and_then(toml::Value::as_table)
        .expect("features table");
    assert_eq!(
        features.get("hooks").and_then(toml::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        features.get("plugins").and_then(toml::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        features.get("unified_exec").and_then(toml::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        features.get("multi_agent").and_then(toml::Value::as_bool),
        Some(true)
    );
    let marketplaces = parsed_config
        .get("marketplaces")
        .and_then(toml::Value::as_table);
    if let Some(marketplaces) = marketplaces
        && let Some(asp_project) = marketplaces
            .get("asp-project")
            .and_then(toml::Value::as_table)
    {
        assert_eq!(
            asp_project.get("source_type").and_then(toml::Value::as_str),
            Some("local")
        );
    }
    let user_config =
        std::fs::read_to_string(codex_home.join("config.toml")).expect("user trust config");
    let plugins = parsed_config
        .get("plugins")
        .and_then(toml::Value::as_table)
        .expect("plugins table");
    let plugin = plugins
        .get("asp-codex-plugin@asp-project")
        .and_then(toml::Value::as_table)
        .expect("asp-codex-plugin@asp-project plugin config");
    assert_eq!(
        plugin.get("enabled").and_then(toml::Value::as_bool),
        Some(true)
    );
    assert!(user_config.contains("sha256:old"));
    assert!(
        !user_config.contains("agent-semantic-protocol trusted hook state"),
        "plugin installation must not synthesize Codex hook trust"
    );
    let _ = std::fs::remove_dir_all(&root);
}
