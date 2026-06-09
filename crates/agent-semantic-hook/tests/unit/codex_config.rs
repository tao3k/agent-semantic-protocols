use agent_semantic_hook::{claude_hook_block, codex_hook_block, merge_claude_settings};
use std::path::Path;

const PROJECT_ROOT: &str = "/workspace/agent-semantic-protocols";

#[test]
fn codex_hook_matcher_includes_apply_patch_surfaces() {
    let block = codex_hook_block(Path::new(PROJECT_ROOT));

    assert!(block.contains("apply_patch|applypatch"));
    assert!(block.contains(PROJECT_ROOT));
    assert!(block.contains("functions\\\\.apply_patch"));
    assert!(block.contains("readFile"));
    assert!(block.contains("FsReadFile"));
    assert!(block.contains("fs/readFile"));
    assert!(block.contains("FsWriteFile"));
    assert!(block.contains("functions\\\\.exec_command"));
    assert!(block.contains("multi_tool_use\\\\.parallel"));
    assert!(!block.contains("matcher = \".*\""));
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
