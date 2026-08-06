use std::fs;
use std::path::{Path, PathBuf};

use agent_semantic_config::{
    CLIENT_HOOK_CONFIG_SCHEMA_ID, HookClientConfigFile, HookClientResidentAgentConfig,
    agent_route_registry::render_hook_agent_routes, default_hook_client_config_file,
    default_hook_client_config_template, hook_client_contract_fingerprint,
    load_asp_project_config_file, load_hook_client_config_file, merge_asp_project_hook_config,
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

#[path = "hook_client_config/parsing.rs"]
mod parsing;
#[path = "hook_client_config/validation.rs"]
mod validation;

fn write_canonical_config_overlay(path: &std::path::Path, overlay: &str) {
    let mut config = toml::from_str::<toml::Value>(&projected_default_template())
        .expect("parse canonical hook config");
    let overlay = toml::from_str::<toml::Value>(overlay).expect("parse hook config overlay");
    merge_toml_value(&mut config, overlay);
    fs::write(
        path,
        toml::to_string_pretty(&config).expect("render hook config overlay"),
    )
    .expect("write hook config overlay");
}

fn projected_default_template() -> String {
    let mut config = toml::from_str::<toml::Value>(&default_hook_client_config_template())
        .expect("parse default hook config template");
    let projection = toml::from_str::<toml::Value>(&projected_agent_routes())
        .expect("parse project agent route projection");
    config["agents"] = projection["agents"].clone();
    toml::to_string_pretty(&config).expect("render projected default hook config")
}

fn projected_agent_routes() -> String {
    render_hook_agent_routes(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("agents"),
    )
    .expect("render canonical agent route registry")
}

fn merge_toml_value(base: &mut toml::Value, overlay: toml::Value) {
    match (base, overlay) {
        (toml::Value::Table(base), toml::Value::Table(overlay)) => {
            for (key, value) in overlay {
                if let Some(existing) = base.get_mut(&key) {
                    merge_toml_value(existing, value);
                } else {
                    base.insert(key, value);
                }
            }
        }
        (base, overlay) => *base = overlay,
    }
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
