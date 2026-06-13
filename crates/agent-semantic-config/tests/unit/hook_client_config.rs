use std::fs;
use std::path::{Path, PathBuf};

use super::{
    CLIENT_HOOK_CONFIG_SCHEMA_ID, default_hook_client_config_path,
    default_hook_client_config_template, load_hook_client_config_file,
};

#[test]
fn default_template_round_trips_through_config_parser() {
    let root = temp_root("hook-client-template");
    let config_path = default_hook_client_config_path(&root);
    fs::create_dir_all(config_path.parent().expect("config parent")).expect("config dir");
    fs::write(&config_path, default_hook_client_config_template()).expect("write config");

    let config = load_hook_client_config_file(&config_path).expect("load config");

    assert_eq!(
        config.schema_id.as_deref(),
        Some(CLIENT_HOOK_CONFIG_SCHEMA_ID)
    );
    assert_eq!(
        config
            .experimental
            .get("semanticAstPatch")
            .and_then(|feature| feature.get("enabled")),
        Some(&false)
    );
    assert_eq!(config.rules.len(), 1);
    let rule = config.rules.first().expect("default rule");
    assert_eq!(rule.id, "deny-shell-source-argv");
    assert_eq!(rule.match_config.command_any, ["sed", "perl", "rg", "wl"]);
    assert!(
        rule.match_config
            .argv_source_glob_any
            .iter()
            .any(|glob| glob == "**/*.rs")
    );
    assert_eq!(
        rule.match_config.argv_source_exclude_flag_any,
        ["--output", "--output-file", "--out", "-o"]
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn missing_config_loads_empty_defaults() {
    let root = temp_root("hook-client-missing");
    let config = load_hook_client_config_file(&root.join("missing.toml")).expect("missing config");

    assert!(config.rules.is_empty());
    assert!(config.experimental.is_empty());
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
    assert_eq!(config.rules.len(), 1);
    assert_eq!(config.rules[0].id, "deny-rust-read");

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
kind = "legacy-alias"
argv = ["asp", "rust"]
"#,
    )
    .expect("write config");

    let error = load_hook_client_config_file(&config_path).expect_err("invalid route kind");

    assert!(error.contains("legacy-alias"), "{error}");
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
