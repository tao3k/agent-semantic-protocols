use serde_json::Value;
use std::env;
use std::process::Command;

use crate::provider_command::support::{
    provider, temp_project_root, write_activation, write_echo_provider,
};

#[test]
fn client_search_receipt_records_local_native_provider_command() {
    let root = temp_project_root("client-search-receipt");
    let bin_dir = root.join(".bin");
    write_echo_provider(&bin_dir, "rs-harness", "rs");
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .env("PATH", &bin_dir)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
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
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("rs"), "{stdout}");
    let stdout_len = stdout.len();
    let receipt: Value = serde_json::from_slice(&output.stderr).expect("receipt json on stderr");
    assert_eq!(receipt["method"], "search");
    assert_eq!(receipt["route"], "local-native");
    assert_eq!(receipt["cacheStatus"], "miss");
    assert_eq!(receipt["providerCommandCount"], 1);
    assert_eq!(receipt["providerProcessesSpawned"], 1);
    assert_eq!(receipt["nativeProvenance"][0]["providerId"], "rs-harness");
    let resolved_provider = std::fs::canonicalize(bin_dir.join("rs-harness"))
        .expect("canonical provider binary")
        .display()
        .to_string();
    assert_eq!(
        receipt["providerCommands"][0]["argv"],
        serde_json::json!([resolved_provider, "search", "prime", "--view", "seeds"])
    );
    assert_eq!(receipt["providerCommands"][0]["exitCode"], 0);
    assert_eq!(receipt["providerCommands"][0]["stdoutBytes"], stdout_len);
    assert_eq!(receipt["stdoutBytes"], stdout_len);
    assert_eq!(receipt["stderrBytes"], 0);
    assert!(receipt["elapsedMs"].as_u64().is_some());
    let _ = std::fs::remove_dir_all(root);
}
