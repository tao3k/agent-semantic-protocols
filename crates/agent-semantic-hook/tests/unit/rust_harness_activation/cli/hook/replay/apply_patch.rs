use serde_json::json;

use crate::rust_harness_activation::support::{stage_project_candidates, temp_project_root};

use crate::rust_harness_activation::cli::hook::support::{last_hook_event, run_hook_decision};

#[test]
fn cli_hook_replay_blocks_source_apply_patch() {
    let root = temp_project_root("hook-source-apply-patch");
    std::fs::create_dir_all(root.join("src")).expect("create Rust source root");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"hook-source-apply-patch\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("write Rust package entry");
    std::fs::write(root.join("src/lib.rs"), "pub fn old() {}\n")
        .expect("write registered Rust source");
    stage_project_candidates(&root);
    let command = r#"apply_patch <<'PATCH'
*** Begin Patch
*** Update File: src/lib.rs
@@
-pub fn old() {}
+pub fn new() {}
*** End Patch
PATCH"#;
    let decision = run_hook_decision(
        &root,
        "pre-tool",
        json!({ "tool_name": "functions.exec_command", "tool_input": { "cmd": command } }),
    );
    assert_eq!(decision["decision"], "deny");
    assert_eq!(decision["reasonKind"], "semantic-ast-patch-required");
    assert_eq!(decision["subject"]["toolName"], "functions.exec_command");
    assert_eq!(decision["subject"]["command"], command);
    assert_eq!(decision["subject"]["paths"], json!(["src/lib.rs"]));
    assert_eq!(decision["routes"][0]["providerId"], "rs-harness");
    assert_eq!(decision["routes"][0]["argv"][0], "asp");
    assert_eq!(decision["routes"][0]["argv"][1], "rust");
    assert_eq!(decision["routes"][0]["argv"][2], "search");
    assert_eq!(decision["routes"][0]["argv"][4], "src/lib.rs");
    let message = decision["message"].as_str().unwrap();
    assert!(message.contains("asp rust ast-patch dry-run"));
    assert!(message.contains("source apply_patch denied"));
    assert!(message.contains("provider-native"));
    assert!(!message.contains("only then retry Codex apply_patch"));
    let event = last_hook_event(&root);
    assert_eq!(event["event"], "pre-tool");
    assert_eq!(event["decision"], "deny");
    assert_eq!(event["reasonKind"], "semantic-ast-patch-required");
    std::fs::remove_dir_all(root).expect("remove temp project root")
}
