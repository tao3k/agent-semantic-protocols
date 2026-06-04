#[path = "../../src/command/apply_patch.rs"]
mod apply_patch_impl;

use apply_patch_impl::apply_patch_source_paths;

#[test]
fn extracts_unique_paths_from_apply_patch_hunks() {
    let command = r#"apply_patch <<'PATCH'
*** Begin Patch
*** Update File: src/lib.rs
@@
-old
+new
*** Move to: src/main.rs
*** Update File: src/lib.rs
*** End Patch
PATCH"#;

    assert_eq!(
        apply_patch_source_paths("functions.exec_command", command),
        vec!["src/lib.rs".to_string(), "src/main.rs".to_string()]
    );
}

#[test]
fn ignores_patch_text_without_apply_patch_invocation() {
    let command = r#"printf '%s\n' '*** Begin Patch
*** Update File: src/lib.rs
*** End Patch'"#;

    assert!(apply_patch_source_paths("functions.exec_command", command).is_empty());
}

#[test]
fn accepts_direct_apply_patch_tool_payload() {
    let patch = r#"*** Begin Patch
*** Delete File: "src/generated.ts"
*** End Patch"#;

    assert_eq!(
        apply_patch_source_paths("functions.apply_patch", patch),
        vec!["src/generated.ts".to_string()]
    );
}
