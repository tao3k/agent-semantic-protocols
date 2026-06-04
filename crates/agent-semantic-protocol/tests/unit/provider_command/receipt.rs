use super::support::{
    cache_root, provider, temp_project_root, write_activation, write_cache_manifest,
    write_echo_provider, write_marker_provider, write_stdout_stderr_exit_provider,
    write_stdout_stderr_provider,
};
use serde_json::{Value, json};
use std::env;
use std::process::Command;

#[test]
fn client_search_receipt_records_local_native_provider_command() {
    let root = temp_project_root("client-search-receipt");
    let bin_dir = root.join(".bin");
    write_echo_provider(&bin_dir, "rs-harness", "rs");
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .env("PATH", &bin_dir)
        .env("PRJ_HOME_CACHE", root.join(".cache"))
        .args([
            "search",
            "--language",
            "rust",
            "prime",
            "--view",
            "seeds",
            ".",
            "--receipt-json",
        ])
        .output()
        .expect("run asp search");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout_len = output.stdout.len() as u64;
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout"),
        "rs args=[search][prime][--view][seeds][.]\n"
    );
    let receipt: Value =
        serde_json::from_slice(&output.stderr).expect("stderr should be receipt JSON");
    assert_eq!(receipt["method"], "search");
    assert_eq!(receipt["route"], "local-native");
    assert_eq!(receipt["cacheStatus"], "miss");
    assert_eq!(receipt["providerCommandCount"], 1);
    assert_eq!(receipt["providerProcessesSpawned"], 1);
    assert_eq!(receipt["nativeProvenance"][0]["providerId"], "rs-harness");
    assert_eq!(
        receipt["providerCommands"][0]["argv"],
        serde_json::json!(["rs-harness", "search", "prime", "--view", "seeds", "."])
    );
    assert_eq!(receipt["providerCommands"][0]["exitCode"], 0);
    assert_eq!(receipt["providerCommands"][0]["stdoutBytes"], stdout_len);
    assert_eq!(receipt["stdoutBytes"], stdout_len);
    assert_eq!(receipt["stderrBytes"], 0);
    assert!(receipt["elapsedMs"].as_u64().is_some());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn client_search_receipt_reports_warm_provider_when_matching_generation_exists() {
    let root = temp_project_root("client-search-receipt-warm-provider");
    let bin_dir = root.join(".bin");
    write_echo_provider(&bin_dir, "rs-harness", "rs");
    write_activation(&root, &[provider("rust", Vec::new())]);
    write_cache_manifest(&root, valid_manifest(&root));

    let import_output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .env("PATH", &bin_dir)
        .env("PRJ_HOME_CACHE", root.join(".cache"))
        .args(["cache", "import"])
        .output()
        .expect("run asp cache import");
    assert!(
        import_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&import_output.stderr)
    );

    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .env("PATH", &bin_dir)
        .env("PRJ_HOME_CACHE", root.join(".cache"))
        .args([
            "search",
            "--language",
            "rust",
            "prime",
            "--view",
            "seeds",
            ".",
            "--receipt-json",
        ])
        .output()
        .expect("run asp search");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout"),
        "rs args=[search][prime][--view][seeds][.]\n"
    );
    let receipt: Value =
        serde_json::from_slice(&output.stderr).expect("stderr should be receipt JSON");
    assert_eq!(receipt["cacheStatus"], "warm-provider");
    assert_eq!(receipt["clientDbStatus"], "present");
    assert_eq!(receipt["clientDbGenerationCount"], 1);
    assert_eq!(receipt["providerCommandCount"], 1);
    assert_eq!(receipt["providerProcessesSpawned"], 1);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn client_search_receipt_reports_cache_hit_when_prompt_output_artifact_exists() {
    let root = temp_project_root("client-search-receipt-cache-hit");
    let bin_dir = root.join(".bin");
    let called = root.join("provider-called");
    let artifact_id = "prompt-output/rust-prime.txt";
    let artifact_path = cache_root(&root)
        .parent()
        .expect("protocol cache root")
        .join("artifacts")
        .join(artifact_id);
    std::fs::create_dir_all(artifact_path.parent().expect("artifact parent"))
        .expect("create artifact dir");
    std::fs::write(&artifact_path, "[search-prime] cached\n").expect("write artifact");
    write_marker_provider(&bin_dir, "rs-harness", &called);
    write_activation(&root, &[provider("rust", Vec::new())]);
    write_cache_manifest(&root, valid_manifest_with_artifact(&root, artifact_id));

    let import_output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .env("PATH", &bin_dir)
        .env("PRJ_HOME_CACHE", root.join(".cache"))
        .args(["cache", "import"])
        .output()
        .expect("run asp cache import");
    assert!(
        import_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&import_output.stderr)
    );

    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .env("PATH", &bin_dir)
        .env("PRJ_HOME_CACHE", root.join(".cache"))
        .args([
            "search",
            "--language",
            "rust",
            "prime",
            "--view",
            "seeds",
            ".",
            "--receipt-json",
        ])
        .output()
        .expect("run asp search");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!called.exists(), "cache hit should not spawn provider");
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout"),
        "[search-prime] cached\n"
    );
    let receipt: Value =
        serde_json::from_slice(&output.stderr).expect("stderr should be receipt JSON");
    assert_eq!(receipt["route"], "local-cache");
    assert_eq!(receipt["cacheStatus"], "hit");
    assert_eq!(receipt["clientDbStatus"], "present");
    assert_eq!(receipt["providerCommandCount"], 0);
    assert_eq!(receipt["providerProcessesSpawned"], 0);
    assert_eq!(
        receipt["stdoutBytes"],
        "[search-prime] cached\n".len() as u64
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn receipt_json_suppresses_provider_stderr_from_receipt_stream() {
    let root = temp_project_root("client-search-receipt-stderr");
    let bin_dir = root.join(".bin");
    let stdout_text = "[search-prime] ok\n";
    let stderr_text = "[provider-warning] stderr detail\n";
    write_stdout_stderr_provider(&bin_dir, "rs-harness", stdout_text, stderr_text);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .env("PATH", &bin_dir)
        .env("PRJ_HOME_CACHE", root.join(".cache"))
        .args([
            "search",
            "--language",
            "rust",
            "prime",
            "--view",
            "seeds",
            ".",
            "--receipt-json",
        ])
        .output()
        .expect("run asp search");

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
        "provider stderr must not be mixed into receipt JSON"
    );
    let receipt: Value = serde_json::from_str(&stderr).expect("stderr should be receipt JSON");
    assert_eq!(receipt["method"], "search");
    assert_eq!(
        receipt["providerCommands"][0]["stderrBytes"],
        stderr_text.len() as u64
    );
    assert_eq!(receipt["stderrBytes"], stderr_text.len() as u64);
    let _ = std::fs::remove_dir_all(root);
}

