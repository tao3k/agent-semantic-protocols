use agent_semantic_hook::{
    ROOT_BLOCK_BEGIN, claude_hook_block, codex_hook_block, merge_claude_settings,
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
fn codex_hook_merge_replaces_legacy_bare_asp_explorer_role() {
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
    assert!(merged.contains("[[hooks.pre_tool_use]]"), "{merged}");
    assert!(merged.contains("[[hooks.session_start]]"), "{merged}");
    assert!(merged.contains("[[hooks.permission_request]]"), "{merged}");
    assert!(!merged.contains("[[hooks.PreToolUse]]"), "{merged}");
    assert!(!merged.contains("[[hooks.SessionStart]]"), "{merged}");
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
