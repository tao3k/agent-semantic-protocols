use agent_semantic_hook::{
    ROOT_BLOCK_BEGIN, ROOT_BLOCK_END, claude_hook_block, codex_hook_block, merge_claude_settings,
    merge_codex_asp_explorer_role_config, remove_codex_managed_hook_blocks,
};
use std::path::Path;

const PROJECT_ROOT: &str = "/workspace/agent-semantic-protocols";

#[test]
fn codex_hook_matcher_omits_apply_patch_surfaces_by_default() {
    let block = codex_hook_block(Path::new(PROJECT_ROOT));

    assert!(block.contains(PROJECT_ROOT));
    assert!(block.contains("readFile"));
    assert!(block.contains("FsReadFile"));
    assert!(block.contains("fs/readFile"));
    assert!(block.contains("functions\\\\.exec_command"));
    assert!(block.contains("multi_tool_use\\\\.parallel"));
    assert!(!block.contains("apply_patch"));
    assert!(!block.contains("applypatch"));
    assert!(!block.contains("functions\\\\.apply_patch"));
    assert!(!block.contains("FsWriteFile"));
    assert!(!block.contains("functions\\\\.write"));
    assert!(!block.contains("matcher = \".*\""));
}

#[test]
fn codex_plugin_cleanup_removes_managed_hook_blocks_without_dropping_config() {
    let existing = format!(
        r#"[features]
model_context_protocol = true

{ROOT_BLOCK_BEGIN}
[[hooks.PreToolUse]]
matcher = "Bash"
{ROOT_BLOCK_END}

[profiles.default]
model = "gpt-5"
"#
    );

    let cleaned = remove_codex_managed_hook_blocks(&existing);

    assert!(!cleaned.contains(ROOT_BLOCK_BEGIN), "{cleaned}");
    assert!(!cleaned.contains(ROOT_BLOCK_END), "{cleaned}");
    assert!(cleaned.contains("model_context_protocol = true"));
    assert!(cleaned.contains("[profiles.default]"));
    assert!(cleaned.contains("model = \"gpt-5\""));
}

#[test]
fn codex_plugin_role_merge_registers_asp_explorer_without_hook_block() {
    let existing = r#"[features]
hooks = true
unified_exec = true

[marketplaces.asp-project]
source_type = "local"
source = "."
"#;

    let merged = merge_codex_asp_explorer_role_config(existing).expect("merge role config");

    toml::from_str::<toml::Value>(&merged).expect("merged Codex config is valid TOML");
    assert!(!merged.contains(ROOT_BLOCK_BEGIN), "{merged}");
    assert!(merged.contains("[agents.asp_explorer]"));
    assert!(merged.contains("config_file = \"agents/asp-explorer.toml\""));
    assert!(merged.contains("[marketplaces.asp-project]"));
    assert!(merged.contains("source = \".\""));

    let merged_again =
        merge_codex_asp_explorer_role_config(&merged).expect("merge existing role config");
    assert_eq!(merged_again, merged);
}

#[test]
fn claude_hook_matcher_reuses_shared_tool_surfaces() {
    let block = claude_hook_block(Path::new(PROJECT_ROOT));
    let pre_tool = block["hooks"]["PreToolUse"][0]["matcher"]
        .as_str()
        .expect("pre-tool matcher");

    assert_ne!(pre_tool, "*");
    assert!(block.to_string().contains(PROJECT_ROOT));
    assert!(pre_tool.contains("Bash|Shell"));
    assert!(pre_tool.contains("functions\\.exec_command"));
    assert!(!pre_tool.contains("apply_patch"));
    assert!(!pre_tool.contains("functions\\.write"));
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
    assert!(!merged.contains(r#""matcher": "*""#));
    assert!(merged.contains("exec asp hook pre-tool --client claude"));
    assert!(merged.contains(PROJECT_ROOT));
    assert!(!merged.contains("ASP_HOOK_PROJECT_ROOT"));
}
