// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::fs;
use std::path::Path;
use std::path::PathBuf;

use agent_semantic_config::CLIENT_HOOK_CONFIG_SCHEMA_ID;
use agent_semantic_config::default_hook_client_config_file;
use agent_semantic_config::default_hook_client_config_template;
use agent_semantic_config::hook_client_contract_fingerprint;
use agent_semantic_config::load_asp_project_config_file;
use agent_semantic_config::load_hook_client_config_file;
use agent_semantic_config::merge_asp_project_hook_config;

#[path = "hook_client_config/parsing.rs"]
mod parsing;
#[path = "hook_client_config/validation.rs"]
mod validation;

fn write_canonical_config_overlay(path: &std::path::Path, overlay: &str) {
    let mut config = toml::from_str::<toml::Value>(&canonical_default_template())
        .expect("parse canonical hook config");
    let overlay = toml::from_str::<toml::Value>(overlay).expect("parse hook config overlay");
    merge_toml_value(&mut config, overlay);
    fs::write(
        path,
        toml::to_string_pretty(&config).expect("render hook config overlay"),
    )
    .expect("write hook config overlay");
}

fn canonical_default_template() -> String {
    default_hook_client_config_template()
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
