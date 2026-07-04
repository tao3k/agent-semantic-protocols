use serde_json::Value;

use super::fixtures::{
    sample_query_packet, sample_search_packet, valid_manifest_with_artifact,
    valid_query_manifest_with_artifact, valid_search_manifest_with_artifact,
};
use crate::provider_command::support::{
    artifacts_root, asp_command, provider, temp_project_root, write_activation,
    write_cache_manifest, write_marker_provider,
};

fn assert_client_db_runtime_profile(receipt: &Value) {
    assert_eq!(receipt["clientDbJournalMode"], "wal");
    assert_eq!(receipt["clientDbSynchronous"], 1);
    assert_eq!(receipt["clientDbBusyTimeoutMs"], 5000);
    assert_eq!(receipt["clientDbForeignKeys"], true);
}

#[test]
fn client_search_receipt_reports_cache_hit_when_prompt_output_artifact_exists() {
    let root = temp_project_root("client-search-receipt-cache-hit");
    let bin_dir = root.join(".bin");
    let called = root.join("provider-called");
    let cached_stdout = "cached prompt artifact\n";
    let artifact_id = "prompt-output/rust-prime.txt";
    let artifact_path = artifacts_root(&root).join(artifact_id);

    std::fs::create_dir_all(artifact_path.parent().expect("artifact parent"))
        .expect("create artifact dir");
    std::fs::write(&artifact_path, cached_stdout).expect("write prompt artifact");
    write_marker_provider(&bin_dir, "rs-harness", &called);
    write_activation(&root, &[provider("rust", Vec::new())]);
    write_cache_manifest(&root, valid_manifest_with_artifact(&root, artifact_id));

    let import_output = asp_command(&root)
        .env("PATH", &bin_dir)
        .args(["cache", "import"])
        .output()
        .expect("run cache import");
    assert!(
        import_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&import_output.stderr)
    );

    let output = asp_command(&root)
        .env("PATH", &bin_dir)
        .args([
            "rust",
            "search",
            "prime",
            "--workspace",
            ".",
            "--view",
            "seeds",
            "--receipt-json",
        ])
        .output()
        .expect("run search");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        called.exists(),
        "prompt-output artifacts are not replayed without a request fingerprint"
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert_ne!(stdout, cached_stdout);

    let receipt: Value = serde_json::from_slice(&output.stderr).expect("receipt json on stderr");
    assert_eq!(receipt["route"], "local-native");
    assert_eq!(receipt["cacheStatus"], "miss");
    assert_eq!(receipt["clientDbStatus"], "present");
    assert_client_db_runtime_profile(&receipt);
    assert_eq!(receipt["providerCommandCount"], 1);
    assert_eq!(receipt["providerProcessesSpawned"], 1);
    assert_eq!(receipt["stdoutBytes"], stdout.len());

    std::fs::remove_dir_all(root).expect("remove temp root");
}

