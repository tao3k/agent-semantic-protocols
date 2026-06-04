use super::support::{
    cache_manifest_path, cache_root, provider, temp_project_root, write_activation,
    write_cache_manifest, write_marker_provider,
};
use serde_json::{Value, json};
use std::env;
use std::process::Command;

#[test]
fn cache_status_reports_missing_manifest_with_receipt() {
    let root = temp_project_root("cache-status-missing-manifest");
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .env("PRJ_HOME_CACHE", root.join(".cache"))
        .args(["cache", "status", "--receipt-json"])
        .output()
        .expect("run asp cache status");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("[asp-cache] status=disabled"), "{stdout}");
    assert!(stdout.contains("manifest=missing"), "{stdout}");
    assert!(stdout.contains("generations=0"), "{stdout}");
    assert!(
        stdout.contains(&format!("cacheRoot={}", cache_root(&root).display())),
        "{stdout}"
    );
    assert!(
        stdout.contains(&format!(
            "manifestPath={}",
            cache_manifest_path(&root).display()
        )),
        "{stdout}"
    );
    assert!(
        stdout.contains(&format!(
            "|db path={}/client.sqlite3 status=missing",
            cache_root(&root).display()
        )),
        "{stdout}"
    );

    let receipt: Value =
        serde_json::from_slice(&output.stderr).expect("stderr should be receipt JSON");
    assert_eq!(receipt["method"], "cache-status");
    assert_eq!(receipt["route"], "local-cache");
    assert_eq!(receipt["cacheStatus"], "disabled");
    assert_eq!(receipt["cacheManifestStatus"], "missing");
    assert_eq!(receipt["cacheGenerationCount"], 0);
    assert_eq!(receipt["rawSourceStored"], false);
    assert_eq!(
        receipt["cacheRoot"],
        cache_root(&root).display().to_string()
    );
    assert_eq!(
        receipt["cacheManifestPath"],
        cache_manifest_path(&root).display().to_string()
    );
    assert_eq!(
        receipt["clientDbPath"],
        cache_root(&root)
            .join("client.sqlite3")
            .display()
            .to_string()
    );
    assert_eq!(receipt["clientDbStatus"], "missing");
    assert_eq!(receipt["clientDbGenerationCount"], 0);
    assert_eq!(receipt["clientDbRawSourceStored"], false);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn cache_status_reads_manifest_without_spawning_provider() {
    let root = temp_project_root("cache-status-present-manifest");
    let bin_dir = root.join(".bin");
    let called = root.join("provider-called");
    write_marker_provider(&bin_dir, "rs-harness", &called);
    write_activation(&root, &[provider("rust", Vec::new())]);
    write_cache_manifest(&root, valid_manifest(&root));

    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .env("PATH", &bin_dir)
        .env("PRJ_HOME_CACHE", root.join(".cache"))
        .args(["cache", "status", "--receipt-json"])
        .output()
        .expect("run asp cache status");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!called.exists(), "cache status should not spawn provider");
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("manifest=present"), "{stdout}");
    assert!(stdout.contains("generations=1"), "{stdout}");
    assert!(stdout.contains("rawSourceStored=false"), "{stdout}");
    assert!(stdout.contains("status=missing"), "{stdout}");

    let receipt: Value =
        serde_json::from_slice(&output.stderr).expect("stderr should be receipt JSON");
    assert_eq!(receipt["cacheManifestStatus"], "present");
    assert_eq!(receipt["cacheGenerationCount"], 1);
    assert_eq!(receipt["rawSourceStored"], false);
    assert_eq!(receipt["clientDbStatus"], "missing");
    assert_eq!(receipt["providerCommandCount"], 0);
    assert_eq!(receipt["providerProcessesSpawned"], 0);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn cache_import_writes_manifest_generations_to_client_db_without_spawning_provider() {
    let root = temp_project_root("cache-import-present-manifest");
    let bin_dir = root.join(".bin");
    let called = root.join("provider-called");
    write_marker_provider(&bin_dir, "rs-harness", &called);
    write_activation(&root, &[provider("rust", Vec::new())]);
    write_cache_manifest(&root, valid_manifest(&root));

    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .env("PATH", &bin_dir)
        .env("PRJ_HOME_CACHE", root.join(".cache"))
        .args(["cache", "import", "--receipt-json"])
        .output()
        .expect("run asp cache import");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!called.exists(), "cache import should not spawn provider");
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("[asp-cache] status=imported"), "{stdout}");
    assert!(stdout.contains("manifest=present"), "{stdout}");
    assert!(stdout.contains("|db "), "{stdout}");
    assert!(stdout.contains("status=present"), "{stdout}");
    assert!(stdout.contains("generations=1"), "{stdout}");

    let receipt: Value =
        serde_json::from_slice(&output.stderr).expect("stderr should be receipt JSON");
    assert_eq!(receipt["method"], "cache-import");
    assert_eq!(receipt["route"], "local-cache");
    assert_eq!(receipt["cacheManifestStatus"], "present");
    assert_eq!(receipt["cacheGenerationCount"], 1);
    assert_eq!(receipt["clientDbStatus"], "present");
    assert_eq!(receipt["clientDbGenerationCount"], 1);
    assert_eq!(receipt["clientDbRawSourceStored"], false);
    assert_eq!(receipt["providerCommandCount"], 0);
    assert_eq!(receipt["providerProcessesSpawned"], 0);

    let status_output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .env("PATH", &bin_dir)
        .env("PRJ_HOME_CACHE", root.join(".cache"))
        .args(["cache", "status", "--receipt-json"])
        .output()
        .expect("run asp cache status after import");
    assert!(
        status_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&status_output.stderr)
    );
    let status_stdout = String::from_utf8(status_output.stdout).expect("status stdout");
    assert!(status_stdout.contains("status=present"), "{status_stdout}");
    assert!(status_stdout.contains("generations=1"), "{status_stdout}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn cache_status_reports_invalid_manifest_without_polluting_receipt_stream() {
    let root = temp_project_root("cache-status-invalid-manifest");
    write_activation(&root, &[provider("rust", Vec::new())]);
    let mut manifest = valid_manifest(&root);
    manifest["generations"][0]["rawSourceStored"] = json!(true);
    write_cache_manifest(&root, manifest);

    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .env("PRJ_HOME_CACHE", root.join(".cache"))
        .args(["cache", "status", "--receipt-json"])
        .output()
        .expect("run asp cache status");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("manifest=invalid"), "{stdout}");
    assert!(stdout.contains("rawSourceStored=true"), "{stdout}");
    assert!(
        stdout.contains("detail=rawSourceStored_must_be_false"),
        "{stdout}"
    );

    let stderr = String::from_utf8(output.stderr).expect("stderr");
    let receipt: Value = serde_json::from_str(&stderr).expect("stderr should be receipt JSON");
    assert_eq!(receipt["cacheManifestStatus"], "invalid");
    assert_eq!(receipt["cacheGenerationCount"], 1);
    assert_eq!(receipt["rawSourceStored"], true);
    assert!(!stderr.contains("rawSourceStored must be false"));
    let _ = std::fs::remove_dir_all(root);
}

fn valid_manifest(root: &std::path::Path) -> Value {
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
                "artifactIds": ["search/rust-main-1.json"]
            }
        ]
    })
}
