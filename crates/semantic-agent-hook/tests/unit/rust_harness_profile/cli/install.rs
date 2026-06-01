use std::process::Command;

use semantic_agent_hook::parse_profiles;

use crate::rust_harness_profile::support::temp_project_root;

#[test]
fn cli_install_writes_root_owned_codex_hook_config() {
    let root = temp_project_root("install");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write temp Cargo.toml");
    let output = Command::new(env!("CARGO_BIN_EXE_semantic-agent-hook"))
        .args([
            "install",
            "--client",
            "codex",
            root.to_str().expect("utf8 temp root"),
        ])
        .output()
        .expect("run semantic-agent-hook install");

    assert!(
        output.status.success(),
        "install stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("install stdout");
    assert!(stdout.contains("[agent-install] client=codex"));
    assert!(stdout.contains("profiles=.codex/semantic-agent-hook/profiles.json"));
    let config =
        std::fs::read_to_string(root.join(".codex/config.toml")).expect("installed config");
    assert!(config.contains("# BEGIN semantic-agent-hook agent hooks"));
    assert!(config.contains(".codex/semantic-agent-hook/bin/semantic-agent-hook"));
    assert!(config.contains("semantic-agent-hook hook --client codex pre-tool"));
    assert!(config.contains("--profiles \"$profiles\""));
    let parsed_config =
        toml::from_str::<toml::Value>(&config).expect("installed Codex config is valid TOML");
    assert!(parsed_config.get("unified_exec").is_none());
    assert_eq!(
        parsed_config
            .get("features")
            .and_then(toml::Value::as_table)
            .and_then(|features| features.get("unified_exec"))
            .and_then(toml::Value::as_bool),
        Some(true)
    );
    assert!(config.contains("fs\\\\.read"));
    assert!(config.contains("multi_tool_use\\\\.parallel"));
    assert!(!config.contains("ts-harness agent hook --client codex"));
    assert!(!config.contains("rs-harness agent hook --client codex"));
    assert!(
        root.join(".codex/semantic-agent-hook/bin/semantic-agent-hook")
            .is_file()
    );
    let profiles = std::fs::read_to_string(root.join(".codex/semantic-agent-hook/profiles.json"))
        .expect("installed profile registry");
    let registry = parse_profiles(&profiles).expect("valid installed profile registry");
    assert_eq!(registry.profiles.len(), 1);
    assert_eq!(registry.profiles[0].language_id, "rust");
    assert_eq!(
        registry.profiles[0].commands.text.argv,
        [
            "rs-harness",
            "search",
            "text",
            "{query}",
            "tests",
            "--view",
            "seeds",
            "."
        ]
    );
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn cli_install_migrates_legacy_top_level_unified_exec_to_features() {
    let root = temp_project_root("install-unified-exec-feature");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write temp Cargo.toml");
    std::fs::create_dir_all(root.join(".codex")).expect("create .codex");
    let config_path = root.join(".codex/config.toml");
    std::fs::write(
        &config_path,
        "unified_exec = true\n\n[features]\nmulti_agent = true\n",
    )
    .expect("write legacy config");

    let output = Command::new(env!("CARGO_BIN_EXE_semantic-agent-hook"))
        .args([
            "install",
            "--client",
            "codex",
            root.to_str().expect("utf8 temp root"),
        ])
        .output()
        .expect("run semantic-agent-hook install");

    assert!(
        output.status.success(),
        "install stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let config = std::fs::read_to_string(&config_path).expect("installed config");
    let parsed_config =
        toml::from_str::<toml::Value>(&config).expect("installed Codex config is valid TOML");
    assert!(parsed_config.get("unified_exec").is_none());
    let features = parsed_config
        .get("features")
        .and_then(toml::Value::as_table)
        .expect("features table");
    assert_eq!(
        features.get("unified_exec").and_then(toml::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        features.get("multi_agent").and_then(toml::Value::as_bool),
        Some(true)
    );
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn cli_install_writes_executable_python_ingest_route() {
    let root = temp_project_root("install-python");
    std::fs::write(
        root.join("pyproject.toml"),
        "[project]\nname = \"demo-python\"\nversion = \"0.1.0\"\n",
    )
    .expect("write temp pyproject.toml");
    let output = Command::new(env!("CARGO_BIN_EXE_semantic-agent-hook"))
        .args([
            "install",
            "--client",
            "codex",
            root.to_str().expect("utf8 temp root"),
        ])
        .output()
        .expect("run semantic-agent-hook install");

    assert!(
        output.status.success(),
        "install stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let profiles = std::fs::read_to_string(root.join(".codex/semantic-agent-hook/profiles.json"))
        .expect("installed profile registry");
    let registry = parse_profiles(&profiles).expect("valid installed profile registry");
    let python = registry
        .profiles
        .iter()
        .find(|profile| profile.language_id == "python")
        .expect("python profile");
    assert_eq!(
        python.commands.ingest.argv,
        ["py-harness", "search", "ingest", "."]
    );
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn cli_install_refuses_to_overwrite_invalid_codex_toml() {
    let root = temp_project_root("install-invalid-toml");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write temp Cargo.toml");
    std::fs::create_dir_all(root.join(".codex")).expect("create .codex");
    let config_path = root.join(".codex/config.toml");
    std::fs::write(&config_path, "unified_exec = \"unterminated\n").expect("write invalid config");

    let output = Command::new(env!("CARGO_BIN_EXE_semantic-agent-hook"))
        .args([
            "install",
            "--client",
            "codex",
            root.to_str().expect("utf8 temp root"),
        ])
        .output()
        .expect("run semantic-agent-hook install");

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("refusing to write invalid Codex config TOML")
    );
    let config = std::fs::read_to_string(&config_path).expect("preserved config");
    assert_eq!(config, "unified_exec = \"unterminated\n");
    assert!(!config.contains("# BEGIN semantic-agent-hook agent hooks"));
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}
