use super::{
    ASP_CODEX_PLUGIN_HOOKS_JSON, ASP_CODEX_PLUGIN_MANIFEST_JSON, ASP_CODEX_PLUGIN_MARKETPLACE_JSON,
    remove_codex_managed_global_hook_config,
};
use std::path::PathBuf;

fn temp_root(label: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock must be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "asp-plugin-hook-{label}-{}-{nonce}",
        std::process::id()
    ))
}

#[test]
fn bundled_manifest_declares_plugin_hook_authority() {
    let manifest: serde_json::Value =
        serde_json::from_str(ASP_CODEX_PLUGIN_MANIFEST_JSON).expect("valid plugin manifest");
    assert_eq!(
        manifest.get("hooks").and_then(serde_json::Value::as_str),
        Some("./hooks/hooks.json")
    );

    let hooks: serde_json::Value =
        serde_json::from_str(ASP_CODEX_PLUGIN_HOOKS_JSON).expect("valid plugin hooks");
    let events = hooks
        .get("hooks")
        .and_then(serde_json::Value::as_object)
        .expect("plugin hook event map");
    assert_eq!(events.len(), 8);
    for event in ["PreToolUse", "PermissionRequest", "PostToolUse"] {
        let groups = events[event]
            .as_array()
            .unwrap_or_else(|| panic!("plugin Hook event {event} must contain matcher groups"));
        assert!(
            groups.iter().all(|group| matches!(
                group.get("matcher").and_then(serde_json::Value::as_str),
                None | Some("") | Some("*")
            )),
            "plugin Hook event {event} must use a Codex match-all matcher (`*`, empty, or omitted) so Read, MCP, apply_patch, and Bash all reach the internal action classifier: encodedMatcher={} actual={groups:?}",
            serde_json::to_string(&groups[0]["matcher"]).expect("encode matcher diagnostic")
        );
    }
    for (event, groups) in events {
        let groups = groups
            .as_array()
            .unwrap_or_else(|| panic!("plugin Hook event {event} must contain matcher groups"));
        for group in groups {
            let handlers = group["hooks"]
                .as_array()
                .unwrap_or_else(|| panic!("plugin Hook event {event} must contain handlers"));
            for handler in handlers {
                assert_eq!(
                    handler["type"].as_str(),
                    Some("command"),
                    "plugin Hook event {event} must use the command transport"
                );
                assert_eq!(
                    handler["timeout"].as_u64(),
                    Some(1),
                    "plugin Hook event {event} must keep the host hard deadline at one second"
                );
                let suffix = match event.as_str() {
                    "SessionStart" => "session-start",
                    "UserPromptSubmit" => "user-prompt",
                    "PreToolUse" => "pre-tool",
                    "PermissionRequest" => "permission-request",
                    "PostToolUse" => "post-tool",
                    "SubagentStart" => "subagent-start",
                    "SubagentStop" => "subagent-stop",
                    "Stop" => "stop",
                    _ => panic!("unexpected plugin event {event}"),
                };
                assert_eq!(
                    handler["command"].as_str(),
                    Some(format!("asp hook {suffix} --client codex").as_str())
                );
            }
        }
    }

    let marketplace: serde_json::Value = serde_json::from_str(ASP_CODEX_PLUGIN_MARKETPLACE_JSON)
        .expect("valid repo marketplace manifest");
    assert_eq!(
        marketplace["plugins"][0]["source"]["path"].as_str(),
        Some("./asp-codex-plugin")
    );
}

#[test]
fn production_cleanup_removes_inline_hook_without_touching_other_config() {
    let root = temp_root("cleanup");
    std::fs::create_dir_all(&root).expect("create isolated config fixture");
    let config_path = root.join("config.toml");
    let project_config_path = root.join("project.toml");
    let inline = agent_semantic_hook::codex_global_hook_block_with_binary(None);
    std::fs::write(&config_path, format!("model = \"gpt-test\"\n\n{inline}\n"))
        .expect("write isolated config fixture");

    let receipt = remove_codex_managed_global_hook_config(&config_path, &project_config_path)
        .expect("remove native inline Hook injection");
    let cleaned = std::fs::read_to_string(&config_path).expect("read cleaned config");
    assert!(receipt.changed);
    assert!(cleaned.contains("model = \"gpt-test\""));
    assert!(!cleaned.contains(agent_semantic_hook::ROOT_BLOCK_BEGIN));
    assert!(!cleaned.contains("[[hooks.PreToolUse]]"));
    std::fs::remove_dir_all(&root).expect("remove isolated config fixture");
}
