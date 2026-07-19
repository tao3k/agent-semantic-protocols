use crate::provider_command::receipt::writeback::support::PRIME_DECISION_LINE;
use crate::provider_command::support;
use serde_json::Value;
#[test]
fn client_search_miss_writes_prompt_output_cache_for_next_hit() {
    let root = support::temp_project_root("client-search-writeback");
    let bin_dir = root.join(".bin");
    let called = root.join("provider-called-after-writeback");
    let called_after_invalidate = root.join("provider-called-after-invalidate");
    let different_args_called = root.join("provider-called-for-different-args");
    let stdout_text = format!("[search-prime] cached\n{PRIME_DECISION_LINE}\n|owner src/lib.rs\n");
    let stdout_after_invalidate =
        format!("[search-prime] after invalidate\n{PRIME_DECISION_LINE}\n|owner src/lib.rs\n");
    support::write_cache_source_fixture(&root);
    support::write_stdout_stderr_provider(&bin_dir, "rs-harness", &stdout_text, "");
    support::write_activation(&root, &[support::provider("rust", Vec::new())]);

    let first_output = support::asp_command(&root)
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
        .expect("run first search");
    assert!(
        first_output.status.success(),
        "{}",
        String::from_utf8_lossy(&first_output.stderr)
    );
    assert_eq!(
        String::from_utf8(first_output.stdout).expect("first stdout"),
        stdout_text
    );
    let first_receipt: Value = serde_json::from_slice(&first_output.stderr).expect("first receipt");
    assert_eq!(first_receipt["route"], "local-native");
    assert_eq!(first_receipt["cacheStatus"], "miss");
    assert_eq!(first_receipt["clientDbStatus"], "present");
    assert_eq!(first_receipt["clientDbGenerationCount"], 1);
    assert_eq!(first_receipt["providerCommandCount"], 1);
    assert_eq!(first_receipt["dbReadCount"], 2);
    assert_eq!(first_receipt["dbWriteCount"], 2);
    assert_eq!(
        first_receipt["providerCommands"][0]["stdoutBytes"].as_u64(),
        Some(stdout_text.len() as u64)
    );
    assert!(
        first_receipt["providerCommands"][0]["stdoutSha256"]
            .as_str()
            .is_some()
    );
    assert!(
        first_receipt["providerCommands"][0]["stderrSha256"]
            .as_str()
            .is_some()
    );

    let manifest_text =
        std::fs::read_to_string(support::cache_root(&root).join("cache-manifest.json"))
            .expect("read manifest");
    assert!(manifest_text.contains("prompt-output/"), "{manifest_text}");
    assert!(
        manifest_text.contains("client-prompt-output"),
        "{manifest_text}"
    );
    assert!(
        manifest_text.contains("requestFingerprint"),
        "{manifest_text}"
    );
    assert!(manifest_text.contains(".command.json"), "{manifest_text}");

    let manifest: Value = serde_json::from_str(&manifest_text).expect("manifest JSON");
    let artifact_ids = manifest["generations"][0]["artifactIds"]
        .as_array()
        .expect("artifact ids");
    let prompt_artifact_id = artifact_ids
        .iter()
        .filter_map(Value::as_str)
        .find(|artifact_id| {
            artifact_id.starts_with("prompt-output/") && artifact_id.ends_with(".txt")
        })
        .expect("prompt-output artifact id");
    let command_artifact_id = artifact_ids
        .iter()
        .filter_map(Value::as_str)
        .find(|artifact_id| {
            artifact_id.starts_with("prompt-output/") && artifact_id.ends_with(".command.json")
        })
        .expect("prompt-output command artifact id");
    let command_artifact_path = support::artifacts_root(&root).join(command_artifact_id);
    let command_artifact: Value = serde_json::from_slice(
        &std::fs::read(command_artifact_path).expect("read command artifact"),
    )
    .expect("command artifact JSON");
    assert_eq!(
        command_artifact["schemaId"],
        "agent.semantic-protocols.client-prompt-output-command"
    );
    assert_eq!(
        command_artifact["promptOutputArtifactId"],
        prompt_artifact_id
    );
    assert_eq!(
        command_artifact["providerCommands"][0]["providerId"],
        "rs-harness"
    );
    let argv = command_artifact["providerCommands"][0]["argv"]
        .as_array()
        .expect("provider argv");
    assert!(argv.iter().any(|arg| arg.as_str() == Some("search")));
    assert!(argv.iter().any(|arg| arg.as_str() == Some("prime")));

    support::write_marker_provider(&bin_dir, "rs-harness", &called);
    let second_output = support::asp_command(&root)
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
        .expect("run second search");
    assert!(
        second_output.status.success(),
        "{}",
        String::from_utf8_lossy(&second_output.stderr)
    );
    assert!(
        !called.exists(),
        "provider should not be called on cache hit"
    );
    let second_stdout = String::from_utf8(second_output.stdout).expect("second stdout");
    assert_eq!(second_stdout, stdout_text);
    let second_receipt: Value =
        serde_json::from_slice(&second_output.stderr).expect("second receipt");
    assert_eq!(second_receipt["route"], "local-cache");
    assert_eq!(second_receipt["cacheStatus"], "hit");
    assert_eq!(second_receipt["providerCommandCount"], 0);
    assert_eq!(second_receipt["providerProcessesSpawned"], 0);
    assert_eq!(second_receipt["dbReadCount"], 2);
    assert_eq!(second_receipt["dbWriteCount"], 0);

    let invalidate_output = support::asp_command(&root)
        .env("PATH", &bin_dir)
        .args(["cache", "invalidate", "--receipt-json"])
        .output()
        .expect("run cache invalidate");
    assert!(
        invalidate_output.status.success(),
        "{}",
        String::from_utf8_lossy(&invalidate_output.stderr)
    );
    let invalidate_receipt: Value =
        serde_json::from_slice(&invalidate_output.stderr).expect("invalidate receipt");
    assert_eq!(invalidate_receipt["method"], "cache-invalidate");
    assert_eq!(invalidate_receipt["route"], "local-cache");
    assert_eq!(invalidate_receipt["cacheStatus"], "invalidated");
    assert_eq!(invalidate_receipt["clientDbStatus"], "present");
    assert_eq!(invalidate_receipt["clientDbGenerationCount"], 0);
    assert_eq!(invalidate_receipt["providerCommandCount"], 0);
    assert_eq!(invalidate_receipt["providerProcessesSpawned"], 0);

    support::write_stdout_stderr_provider(&bin_dir, "rs-harness", &stdout_after_invalidate, "");
    let third_output = support::asp_command(&root)
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
        .expect("run search after invalidate");
    assert!(
        third_output.status.success(),
        "{}",
        String::from_utf8_lossy(&third_output.stderr)
    );
    assert_eq!(
        String::from_utf8(third_output.stdout).expect("third stdout"),
        stdout_after_invalidate
    );
    let third_receipt: Value = serde_json::from_slice(&third_output.stderr).expect("third receipt");
    assert_eq!(third_receipt["route"], "local-native");
    assert_eq!(third_receipt["cacheStatus"], "miss");
    assert_eq!(third_receipt["providerCommandCount"], 1);
    assert_eq!(third_receipt["providerProcessesSpawned"], 1);

    support::write_marker_provider(&bin_dir, "rs-harness", &called_after_invalidate);
    let fourth_output = support::asp_command(&root)
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
        .expect("run search after rehydrated cache");
    assert!(
        fourth_output.status.success(),
        "{}",
        String::from_utf8_lossy(&fourth_output.stderr)
    );
    assert!(
        !called_after_invalidate.exists(),
        "provider should not be called after rehydrated cache hit"
    );
    assert_eq!(
        String::from_utf8(fourth_output.stdout).expect("fourth stdout"),
        stdout_after_invalidate
    );
    let fourth_receipt: Value =
        serde_json::from_slice(&fourth_output.stderr).expect("fourth receipt");
    assert_eq!(fourth_receipt["route"], "local-cache");
    assert_eq!(fourth_receipt["cacheStatus"], "hit");
    assert_eq!(fourth_receipt["providerCommandCount"], 0);
    assert_eq!(fourth_receipt["providerProcessesSpawned"], 0);
    assert_eq!(fourth_receipt["dbReadCount"], 2);
    assert_eq!(fourth_receipt["dbWriteCount"], 0);

    support::write_marker_provider(&bin_dir, "rs-harness", &different_args_called);
    let fifth_output = support::asp_command(&root)
        .env("PATH", &bin_dir)
        .args([
            "rust",
            "search",
            "prime",
            "--view",
            "seeds",
            "--focus",
            "tests",
            ".",
            "--receipt-json",
        ])
        .output()
        .expect("run search with different forwarded args");
    assert!(
        fifth_output.status.success(),
        "{}",
        String::from_utf8_lossy(&fifth_output.stderr)
    );
    assert!(
        different_args_called.exists(),
        "different forwarded args must not replay the previous prompt-output artifact"
    );
    let fifth_receipt: Value = serde_json::from_slice(&fifth_output.stderr).expect("fifth receipt");
    assert_eq!(fifth_receipt["route"], "local-native");
    assert_eq!(fifth_receipt["cacheStatus"], "miss");
    assert_eq!(fifth_receipt["providerCommandCount"], 1);
    assert_eq!(fifth_receipt["providerProcessesSpawned"], 1);

    std::fs::remove_dir_all(root).expect("remove temp root");
}
