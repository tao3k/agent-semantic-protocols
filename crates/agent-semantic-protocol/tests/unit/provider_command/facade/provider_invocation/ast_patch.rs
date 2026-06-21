use std::env;
use std::process::Command;

use crate::provider_command::support::{
    asp_command, make_executable, prepend_path, provider, temp_project_root, write_activation,
    write_echo_provider,
};

#[test]
fn provider_native_ast_patch_command_is_wrapped_by_language_facade() {
    let root = temp_project_root("provider-ast-patch-facade");
    let bin_dir = root.join(".bin");
    write_echo_provider(&bin_dir, "rs-harness", "rs");
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("HOME", root.join("home"))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .env("PATH", prepend_path(&bin_dir))
        .args([
            "rust",
            "ast-patch",
            "dry-run",
            "--packet",
            "packet.json",
            ".",
        ])
        .output()
        .expect("run asp rust ast-patch");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout"),
        "rs args=[ast-patch][dry-run][--packet][packet.json]\n"
    );
    let _ = std::fs::remove_dir_all(&root);

    let root = temp_project_root("provider-ast-patch-real-apply");
    let bin_dir = root.join(".bin");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    let source_path = root.join("src/lib.rs");
    let before = "pub fn demo() -> usize {\n    1\n}\n";
    std::fs::write(&source_path, before).expect("write source");
    write_activation(&root, &[provider("rust", Vec::new())]);

    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root");
    let harness_root = workspace_root.join("languages/rust-lang-project-harness");
    let harness_manifest = harness_root.join("Cargo.toml");
    let harness_target_dir = harness_root.join("target");
    let build_output = Command::new("cargo")
        .arg("build")
        .arg("--quiet")
        .arg("--manifest-path")
        .arg(&harness_manifest)
        .arg("--features")
        .arg("cli,search")
        .arg("--bin")
        .arg("rs-harness")
        .env("CARGO_TARGET_DIR", &harness_target_dir)
        .output()
        .expect("build rs-harness");
    assert!(
        build_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&build_output.stderr)
    );
    let harness_binary = harness_target_dir
        .join("debug")
        .join(format!("rs-harness{}", std::env::consts::EXE_SUFFIX));
    assert!(harness_binary.exists(), "{}", harness_binary.display());
    std::fs::create_dir_all(&bin_dir).expect("create bin dir");
    let wrapper_path = bin_dir.join("rs-harness");
    let harness_binary_quoted = harness_binary.to_string_lossy().replace('\'', "'\\''");
    std::fs::write(
        &wrapper_path,
        format!("#!/bin/sh\nexec '{harness_binary_quoted}' \"$@\"\n"),
    )
    .expect("write rs-harness wrapper");
    make_executable(&wrapper_path);

    let packet = serde_json::json!({
        "target": {
            "ownerPath": "src/lib.rs",
            "locator": "src/lib.rs#fn:demo",
            "read": "src/lib.rs:1:3",
            "itemName": "demo",
            "itemKind": "fn"
        },
        "operation": {
            "op": "replace_item",
            "snippet": "pub fn demo() -> usize { 2 }",
            "expectedSnippet": "pub fn demo",
            "maxEdits": 1
        }
    })
    .to_string();
    let mut child = asp_command(&root)
        .env("HOME", root.join("home"))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .env("PATH", prepend_path(&bin_dir))
        .args(["rust", "ast-patch", "apply", "--packet", "-", "."])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("run real provider ast-patch apply");
    std::io::Write::write_all(child.stdin.as_mut().expect("stdin"), packet.as_bytes())
        .expect("write packet");
    let output = child.wait_with_output().expect("wait for ast-patch");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "successful provider ast-patch apply should be quiet: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let after = std::fs::read_to_string(&source_path).expect("read after");
    assert_ne!(before, after);
    assert!(after.contains("2"), "{after}");
    let _ = std::fs::remove_dir_all(root);
}
