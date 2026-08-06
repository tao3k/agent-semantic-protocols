use super::{
    ASP_CODEX_PLUGIN_HOOKS_JSON, ASP_CODEX_PLUGIN_MANIFEST_JSON, ASP_CODEX_PLUGIN_MARKETPLACE_JSON,
    remove_codex_managed_global_hook_config, validate_codex_plugin_source_payload,
};
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
    assert_eq!(
        manifest.get("hooks").and_then(serde_json::Value::as_str),
        Some("./hooks/hooks.json")
    );
    let marketplace: serde_json::Value = serde_json::from_str(ASP_CODEX_PLUGIN_MARKETPLACE_JSON)
        .expect("valid marketplace manifest");
    assert_eq!(
        marketplace["plugins"][0]["source"]["path"].as_str(),
        Some("./asp-codex-plugin")
    );
}

#[test]
fn plugin_hook_shape_uses_internal_action_classification() {
    let hooks: serde_json::Value =
        serde_json::from_str(ASP_CODEX_PLUGIN_HOOKS_JSON).expect("valid plugin hooks");
    assert_eq!(
        hooks["hooks"]["PreToolUse"][0]["matcher"].as_str(),
        Some("*")
    );
    assert_eq!(
        hooks["hooks"]["PreToolUse"][0]["hooks"][0]["timeout"].as_u64(),
        Some(1)
    );
}

#[test]
fn production_cleanup_is_idempotent_and_preserves_plugin_enablement() {
    let root = unique_plugin_test_directory("cleanup");
    std::fs::create_dir_all(&root).expect("create isolated plugin fixture");
    let config_path = root.join("config.toml");
    let project_config_path = root.join("project.toml");
    let inline = agent_semantic_hook::codex_global_hook_block_with_binary(None);
    let original = format!(
        "[plugins.\"asp-codex-plugin@asp-project\"]\nenabled = true\n\n[hooks.state.\"asp-codex-plugin@asp-project:hooks/hooks.json:pre_tool_use:0:0\"]\ntrusted_hash = \"authorized\"\n\n{inline}\n"
    );
    std::fs::write(&config_path, original).expect("write isolated Codex config");

    let first = remove_codex_managed_global_hook_config(&config_path, &project_config_path)
        .expect("remove inline Hook");
    let second = remove_codex_managed_global_hook_config(&config_path, &project_config_path)
        .expect("repeat inline Hook cleanup");
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
    let project_config_path = root.join("project.toml");
    let invalid = "[plugins\nenabled = true\n";
    std::fs::write(&config_path, invalid).expect("write invalid isolated config");

    assert!(remove_codex_managed_global_hook_config(&config_path, &project_config_path,).is_err());
    assert_eq!(
        std::fs::read_to_string(&config_path).expect("read unchanged invalid config"),
        invalid
    );
    std::fs::remove_dir_all(&root).expect("remove isolated plugin fixture");
}
