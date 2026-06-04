use agent_semantic_hook::codex_hook_block;

#[test]
fn codex_hook_matcher_includes_apply_patch_surfaces() {
    let block = codex_hook_block();

    assert!(block.contains("apply_patch|applypatch"));
    assert!(block.contains("functions\\\\.apply_patch"));
    assert!(block.contains("functions.exec_command"));
    assert!(!block.contains("matcher = \".*\""));
}
