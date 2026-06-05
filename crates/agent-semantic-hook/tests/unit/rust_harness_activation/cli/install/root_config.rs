use std::env;

use agent_semantic_hook::parse_hook_activation;

use crate::rust_harness_activation::support::write_fake_provider_binary;

use super::support::{assert_installed_hook_state, git_project_root, protocol_command};

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
    let canonical_config =
        std::fs::canonicalize(root.join(".codex/config.toml")).expect("canonical config path");
    write_legacy_codex_user_trust_state(&codex_home, &canonical_config);

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
    assert_profile_registry(&root);
    let config =
        std::fs::read_to_string(root.join(".codex/config.toml")).expect("installed config");
    assert_codex_config(&config);
    assert_client_config(&root);
    let parsed_config =
        toml::from_str::<toml::Value>(&config).expect("installed Codex config is valid TOML");
    assert_enabled_features(&parsed_config);
    let user_config =
        std::fs::read_to_string(codex_home.join("config.toml")).expect("user trust config");
    assert!(!user_config.contains("semantic-agent-hook trusted hook state"));
    assert!(!user_config.contains("stale-sandbox"));
    assert!(user_config.contains("agent-semantic-protocol trusted hook state"));
    let parsed_user_config =
        toml::from_str::<toml::Value>(&user_config).expect("user trust config is valid TOML");
    assert_installed_hook_state(&parsed_user_config, &canonical_config);
    assert_doctor(&root, &path, &codex_home);
    assert_installed_activation(&root);
    let _ = std::fs::remove_dir_all(&root);
}

fn write_legacy_codex_user_trust_state(
    codex_home: &std::path::Path,
    config_path: &std::path::Path,
) {
    std::fs::create_dir_all(codex_home).expect("create fake Codex home");
    let stale_config_path = codex_home.join("stale-sandbox/.codex/config.toml");
    std::fs::write(
        codex_home.join("config.toml"),
        format!(
            r#"# BEGIN semantic-agent-hook trusted hook state: {config_path}
[hooks.state."{config_path}:PreToolUse"]
trusted = true
ask = false
# END semantic-agent-hook trusted hook state

# BEGIN agent-semantic-protocol trusted hook state: {stale_config_path}
[hooks.state."{stale_config_path}:pre_tool_use:0:0"]
trusted_hash = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
# END agent-semantic-protocol trusted hook state
"#,
            config_path = config_path.display(),
            stale_config_path = stale_config_path.display()
        ),
    )
    .expect("write legacy user trust state");
}

fn write_legacy_hook_cache(root: &std::path::Path) {
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
    assert!(stdout.contains("activation=.cache/agent-semantic-protocol/hooks/activation.json"));
    assert!(stdout.contains("clientConfig=.codex/agent-semantic-protocol/hooks/config.toml"));
    assert!(stdout.contains("profileCache=.cache/agent-semantic-protocol/hooks/profiles.json"));
    assert!(stdout.contains("skill=.agents/skills/agent-semantic-protocols/SKILL.md"));
    assert!(stdout.contains("trustConfig="));
    assert!(stdout.contains("binary=asp"));
    assert!(stdout.contains("binaryInstall=installed"));
    assert!(stdout.contains("binaryPath="));
}

fn assert_installed_skill(root: &std::path::Path) {
    let skill =
        std::fs::read_to_string(root.join(".agents/skills/agent-semantic-protocols/SKILL.md"))
            .expect("installed agent skill");
    assert!(skill.contains("Generated by `asp hook install`"));
    assert!(skill.contains("Do not edit this installed copy"));
    assert!(skill.contains("## Active Providers"));
    assert!(skill.contains("| rust | `asp rust` | rs-harness | `"));
    assert!(skill.contains("/.bin/rs-harness` |"));
    assert!(skill.contains("Start with `asp <language> guide .`"));
    assert!(!skill.contains("Start with `asp <language> agent guide .`"));
    assert!(!skill.contains("`asp typescript`"));
    assert!(!skill.contains("`asp python`"));
    assert!(!skill.contains("`asp julia`"));
    assert!(!skill.contains("Julia participates in language facade parity"));
    assert!(skill.contains("Do not add `--json` during agent exploration."));
    assert!(skill.contains("single-quoted argv literal"));
    assert!(skill.contains("--query-set 'Start with `asp <language> guide .`'"));
    assert!(skill.contains("do not interpolate raw prose into a"));
    assert!(skill.contains("## Complex Flows"));
    assert!(skill.contains("### Hook Recovery"));
}

