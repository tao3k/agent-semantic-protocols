use super::ASP_CODEX_PLUGIN_HOOKS_JSON;
use super::ASP_CODEX_PLUGIN_MANIFEST_JSON;
use super::ASP_CODEX_PLUGIN_MARKETPLACE_JSON;
use super::remove_codex_managed_global_hook_config;
use super::validate_codex_plugin_source_payload;
use std::path::PathBuf;

fn unique_plugin_test_directory(label: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock must be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "asp-canonical-plugin-{label}-{}-{nonce}",
        std::process::id()
    ))
}

#[test]
fn canonical_plugin_payload_is_marketplace_owned_and_complete() {
    let digest = validate_codex_plugin_source_payload().expect("validate canonical plugin payload");
    assert_eq!(digest.len(), 64);

    let manifest: serde_json::Value =
        serde_json::from_str(ASP_CODEX_PLUGIN_MANIFEST_JSON).expect("valid plugin manifest");
    assert!(manifest.get("hooks").is_none());
    assert!(manifest.get("skills").is_none());
    let marketplace: serde_json::Value = serde_json::from_str(ASP_CODEX_PLUGIN_MARKETPLACE_JSON)
        .expect("valid marketplace manifest");
    assert_eq!(
        marketplace["plugins"][0]["source"]["path"].as_str(),
        Some("./asp-codex-plugin")
    );
}

#[test]
fn plugin_hook_shape_uses_one_entry_per_native_action_family() {
    let hooks: serde_json::Value =
        serde_json::from_str(ASP_CODEX_PLUGIN_HOOKS_JSON).expect("valid plugin hooks");
    let pre_tool_use = hooks["hooks"]["PreToolUse"]
        .as_array()
        .expect("PreToolUse action entries");
    let matchers = pre_tool_use
        .iter()
        .map(|entry| entry["matcher"].as_str().expect("action matcher"))
        .collect::<Vec<_>>();

    assert_eq!(&matchers[..3], ["^apply_patch$", "Bash", "spawn_agent"]);
    assert!(
        matchers[3..]
            .iter()
            .all(|matcher| matcher.starts_with("mcp__codex_app__"))
    );
    assert!(!matchers.contains(&"^mcp__.*$"));
    assert_eq!(
        matchers
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        matchers.len(),
        "each native Host tool has one physical matcher"
    );
    assert!(!matchers.contains(&"*"));
    assert!(pre_tool_use.iter().all(|entry| {
        entry["hooks"].as_array().is_some_and(|handlers| {
            handlers.len() == 1 && handlers[0]["timeout"].as_u64() == Some(1)
        })
    }));
    let commands = pre_tool_use
        .iter()
        .map(|entry| {
            entry["hooks"][0]["command"]
                .as_str()
                .expect("typed command")
        })
        .collect::<Vec<_>>();
    for expected in [
        "--host-match apply_patch",
        "--host-match Bash",
        "--host-match spawn_agent",
        "--host-match mcp__codex_app__send_message_to_thread",
    ] {
        assert!(
            commands.iter().any(|command| command.ends_with(expected)),
            "{expected}"
        );
    }
}

#[test]
fn production_cleanup_is_idempotent_and_preserves_plugin_enablement() {
    let root = unique_plugin_test_directory("cleanup");
    std::fs::create_dir_all(&root).expect("create isolated plugin fixture");
    let config_path = root.join("config.toml");
    let inline = agent_semantic_hook::codex_global_hook_block_with_binary(None);
    let original = format!(
        "[plugins.\"asp-codex-plugin@asp-project\"]\nenabled = true\n\n[hooks.state.\"asp-codex-plugin@asp-project:hooks/hooks.json:pre_tool_use:0:0\"]\ntrusted_hash = \"authorized\"\n\n{inline}\n"
    );
    std::fs::write(&config_path, original).expect("write isolated Codex config");

    let first = remove_codex_managed_global_hook_config(&config_path).expect("remove inline Hook");
    let second =
        remove_codex_managed_global_hook_config(&config_path).expect("repeat inline Hook cleanup");
    let cleaned = std::fs::read_to_string(&config_path).expect("read cleaned Codex config");
    assert!(first.changed);
    assert!(!second.changed);
    assert!(cleaned.contains("asp-codex-plugin@asp-project"));
    assert!(cleaned.contains("trusted_hash = \"authorized\""));
    assert!(!cleaned.contains(agent_semantic_hook::ROOT_BLOCK_BEGIN));
    std::fs::remove_dir_all(&root).expect("remove isolated plugin fixture");
}

#[test]
fn invalid_global_config_is_never_replaced() {
    let root = unique_plugin_test_directory("invalid");
    std::fs::create_dir_all(&root).expect("create isolated plugin fixture");
    let config_path = root.join("config.toml");
    let invalid = "[plugins\nenabled = true\n";
    std::fs::write(&config_path, invalid).expect("write invalid isolated config");

    assert!(remove_codex_managed_global_hook_config(&config_path).is_err());
    assert_eq!(
        std::fs::read_to_string(&config_path).expect("read unchanged invalid config"),
        invalid
    );
    std::fs::remove_dir_all(&root).expect("remove isolated plugin fixture");
}
