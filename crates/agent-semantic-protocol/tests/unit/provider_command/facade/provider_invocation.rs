use std::env;
use std::process::Command;

use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation, write_echo_provider,
    write_runtime_profiles,
};

#[test]
fn provider_command_prefix_is_used_as_full_invocation_prefix() {
    let root = temp_project_root("provider-prefix-facade");
    let bin_dir = root.join(".bin");
    std::fs::create_dir_all(&bin_dir).expect("create bin dir");
    let wrapper_path = bin_dir.join("provider-wrapper");
    std::fs::write(
        &wrapper_path,
        r#"#!/bin/sh
printf 'wrapper args='
for arg in "$@"; do printf '[%s]' "$arg"; done
printf '
'
printf 'cache=%s
' "$PRJ_HOME_CACHE"
printf 'runtime=%s
' "$ASP_RUNTIME_BIN_DIR"
printf 'path0=%s
' "${PATH%%:*}"
printf 'renderer=%s
' "$SEMANTIC_AGENT_PROTOCOL_BIN"
"#,
    )
    .expect("write provider wrapper");
    let mut permissions = std::fs::metadata(&wrapper_path)
        .expect("wrapper metadata")
        .permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
    std::fs::set_permissions(&wrapper_path, permissions).expect("set wrapper permissions");
    write_activation(
        &root,
        &[provider(
            "rust",
            vec!["provider-wrapper".to_string(), "rs-harness".to_string()],
        )],
    );

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env_remove("SEMANTIC_AGENT_PROTOCOL_BIN")
        .args(["rust", "query", "src/lib.rs", "."])
        .output()
        .expect("run asp rust query");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let canonical_root = std::fs::canonicalize(&root).unwrap_or_else(|_| root.clone());
    let cache_home = canonical_root.join(".cache");
    let runtime_bin = cache_home.join("agent-semantic-protocol/runtime/bin");
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout"),
        format!(
            "wrapper args=[rs-harness][query][src/lib.rs][.]\ncache={}\nruntime={}\npath0={}\nrenderer={}\n",
            cache_home.display(),
            runtime_bin.display(),
            runtime_bin.display(),
            env!("CARGO_BIN_EXE_asp")
        )
    );

    let nested_root = root.join("languages/rust-lang-project-harness");
    write_activation(
        &nested_root,
        &[provider(
            "rust",
            vec!["provider-wrapper".to_string(), "rs-harness".to_string()],
        )],
    );
    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .args([
            "rust",
            "check",
            "--changed",
            "languages/rust-lang-project-harness",
        ])
        .output()
        .expect("run asp rust check nested root");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout"),
        format!(
            "wrapper args=[rs-harness][check][--changed][.]\ncache={}\nruntime={}\npath0={}\nrenderer={}\n",
            cache_home.display(),
            runtime_bin.display(),
            runtime_bin.display(),
            env!("CARGO_BIN_EXE_asp")
        )
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn language_facade_query_uses_runtime_profile_before_path_lookup() {
    let root = temp_project_root("provider-runtime-profile-facade");
    let profile_bin_dir = root.join(".profile-bin");
    let path_bin_dir = root.join(".path-bin");
    write_echo_provider(&profile_bin_dir, "rs-harness", "profile");
    std::fs::create_dir_all(&path_bin_dir).expect("create path bin dir");
    let path_provider = path_bin_dir.join("rs-harness");
    std::fs::write(
        &path_provider,
        "#!/bin/sh\nprintf 'path provider should not run\\n' >&2\nexit 42\n",
    )
    .expect("write path provider");
    let mut permissions = std::fs::metadata(&path_provider)
        .expect("path provider metadata")
        .permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
    std::fs::set_permissions(&path_provider, permissions).expect("set path provider permissions");

    write_activation(&root, &[provider("rust", Vec::new())]);
    write_runtime_profiles(
        &root,
        "rust",
        vec![profile_bin_dir.join("rs-harness").display().to_string()],
    );

    let output = asp_command(&root)
        .env("PATH", prepend_path(&path_bin_dir))
        .args(["rust", "query", "src/lib.rs", "."])
        .output()
        .expect("run asp rust query");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout"),
        "profile args=[query][src/lib.rs][.]\n"
    );
    let _ = std::fs::remove_dir_all(root);
}
#[test]
fn provider_native_ast_patch_command_is_wrapped_by_language_facade() {
    let root = temp_project_root("provider-ast-patch-facade");
    let bin_dir = root.join(".bin");
    write_echo_provider(&bin_dir, "rs-harness", "rs");
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
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
        "rs args=[ast-patch][dry-run][--packet][packet.json][.]\n"
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
    let mut wrapper_permissions = std::fs::metadata(&wrapper_path)
        .expect("wrapper metadata")
        .permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut wrapper_permissions, 0o755);
    std::fs::set_permissions(&wrapper_path, wrapper_permissions).expect("chmod wrapper");

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
