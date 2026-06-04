use serde_json::Value;
use std::env;
use std::process::Command;

use crate::provider_command::support::{
    provider, temp_project_root, write_activation, write_stdout_stderr_exit_provider,
    write_stdout_stderr_provider,
};

#[test]
fn receipt_json_suppresses_provider_stderr_from_receipt_stream() {
    let root = temp_project_root("client-search-receipt-stderr");
    let bin_dir = root.join(".bin");
    let stdout_text = "provider stdout line\n";
    let stderr_text = "provider stderr should be measured\n";
    write_stdout_stderr_provider(&bin_dir, "rs-harness", stdout_text, stderr_text);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .env("PATH", &bin_dir)
        .env("PRJ_HOME_CACHE", root.join(".cache"))
        .args([
            "rust",
            "search",
            "prime",
            "--view",
            "seeds",
            ".",
            "--receipt-json",
        ])
        .output()
        .expect("run search");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout"),
        stdout_text
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(
        !stderr.contains(stderr_text),
        "provider stderr should stay out of receipt stream"
    );
    let receipt: Value = serde_json::from_str(&stderr).expect("receipt json on stderr");
    assert_eq!(receipt["method"], "search");
    assert_eq!(
        receipt["providerCommands"][0]["stderrBytes"],
        stderr_text.len()
    );
    assert_eq!(receipt["stderrBytes"], stderr_text.len());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn receipt_json_is_emitted_for_nonzero_provider_exit() {
    let root = temp_project_root("client-search-receipt-nonzero");
    let bin_dir = root.join(".bin");
    let stdout_text = "";
    let stderr_text = "provider failed intentionally\n";
    write_stdout_stderr_exit_provider(&bin_dir, "rs-harness", stdout_text, stderr_text, 7);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .env("PATH", &bin_dir)
        .env("PRJ_HOME_CACHE", root.join(".cache"))
        .args([
            "rust",
            "search",
            "prime",
            "--view",
            "seeds",
            ".",
            "--receipt-json",
        ])
        .output()
        .expect("run search");

    assert_eq!(output.status.code(), Some(7));
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout"),
        stdout_text
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(
        !stderr.contains(stderr_text),
        "provider stderr should stay out of receipt stream"
    );
    let receipt: Value = serde_json::from_str(&stderr).expect("receipt json on stderr");
    assert_eq!(receipt["providerCommands"][0]["exitCode"], 7);
    assert_eq!(
        receipt["providerCommands"][0]["stderrBytes"],
        stderr_text.len()
    );
    assert_eq!(receipt["stderrBytes"], stderr_text.len());
    let _ = std::fs::remove_dir_all(root);
}