fn assert_profile_registry(root: &std::path::Path) {
    let profiles_text =
        std::fs::read_to_string(root.join(".cache/agent-semantic-protocol/hooks/profiles.json"))
            .expect("installed profile registry");
    let profiles: serde_json::Value =
        serde_json::from_str(&profiles_text).expect("profile registry JSON");
    assert_eq!(
        profiles["schemaId"],
        "agent.semantic-protocols.hook.profile-registry"
    );
    assert_eq!(profiles["projectRoot"], ".");
    let profile_entries = profiles["profiles"].as_array().expect("profile entries");
    assert_eq!(profile_entries.len(), 1);
    assert_eq!(profile_entries[0]["providerId"], "rs-harness");
    let query_argv = profile_entries[0]["commands"]["query"]["argv"]
        .as_array()
        .expect("query argv");
    assert!(
        query_argv[0]
            .as_str()
            .is_some_and(|program| program.ends_with("/.bin/rs-harness")),
        "{query_argv:?}"
    );
    assert_eq!(
        &query_argv[1..],
        serde_json::json!([
            "query",
            "--from-hook",
            "direct-source-read",
            "--selector",
            "{selector}",
            "{termArgs}",
            "--surface",
            "owners,tests",
            "--view",
            "seeds",
            "."
        ])
        .as_array()
        .expect("expected query argv tail")
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

fn assert_codex_config(config: &str) {
    assert!(config.contains("# BEGIN agent-semantic-protocol agent hooks"));
    assert!(!config.contains("# BEGIN semantic-agent-protocol agent hooks"));
    assert!(!config.contains("# BEGIN agent-semantic-hook agent hooks"));
    assert!(!config.contains("hook_bin="));
    assert!(!config.contains("exec semantic-agent-protocol"));
    assert!(!config.contains("exec agent-semantic-hook"));
    assert!(!config.contains(".codex/agent-semantic-hook/bin/agent-semantic-hook"));
    assert!(config.contains("exec asp hook pre-tool --client codex"));
    assert!(config.contains(
        "activation=\"$repo_root/.cache/agent-semantic-protocol/hooks/activation.json\""
    ));
    assert!(
        config.contains("config=\"$repo_root/.codex/agent-semantic-protocol/hooks/config.toml\"")
    );
    assert!(config.contains("--config \"$config\""));
    assert!(!config.contains("asp hook --client codex"));
    for event in [
        "session-start",
        "user-prompt",
        "pre-tool",
        "permission-request",
        "post-tool",
        "subagent-start",
        "subagent-stop",
        "stop",
    ] {
        assert!(
            config.contains(&format!("exec asp hook {event} --client codex")),
            "missing protocol hook command for {event}"
        );
    }
    assert!(config.contains("--activation \"$activation\""));
    assert!(config.matches("[hooks.state.").count() == 0);
    assert!(!config.contains("matcher = \".*\""));
    assert_eq!(config.matches("functions\\\\.exec_command").count(), 5);
    assert!(config.contains("Read|read|readFile"));
    assert!(config.contains("read_file"));
    assert!(config.contains("functions\\\\.read"));
    assert!(config.contains("functions\\\\.read_file"));
    assert!(config.contains("mcp__.*__read_file"));
    assert!(config.contains("multi_tool_use\\\\.parallel"));
    assert!(config.contains("Bash|Shell"));
    assert!(config.contains("fs\\\\.read"));
    assert!(!config.contains("ts-harness agent hook --client codex"));
    assert!(!config.contains("rs-harness agent hook --client codex"));
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
    assert!(client_config.contains("# [[rules]]"));
    assert!(!client_config.contains("\n[[rules]]"));
    toml::from_str::<toml::Value>(&client_config).expect("client hook config is valid TOML");
}

fn assert_enabled_features(parsed_config: &toml::Value) {
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
}

fn assert_doctor(root: &std::path::Path, path: &std::ffi::OsString, codex_home: &std::path::Path) {
    let doctor = protocol_command()
        .env("PATH", path)
        .env("CODEX_HOME", codex_home)
        .args([
            "hook",
            "doctor",
            "--client",
            "codex",
            root.to_str().expect("utf8 temp root"),
        ])
        .output()
        .expect("run agent-semantic-protocol doctor");
    assert!(
        doctor.status.success(),
        "doctor stderr: {}",
        String::from_utf8_lossy(&doctor.stderr)
    );
    let doctor_stdout = String::from_utf8(doctor.stdout).expect("doctor stdout");
    assert!(doctor_stdout.contains("trust=true"));
    assert!(doctor_stdout.contains("trustConfig="));
    assert!(doctor_stdout.contains("binary=true"));
    assert!(doctor_stdout.contains("binaryPath="));
    assert!(!doctor_stdout.contains("|trust missing="));
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
    assert_eq!(registry.providers.len(), 1);
    assert_eq!(registry.providers[0].language_id, "rust");
    assert_eq!(
        registry.providers[0].routes.fzf.argv,
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
