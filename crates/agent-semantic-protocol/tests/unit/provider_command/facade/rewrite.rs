use std::env;
use std::process::Command;

use crate::provider_command::support::{
    provider, temp_project_root, write_activation, write_command_hint_provider,
    write_guide_provider,
};

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
        .expect("run asp guide");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");

    assert!(stdout.contains("provider=rs-harness"));
    assert!(
        stdout.contains("|cmd prime=asp rust search prime ."),
        "{stdout}"
    );
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
        stdout.contains("|cmd agent-doctor=asp rust agent doctor --json ."),
        "{stdout}"
    );
    assert!(
        stdout.contains("|rule hook install/runtime is owned by agent-semantic-hook"),
        "{stdout}"
    );
    assert!(!stdout.contains("rs-harness search"));
    assert!(!stdout.contains("rs-harness ast-patch"));
    std::fs::remove_dir_all(root).expect("remove temp root");
}
