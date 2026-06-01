use std::path::PathBuf;
use std::process::Command;

use semantic_agent_hook::parse_profiles;

use crate::rust_harness_profile::support::temp_project_root;

#[test]
fn cli_install_writes_root_owned_codex_hook_config() {
    let root = temp_project_root("install");
    let codex_home = root.join(".codex-home");
    let provider_path = write_fake_provider_binary(&root, "rs-harness");
    let output = Command::new(env!("CARGO_BIN_EXE_semantic-agent-hook"))
        .env("PATH", &provider_path)
        .env("CODEX_HOME", &codex_home)
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
    assert!(stdout.contains("skill=.agents/skills/agent-semantic-protocols/SKILL.md"));
    assert!(stdout.contains("trustConfig="));
    assert!(stdout.contains("binary=semantic-agent-hook"));
    let skill =
        std::fs::read_to_string(root.join(".agents/skills/agent-semantic-protocols/SKILL.md"))
            .expect("installed agent skill");
    assert!(skill.contains("rs-harness agent guide ."));
    assert!(skill.contains("ts-harness agent guide ."));
    assert!(skill.contains("py-harness agent guide ."));
    assert!(skill.contains("Do not add `--json` during agent exploration."));
    assert!(
        !root
            .join(".codex/skills/agent-semantic-protocols/SKILL.md")
            .exists()
    );
    let config =
        std::fs::read_to_string(root.join(".codex/config.toml")).expect("installed config");
    assert!(config.contains("# BEGIN semantic-agent-hook agent hooks"));
    assert!(!config.contains("hook_bin="));
    assert!(!config.contains(".codex/semantic-agent-hook/bin/semantic-agent-hook"));
    assert!(config.contains("exec semantic-agent-hook hook --client codex pre-tool"));
    assert!(config.contains("--profiles \"$profiles\""));
    let parsed_config =
        toml::from_str::<toml::Value>(&config).expect("installed Codex config is valid TOML");
    assert!(parsed_config.get("unified_exec").is_none());
    assert!(
        parsed_config
            .get("hooks")
            .and_then(toml::Value::as_bool)
            .is_none()
    );
    assert_eq!(
        parsed_config
            .get("features")
            .and_then(toml::Value::as_table)
            .and_then(|features| features.get("hooks"))
            .and_then(toml::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        parsed_config
            .get("features")
            .and_then(toml::Value::as_table)
            .and_then(|features| features.get("unified_exec"))
            .and_then(toml::Value::as_bool),
        Some(true)
    );
    assert!(config.matches("[hooks.state.").count() == 0);
    let user_config =
        std::fs::read_to_string(codex_home.join("config.toml")).expect("user trust config");
    let parsed_user_config =
        toml::from_str::<toml::Value>(&user_config).expect("user trust config is valid TOML");
    assert_installed_hook_state(
        &parsed_user_config,
        &std::fs::canonicalize(root.join(".codex/config.toml")).expect("canonical config path"),
    );
    let doctor = Command::new(env!("CARGO_BIN_EXE_semantic-agent-hook"))
        .env("PATH", &provider_path)
        .env("CODEX_HOME", &codex_home)
        .args([
            "doctor",
            "--client",
            "codex",
            root.to_str().expect("utf8 temp root"),
        ])
        .output()
        .expect("run semantic-agent-hook doctor");
    assert!(
        doctor.status.success(),
        "doctor stderr: {}",
        String::from_utf8_lossy(&doctor.stderr)
    );
    let doctor_stdout = String::from_utf8(doctor.stdout).expect("doctor stdout");
    assert!(doctor_stdout.contains("trust=true"));
    assert!(doctor_stdout.contains("trustConfig="));
    assert!(!doctor_stdout.contains("|trust missing="));
    assert_eq!(config.matches("matcher = \".*\"").count(), 5);
    assert!(!config.contains("fs\\\\.read"));
    assert!(!config.contains("multi_tool_use\\\\.parallel"));
    assert!(!config.contains("ts-harness agent hook --client codex"));
    assert!(!config.contains("rs-harness agent hook --client codex"));
    assert!(
        !root
            .join(".codex/semantic-agent-hook/bin/semantic-agent-hook")
            .exists()
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
fn cli_install_requires_available_provider_binary() {
    let root = temp_project_root("install-missing-provider-bin");
    let output = Command::new(env!("CARGO_BIN_EXE_semantic-agent-hook"))
        .env("PATH", "")
        .env("CODEX_HOME", root.join(".codex-home"))
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
        String::from_utf8_lossy(&output.stderr).contains("expected PATH to contain"),
        "install stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[cfg(unix)]
#[test]
fn cli_install_requires_executable_provider_binary() {
    let root = temp_project_root("install-non-executable-provider-bin");
    let provider_path = write_fake_provider_file(&root, "rs-harness", 0o644);
    let output = Command::new(env!("CARGO_BIN_EXE_semantic-agent-hook"))
        .env("PATH", &provider_path)
        .env("CODEX_HOME", root.join(".codex-home"))
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
        String::from_utf8_lossy(&output.stderr).contains("expected PATH to contain"),
        "install stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn cli_install_migrates_legacy_top_level_unified_exec_to_features() {
    let root = temp_project_root("install-unified-exec-feature");
    let codex_home = root.join(".codex-home");
    let provider_path = write_fake_provider_binary(&root, "rs-harness");
    std::fs::create_dir_all(root.join(".codex")).expect("create .codex");
    let config_path = root.join(".codex/config.toml");
    std::fs::write(
        &config_path,
        "hooks = false\nunified_exec = true\n\n[features]\nmulti_agent = true\n",
    )
    .expect("write legacy config");
    std::fs::create_dir_all(&codex_home).expect("create codex home");
    let canonical_config_path = std::fs::canonicalize(&config_path).expect("canonical config path");
    std::fs::write(
        codex_home.join("config.toml"),
        format!(
            "[hooks.state.\"{}:pre_tool_use:0:0\"]\ntrusted_hash = \"sha256:old\"\n",
            canonical_config_path.display()
        ),
    )
    .expect("write legacy user trust state");

    let output = Command::new(env!("CARGO_BIN_EXE_semantic-agent-hook"))
        .env("PATH", &provider_path)
        .env("CODEX_HOME", &codex_home)
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
    assert!(
        parsed_config
            .get("hooks")
            .and_then(toml::Value::as_bool)
            .is_none()
    );
    let features = parsed_config
        .get("features")
        .and_then(toml::Value::as_table)
        .expect("features table");
    assert_eq!(
        features.get("hooks").and_then(toml::Value::as_bool),
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
    let user_config =
        std::fs::read_to_string(codex_home.join("config.toml")).expect("user trust config");
    assert!(!user_config.contains("sha256:old"));
    let parsed_user_config =
        toml::from_str::<toml::Value>(&user_config).expect("user trust config is valid TOML");
    assert_installed_hook_state(&parsed_user_config, &canonical_config_path);
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn cli_install_writes_executable_python_ingest_route() {
    let root = temp_project_root("install-python");
    let provider_path = write_fake_provider_binary(&root, "py-harness");
    let output = Command::new(env!("CARGO_BIN_EXE_semantic-agent-hook"))
        .env("PATH", &provider_path)
        .env("CODEX_HOME", root.join(".codex-home"))
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
    let provider_path = write_fake_provider_binary(&root, "rs-harness");
    std::fs::create_dir_all(root.join(".codex")).expect("create .codex");
    let config_path = root.join(".codex/config.toml");
    std::fs::write(&config_path, "unified_exec = \"unterminated\n").expect("write invalid config");

    let output = Command::new(env!("CARGO_BIN_EXE_semantic-agent-hook"))
        .env("PATH", &provider_path)
        .env("CODEX_HOME", root.join(".codex-home"))
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

fn write_fake_provider_binary(root: &std::path::Path, binary: &str) -> PathBuf {
    write_fake_provider_file(root, binary, 0o755)
}

fn write_fake_provider_file(root: &std::path::Path, binary: &str, mode: u32) -> PathBuf {
    let bin_dir = root.join(".bin");
    std::fs::create_dir_all(&bin_dir).expect("create fake provider bin dir");
    let path = bin_dir.join(binary);
    std::fs::write(&path, "#!/bin/sh\nexit 0\n").expect("write fake provider binary");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = std::fs::metadata(&path)
            .expect("fake provider metadata")
            .permissions();
        permissions.set_mode(mode);
        std::fs::set_permissions(&path, permissions).expect("chmod fake provider");
    }
    bin_dir
}

fn assert_installed_hook_state(config: &toml::Value, config_path: &std::path::Path) {
    let state = config
        .get("hooks")
        .and_then(toml::Value::as_table)
        .and_then(|hooks| hooks.get("state"))
        .and_then(toml::Value::as_table)
        .expect("generated hook trust state");
    assert_eq!(state.len(), 8);
    let pre_tool_key = format!("{}:pre_tool_use:0:0", config_path.display());
    let pre_tool_hash = state
        .get(&pre_tool_key)
        .and_then(toml::Value::as_table)
        .and_then(|entry| entry.get("trusted_hash"))
        .and_then(toml::Value::as_str)
        .expect("pre tool use trusted hash");
    assert!(pre_tool_hash.starts_with("sha256:"));
}
