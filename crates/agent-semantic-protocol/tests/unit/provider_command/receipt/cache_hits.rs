use super::fixtures::{
    sample_search_packet, valid_manifest_with_artifact, valid_search_manifest_with_artifact,
};
use crate::provider_command::support::{
    artifacts_root, asp_command, provider, temp_project_root, write_activation,
    write_cache_manifest, write_echo_provider, write_marker_provider,
};
use serde_json::Value;

#[test]
fn client_search_receipt_does_not_replay_prompt_output_without_request_fingerprint() {
    let root = temp_project_root("client-search-receipt-prompt-without-fingerprint");
    let bin_dir = root.join(".bin");
    let artifact_id = "prompt-output/rust-prime.txt";
    let artifact_path = artifacts_root(&root).join(artifact_id);
    std::fs::create_dir_all(artifact_path.parent().expect("artifact parent"))
        .expect("create artifact dir");
    std::fs::write(&artifact_path, "cached prompt artifact\n").expect("write prompt artifact");
    write_echo_provider(&bin_dir, "rs-harness", "live-provider");
    write_activation(&root, &[provider("rust", Vec::new())]);
    write_cache_manifest(&root, valid_manifest_with_artifact(&root, artifact_id));

    let imported = asp_command(&root)
        .args(["cache", "import"])
        .output()
        .expect("import cache manifest");
    assert!(
        imported.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&imported.stderr)
    );

    let output = asp_command(&root)
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
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(!stdout.contains("cached prompt artifact"), "{stdout}");
    let receipt: Value = serde_json::from_slice(&output.stderr).expect("receipt");
    assert_eq!(receipt["route"], "local-native");
    assert_eq!(receipt["cacheStatus"], "miss");
    assert_eq!(receipt["clientDbStatus"], "present");
    assert_eq!(receipt["providerCommandCount"], 1);
    assert_eq!(receipt["providerProcessesSpawned"], 1);

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn client_search_receipt_reports_turso_hit_for_search_packet_artifact() {
    let root = temp_project_root("client-search-receipt-search-packet-hit");
    let bin_dir = root.join(".bin");
    let called = root.join("provider-called");
    let artifact_id = "search/rust-main-1.json";
    let artifact_path = artifacts_root(&root).join(artifact_id);
    std::fs::create_dir_all(artifact_path.parent().expect("artifact parent"))
        .expect("create artifact dir");
    std::fs::write(
        &artifact_path,
        serde_json::to_string_pretty(&sample_search_packet()).expect("packet JSON"),
    )
    .expect("write search artifact");
    write_marker_provider(&bin_dir, "rs-harness", &called);
    write_activation(&root, &[provider("rust", Vec::new())]);
    write_cache_manifest(
        &root,
        valid_search_manifest_with_artifact(&root, artifact_id),
    );

    let imported = asp_command(&root)
        .args(["cache", "import"])
        .output()
        .expect("import cache manifest");
    assert!(
        imported.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&imported.stderr)
    );

    let output = asp_command(&root)
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
        .expect("run cached search");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!called.exists(), "provider should not run on Turso hit");
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("CacheReplay"), "{stdout}");
    let receipt: Value = serde_json::from_slice(&output.stderr).expect("receipt");
    assert_eq!(receipt["route"], "local-cache");
    assert_eq!(receipt["cacheStatus"], "hit");
    assert_eq!(receipt["clientDbStatus"], "present");
    assert_eq!(receipt["providerCommandCount"], 0);
    assert_eq!(receipt["providerProcessesSpawned"], 0);
    assert_eq!(receipt["dbReadCount"], 3);
    assert_eq!(receipt["dbWriteCount"], 0);

    let _ = std::fs::remove_dir_all(root);
}