fn valid_manifest(root: &std::path::Path) -> Value {
    valid_manifest_with_artifact(root, "search/rust-main-1.json")
}

fn valid_manifest_with_artifact(root: &std::path::Path, artifact_id: &str) -> Value {
    json!({
        "schemaId": "agent.semantic-protocols.client-cache-manifest",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.client",
        "protocolVersion": "1",
        "cacheRoot": cache_root(root).display().to_string(),
        "generations": [
            {
                "generationId": "rust-main-1",
                "languageId": "rust",
                "providerId": "rs-harness",
                "providerVersion": "0.1.0",
                "exportMethod": "search/prime",
                "projectRoot": root.display().to_string(),
                "packageRoot": ".",
                "schemaIds": ["agent.semantic-protocols.semantic-search-packet"],
                "cacheStatus": "miss",
                "rawSourceStored": false,
                "fileHashes": [],
                "artifactIds": [artifact_id]
            }
        ]
    })
}

#[test]
fn receipt_json_is_emitted_for_nonzero_provider_exit() {
    let root = temp_project_root("client-search-receipt-nonzero");
    let bin_dir = root.join(".bin");
    let stdout_text = "";
    let stderr_text = "[provider-error] failed\n";
    write_stdout_stderr_exit_provider(&bin_dir, "rs-harness", stdout_text, stderr_text, 7);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .env("PATH", &bin_dir)
        .env("PRJ_HOME_CACHE", root.join(".cache"))
        .args([
            "search",
            "--language",
            "rust",
            "prime",
            "--view",
            "seeds",
            ".",
            "--receipt-json",
        ])
        .output()
        .expect("run asp search");

    assert_eq!(output.status.code(), Some(7));
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout"),
        stdout_text
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(
        !stderr.contains(stderr_text),
        "provider stderr must not be mixed into receipt JSON"
    );
    let receipt: Value = serde_json::from_str(&stderr).expect("stderr should be receipt JSON");
    assert_eq!(receipt["providerCommands"][0]["exitCode"], 7);
    assert_eq!(
        receipt["providerCommands"][0]["stderrBytes"],
        stderr_text.len() as u64
    );
    assert_eq!(receipt["stderrBytes"], stderr_text.len() as u64);
    let _ = std::fs::remove_dir_all(root);
}
