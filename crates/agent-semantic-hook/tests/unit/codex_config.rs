use agent_semantic_hook::{
    ROOT_BLOCK_BEGIN, claude_hook_block, codex_hook_block, merge_claude_settings,
};
use agent_semantic_runtime::state_core::resolve_state_home;
use std::path::Path;

const PROJECT_ROOT: &str = "/workspace/agent-semantic-protocols";

#[test]
fn codex_hook_matcher_dispatches_every_tool_action_to_internal_projection() {
    let _state_home_lock = crate::test_process_env::ASP_STATE_HOME_ENV_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let block = codex_hook_block(Path::new(PROJECT_ROOT));
    let state_home_asp = resolve_state_home()
        .expect("State Home")
        .join("runtime")
        .join("bin")
        .join("asp");

    assert!(block.contains(PROJECT_ROOT));
    assert!(block.contains(&state_home_asp.display().to_string()));
    assert!(!block.contains("$repo_root/.bin/asp"));
    assert!(block.contains("[[hooks.PreToolUse]]"));
    assert!(block.contains("[[hooks.PermissionRequest]]"));
    assert!(block.contains("matcher = \"*\""));
    assert!(!block.contains("[[hooks.pre_tool_use]]"));
}

#[test]
fn codex_hook_merge_replaces_legacy_bare_asp_explorer_role() {
    let _state_home_lock = crate::test_process_env::ASP_STATE_HOME_ENV_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let existing = r#"[features]
hooks = true
plugins = true

[agents.asp_explorer]
description = "legacy"
config_file = "agents/legacy.toml"
nickname_candidates = ["ASP selector"]

[marketplaces.asp-project]
source_type = "local"
source = "/tmp/asp-project"
"#;

    let merged =
        agent_semantic_hook::merge_codex_config(existing, &codex_hook_block(PROJECT_ROOT.as_ref()));

    toml::from_str::<toml::Value>(&merged).expect("merged Codex config is valid TOML");
    assert!(merged.contains(ROOT_BLOCK_BEGIN), "{merged}");
    assert_eq!(
        merged.matches("[agents.asp_explorer]").count(),
        0,
        "{merged}"
    );
    assert!(!merged.contains("config_file = \"agents/asp-explorer.toml\""));
    assert!(!merged.contains("agents/legacy.toml"));
    assert!(merged.contains("[marketplaces.asp-project]"));
    assert!(merged.contains("[[hooks.PreToolUse]]"), "{merged}");
    assert!(merged.contains("[[hooks.SessionStart]]"), "{merged}");
    assert!(merged.contains("[[hooks.PermissionRequest]]"), "{merged}");
    assert!(!merged.contains("[[hooks.pre_tool_use]]"), "{merged}");
}

#[test]
fn codex_hook_trust_cleanup_removes_orphan_state_tables() {
    let existing = r#"[features]
plugins = true

[hooks.state."/tmp/project/.codex/config.toml:pre_tool_use:0:0"]
status = "approved"
hash = "legacy"

# END agent-semantic-protocol trusted hook state

[plugins."asp-codex-plugin@asp-project"]
enabled = true
"#;

    let cleaned = agent_semantic_hook::remove_codex_global_hook_trust_config(
        existing,
        std::path::Path::new("/tmp/project/.codex/config.toml"),
    );

    toml::from_str::<toml::Value>(&cleaned).expect("cleaned Codex config is valid TOML");
    assert!(!cleaned.contains("[hooks.state."));
    assert!(!cleaned.contains("legacy"));
    assert!(!cleaned.contains("# END agent-semantic-protocol trusted hook state"));
    assert!(cleaned.contains("[plugins.\"asp-codex-plugin@asp-project\"]"));
}

#[test]
fn claude_hook_matcher_also_dispatches_every_tool_action() {
    let block = claude_hook_block(Path::new(PROJECT_ROOT));
    let pre_tool = block["hooks"]["PreToolUse"][0]["matcher"]
        .as_str()
        .expect("pre-tool matcher");

    assert_eq!(pre_tool, "*");
    assert!(block.to_string().contains(PROJECT_ROOT));
    assert!(block["hooks"].get("PermissionRequest").is_none());
    assert_eq!(block["hooks"]["PostToolUse"][0]["matcher"], pre_tool);
    assert!(!block.to_string().contains("ASP_HOOK_PROJECT_ROOT"));
}

#[test]
fn claude_settings_merge_preserves_unmanaged_hooks_and_replaces_managed_hooks() {
    let existing = r#"{
      "hooks": {
        "PreToolUse": [
          {"matcher": "Bash", "hooks": [{"type": "command", "command": "echo keep"}]},
          {"matcher": "*", "hooks": [{"type": "command", "command": "asp hook pre-tool --client claude --old"}]}
        ],
        "PermissionRequest": [
          {"matcher": "*", "hooks": [{"type": "command", "command": "asp hook permission-request --client claude --old"}]}
        ]
      }
    }"#;
    let merged =
        merge_claude_settings(existing, &claude_hook_block(Path::new(PROJECT_ROOT))).unwrap();
    let value: serde_json::Value = serde_json::from_str(&merged).unwrap();
    let pre_tool = value["hooks"]["PreToolUse"].as_array().unwrap();
    assert_eq!(pre_tool.len(), 2);
    assert!(merged.contains("echo keep"));
    assert!(!merged.contains("--old"));
    assert!(value["hooks"].get("PermissionRequest").is_none());
    assert!(merged.contains(r#""matcher": "*""#));
    assert!(merged.contains("exec asp hook pre-tool --client claude"));
    assert!(merged.contains(PROJECT_ROOT));
    assert!(!merged.contains("ASP_HOOK_PROJECT_ROOT"));
}