#[test]
fn client_search_receipt_reports_cache_hit_when_search_packet_artifact_exists() {
    let root = temp_project_root("client-search-receipt-cache-hit-packet");
    let bin_dir = root.join(".bin");
    let called = root.join("provider-called");
    let artifact_id = "search/rust-main-1.json";
    let artifact_path = artifacts_root(&root).join(artifact_id);
    std::fs::create_dir_all(artifact_path.parent().expect("artifact parent"))
        .expect("create artifact dir");
    std::fs::write(
        &artifact_path,
        serde_json::to_string_pretty(&sample_search_packet()).expect("packet json"),
    )
    .expect("write search artifact");
    write_marker_provider(&bin_dir, "rs-harness", &called);
    write_activation(&root, &[provider("rust", Vec::new())]);
    write_cache_manifest(
        &root,
        valid_search_manifest_with_artifact(&root, artifact_id),
    );

    let import_output = asp_command(&root)
        .env("PATH", &bin_dir)
        .args(["cache", "import"])
        .output()
        .expect("run cache import");
    assert!(
        import_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&import_output.stderr)
    );

    let output = asp_command(&root)
        .env("PATH", &bin_dir)
        .env_remove("SEMANTIC_AGENT_PROTOCOL_BIN")
        .args([
            "rust",
            "search",
            "prime",
            "--workspace",
            ".",
            "--view",
            "seeds",
            "--receipt-json",
        ])
        .output()
        .expect("run search");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!called.exists(), "provider should not run on cache hit");
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("CacheReplay"), "{stdout}");
    let receipt: Value = serde_json::from_slice(&output.stderr).expect("receipt json on stderr");
    assert_eq!(receipt["route"], "local-cache");
    assert_eq!(receipt["cacheStatus"], "hit");
    assert_eq!(receipt["clientDbStatus"], "present");
    assert_client_db_runtime_profile(&receipt);
    assert_eq!(receipt["providerCommandCount"], 0);
    assert_eq!(receipt["providerProcessesSpawned"], 0);
    assert_eq!(receipt["dbReadCount"], 2);
    assert_eq!(receipt["dbWriteCount"], 0);
    assert!(receipt["elapsedMs"].as_u64().is_some());
    assert_eq!(
        receipt["providerCommands"]
            .as_array()
            .expect("providerCommands")
            .len(),
        0
    );
    assert_eq!(receipt["stdoutBytes"], stdout.len());

    let root_arg = root.display().to_string();
    let external_output = asp_command(&root)
        .current_dir(root.parent().expect("root parent"))
        .env("PATH", &bin_dir)
        .args([
            "rust",
            "search",
            "prime",
            "--view",
            "seeds",
            root_arg.as_str(),
            "--receipt-json",
        ])
        .output()
        .expect("run external search");
    assert!(
        external_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&external_output.stderr)
    );
    assert!(
        !called.exists(),
        "provider should not run for external root hit"
    );
    let external_stdout = String::from_utf8(external_output.stdout).expect("external stdout");
    assert!(external_stdout.contains("CacheReplay"), "{external_stdout}");
    let external_receipt: Value =
        serde_json::from_slice(&external_output.stderr).expect("external receipt json");
    assert_eq!(external_receipt["method"], "search");
    assert_eq!(external_receipt["route"], "local-cache");
    assert_eq!(external_receipt["cacheStatus"], "hit");
    assert_eq!(external_receipt["clientDbStatus"], "present");
    assert_client_db_runtime_profile(&external_receipt);
    assert_eq!(external_receipt["providerCommandCount"], 0);
    assert_eq!(external_receipt["providerProcessesSpawned"], 0);
    assert_eq!(external_receipt["dbReadCount"], 2);
    assert_eq!(external_receipt["dbWriteCount"], 0);
    assert!(external_receipt["elapsedMs"].as_u64().is_some());
    assert_eq!(
        external_receipt["providerCommands"]
            .as_array()
            .expect("providerCommands")
            .len(),
        0
    );
    assert_eq!(external_receipt["stdoutBytes"], external_stdout.len());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn client_query_receipt_reports_cache_hit_when_query_packet_artifact_exists() {
    let root = temp_project_root("client-query-receipt-cache-hit-packet");
    let bin_dir = root.join(".bin");
    let called = root.join("provider-called");
    let artifact_id = "query/rust-owner-items-1.json";
    let artifact_path = artifacts_root(&root).join(artifact_id);
    std::fs::create_dir_all(artifact_path.parent().expect("artifact parent"))
        .expect("create artifact dir");
    std::fs::write(
        &artifact_path,
        serde_json::to_string_pretty(&sample_query_packet()).expect("packet json"),
    )
    .expect("write query artifact");
    write_marker_provider(&bin_dir, "rs-harness", &called);
    write_activation(&root, &[provider("rust", Vec::new())]);
    write_cache_manifest(
        &root,
        valid_query_manifest_with_artifact(&root, artifact_id),
    );

    let import_output = asp_command(&root)
        .env("PATH", &bin_dir)
        .args(["cache", "import"])
        .output()
        .expect("run cache import");
    assert!(
        import_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&import_output.stderr)
    );

    let output = asp_command(&root)
        .env("PATH", &bin_dir)
        .args([
            "rust",
            "query",
            "src/lib.rs",
            "--term",
            "CacheReplay",
            ".",
            "--receipt-json",
        ])
        .output()
        .expect("run query");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!called.exists(), "provider should not run on cache hit");
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("CacheReplay"), "{stdout}");
    let receipt: Value = serde_json::from_slice(&output.stderr).expect("receipt json on stderr");
    assert_eq!(receipt["method"], "query");
    assert_eq!(receipt["route"], "local-cache");
    assert_eq!(receipt["cacheStatus"], "hit");
    assert_eq!(receipt["clientDbStatus"], "present");
    assert_client_db_runtime_profile(&receipt);
    assert_eq!(receipt["providerCommandCount"], 0);
    assert_eq!(receipt["providerProcessesSpawned"], 0);
    assert_eq!(receipt["dbReadCount"], 2);
    assert_eq!(receipt["dbWriteCount"], 0);
    assert!(receipt["elapsedMs"].as_u64().is_some());
    assert_eq!(
        receipt["providerCommands"]
            .as_array()
            .expect("providerCommands")
            .len(),
        0
    );
    assert_eq!(receipt["stdoutBytes"], stdout.len());

    let root_arg = root.display().to_string();
    let external_output = asp_command(&root)
        .current_dir(root.parent().expect("root parent"))
        .env("PATH", &bin_dir)
        .args([
            "rust",
            "query",
            "src/lib.rs",
            "--term",
            "CacheReplay",
            root_arg.as_str(),
            "--receipt-json",
        ])
        .output()
        .expect("run external query");
    assert!(
        external_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&external_output.stderr)
    );
    assert!(
        !called.exists(),
        "provider should not run for external root hit"
    );
    let external_stdout = String::from_utf8(external_output.stdout).expect("external stdout");
    assert!(external_stdout.contains("CacheReplay"), "{external_stdout}");
    let external_receipt: Value =
        serde_json::from_slice(&external_output.stderr).expect("external receipt json");
    assert_eq!(external_receipt["method"], "query");
    assert_eq!(external_receipt["route"], "local-cache");
    assert_eq!(external_receipt["cacheStatus"], "hit");
    assert_eq!(external_receipt["clientDbStatus"], "present");
    assert_client_db_runtime_profile(&external_receipt);
    assert_eq!(external_receipt["providerCommandCount"], 0);
    assert_eq!(external_receipt["providerProcessesSpawned"], 0);
    assert_eq!(external_receipt["dbReadCount"], 2);
    assert_eq!(external_receipt["dbWriteCount"], 0);
    assert!(external_receipt["elapsedMs"].as_u64().is_some());
    assert_eq!(
        external_receipt["providerCommands"]
            .as_array()
            .expect("providerCommands")
            .len(),
        0
    );
    assert_eq!(external_receipt["stdoutBytes"], external_stdout.len());
    let _ = std::fs::remove_dir_all(root);
}
