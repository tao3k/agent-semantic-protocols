use std::env;

use sha2::{Digest, Sha256};

use crate::rust_harness_activation::support::{asp_bin_dir, write_fake_provider_binary};

use super::support::{codex_plugin_install_args, git_project_root, protocol_command};

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
fn cli_install_uses_state_core_home_over_prj_cache_home() {
    let root = git_project_root("install-prj-cache-home");
    let codex_home = root.join(".codex-home");
    let asp_state_home = root.join(".asp-state-home");
    let provider_path = write_fake_provider_binary(&root, "rs-harness");
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
    let protocol_bin_dir = root.join(".agent-bin");
    let prj_cache_home = root.join(".project-cache");
    let path = env::join_paths([&protocol_bin_dir, &provider_path]).expect("join PATH");
    let output = protocol_command()
        .env("PATH", &path)
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
    assert!(installed_activation_path(&asp_state_home).is_file());
    assert!(!root.join(".cache").exists());
    assert!(!prj_cache_home.exists());
}

fn installed_activation_path(state_home: &std::path::Path) -> std::path::PathBuf {
    let mut matches = Vec::new();
    collect_activation_paths(state_home, &mut matches);
    let expected_project_root = state_home
        .parent()
        .expect("state home must be rooted under the fixture project")
        .canonicalize()
        .expect("canonical fixture project root");
    // ASP_STATE_HOME may contain activations for multiple independently owned
    // roots. The install contract is one activation for this project scope,
    // not one activation across the entire state home.
    matches.retain(|path| {
        let activation: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(path).expect("read activation candidate"),
        )
        .expect("parse activation candidate");
        activation
            .get("projectRoot")
            .and_then(serde_json::Value::as_str)
            .map(std::path::PathBuf::from)
            .and_then(|project_root| project_root.canonicalize().ok())
            .is_some_and(|project_root| project_root == expected_project_root)
    });
    matches.sort();
    assert_eq!(
        matches.len(),
        1,
        "project activation paths for {expected_project_root:?}: {matches:?}"
    );
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

#[test]
fn cli_install_refreshes_drifted_managed_client_hook_config() {
    let root = git_project_root("install-preserves-client-config");
    let codex_home = root.join(".codex-home");
    let asp_state_home = root.join(".asp-state-home");
    let provider_path = write_fake_provider_binary(&root, "rs-harness");
    let protocol_bin_dir = root.join(".agent-bin");
    let path = env::join_paths([&protocol_bin_dir, &provider_path]).expect("join PATH");
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
    assert!(String::from_utf8_lossy(&output.stdout).contains("userConfigStatus=migrated-managed"));
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
    let provider_path = write_fake_provider_binary(&root, "gslph");
    let protocol_bin_dir = root.join(".agent-bin");
    let path = env::join_paths([&protocol_bin_dir, &provider_path]).expect("join PATH");
    let client_config_path = asp_state_home.join("hooks/config.toml");
    std::fs::create_dir_all(client_config_path.parent().expect("config parent"))
        .expect("create client config dir");
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
    assert!(String::from_utf8_lossy(&output.stdout).contains("userConfigStatus=migrated-managed"));

    let client_config = std::fs::read_to_string(&client_config_path).expect("read client config");
    assert_eq!(
        client_config,
        agent_semantic_hook::default_client_config_template()
    );
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn cli_install_preserves_top_level_flags_and_writes_project_plugin_entries() {
    let root = git_project_root("install-unified-exec-feature");
    let codex_home = root.join(".codex-home");
    let provider_path = write_fake_provider_binary(&root, "rs-harness");
    let asp_bin_dir = asp_bin_dir();
    let path = env::join_paths([provider_path.as_path(), asp_bin_dir.as_path()])
        .expect("provider and asp PATH");
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
        .env("PATH", &path)
        .env("SEMANTIC_AGENT_BIN_DIR", &asp_bin_dir)
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
    if let Some(marketplaces) = marketplaces {
        if let Some(asp_project) = marketplaces
            .get("asp-project")
            .and_then(toml::Value::as_table)
        {
            assert_eq!(
                asp_project.get("source_type").and_then(toml::Value::as_str),
                Some("local")
            );
        }
    }
    let user_config =
        std::fs::read_to_string(codex_home.join("config.toml")).expect("user trust config");
    let parsed_user_config =
        toml::from_str::<toml::Value>(&user_config).expect("user Codex config is valid TOML");
    let plugins = parsed_user_config
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
    assert!(user_config.contains("agent-semantic-protocol trusted hook state"));
    let _ = std::fs::remove_dir_all(&root);
}
