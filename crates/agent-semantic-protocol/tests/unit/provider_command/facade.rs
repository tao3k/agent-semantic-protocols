use super::support::{
    provider, temp_project_root, write_activation, write_command_hint_provider,
    write_echo_provider, write_guide_provider, write_pwd_provider, write_stdin_provider,
};
use std::env;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[test]
fn rust_search_facade_execs_activated_provider() {
    let root = temp_project_root("rust-search-facade");
    let bin_dir = root.join(".bin");
    write_echo_provider(&bin_dir, "rs-harness", "rs");
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .env("PATH", &bin_dir)
        .args(["rust", "search", "prime", "--view", "seeds", "."])
        .output()
        .expect("run asp rust search");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout"),
        "rs args=[search][prime][--view][seeds][.]\n"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn language_facade_discovers_activation_from_child_directory() {
    let root = temp_project_root("child-search-facade");
    let bin_dir = root.join(".bin");
    let child_dir = root.join("nested").join("workspace");
    std::fs::create_dir_all(&child_dir).expect("create child directory");
    write_pwd_provider(&bin_dir, "rs-harness");
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&child_dir)
        .env("PATH", &bin_dir)
        .args(["rust", "search", "prime", "."])
        .output()
        .expect("run asp rust search");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let actual_root = PathBuf::from(String::from_utf8(output.stdout).expect("stdout").trim());
    assert_eq!(
        std::fs::canonicalize(actual_root).expect("canonical actual root"),
        std::fs::canonicalize(&root).expect("canonical expected root")
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn language_facade_selects_matching_provider_from_activation() {
    let root = temp_project_root("typescript-search-facade");
    let bin_dir = root.join(".bin");
    write_echo_provider(&bin_dir, "rs-harness", "rs");
    write_echo_provider(&bin_dir, "ts-harness", "ts");
    write_activation(
        &root,
        &[
            provider("rust", Vec::new()),
            provider("typescript", Vec::new()),
        ],
    );

    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .env("PATH", &bin_dir)
        .args(["typescript", "search", "fzf", "parseSearchArgs", "."])
        .output()
        .expect("run asp typescript search");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout"),
        "ts args=[search][fzf][parseSearchArgs][.]\n"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn provider_command_prefix_is_used_as_full_invocation_prefix() {
    let root = temp_project_root("provider-prefix-facade");
    let bin_dir = root.join(".bin");
    write_echo_provider(&bin_dir, "provider-wrapper", "wrapper");
    write_activation(
        &root,
        &[provider(
            "rust",
            vec!["provider-wrapper".to_string(), "rs-harness".to_string()],
        )],
    );

    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .env("PATH", &bin_dir)
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
        "wrapper args=[rs-harness][query][src/lib.rs][.]\n"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn provider_native_ast_patch_command_is_wrapped_by_language_facade() {
    let root = temp_project_root("provider-ast-patch-facade");
    let bin_dir = root.join(".bin");
    write_echo_provider(&bin_dir, "rs-harness", "rs");
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .env("PATH", &bin_dir)
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
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn provider_stdin_is_preserved_for_pipe_commands() {
    let root = temp_project_root("provider-stdin-facade");
    let bin_dir = root.join(".bin");
    write_stdin_provider(&bin_dir, "rs-harness");
    write_activation(&root, &[provider("rust", Vec::new())]);

    let mut child = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .env("PATH", &bin_dir)
        .args(["rust", "search", "ingest", "."])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("run asp rust search ingest");
    child
        .stdin
        .as_mut()
        .expect("facade stdin")
        .write_all(b"src/lib.rs:10:HookDecision\n")
        .expect("write stdin");

    let output = child.wait_with_output().expect("wait for facade");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout"),
        "stdin=src/lib.rs:10:HookDecision\n"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn provider_output_command_hints_are_rewritten_without_changing_identity() {
    let root = temp_project_root("provider-output-rewrite");
    let bin_dir = root.join(".bin");
    write_command_hint_provider(&bin_dir, "rs-harness");
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .env("PATH", &bin_dir)
        .args(["rust", "ast-patch", "dry-run", "--packet", "-", "."])
        .output()
        .expect("run asp rust ast-patch");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("\"provider\":\"rs-harness\""), "{stdout}");
    assert!(
        stdout.contains("\"next\":\"asp rust query src/lib.rs .\""),
        "{stdout}"
    );
    assert!(!stdout.contains("\"next\":\"rs-harness query"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn agent_guide_rewrites_command_lines_to_language_facade() {
    let root = temp_project_root("agent-guide-facade");
    let bin_dir = root.join(".bin");
    write_guide_provider(&bin_dir, "rs-harness");
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .env("PATH", &bin_dir)
        .args(["rust", "agent", "guide", "."])
        .output()
        .expect("run asp rust agent guide");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("provider=rs-harness"));
    assert!(stdout.contains("|cmd prime=asp rust search prime ."));
    assert!(
        stdout.contains("|cmd ingest=rg -n '<query>' src tests | asp rust search ingest ."),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "|cmd ast-patch=asp rust ast-patch dry-run --packet <semantic-ast-patch.json|-> ."
        ),
        "{stdout}"
    );
    assert!(
        stdout
            .contains("|cmd evidence=asp rust evidence graph --review-packet-json <path> --json ."),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "|rule hook install/runtime uses asp hook; agent-semantic-hook owns classification runtime"
        ),
        "{stdout}"
    );
    assert!(!stdout.contains("hook install/runtime is owned by agent-semantic-hook"));
    let _ = std::fs::remove_dir_all(root);
}
