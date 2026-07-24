use crate::provider_command::support;
use serde_json::Value;
#[test]
fn client_query_invalid_cache_manifest_skips_writeback_provider_export() {
    let root = support::temp_project_root("client-query-invalid-cache-no-writeback-export");
    let bin_dir = root.join(".bin");
    let provider_args_log = root.join("provider-args.log");
    let json_export_marker = root.join("json-export-called");
    support::write_cache_source_fixture(&root);
    std::fs::create_dir_all(support::cache_root(&root)).expect("create client cache dir");
    std::fs::write(
        support::cache_root(&root).join("cache-manifest.json"),
        "{not-json",
    )
    .expect("write invalid cache manifest");
    std::fs::create_dir_all(&bin_dir).expect("create fake provider bin dir");
    let script = format!(
        r#"#!/bin/sh
printf '%s
' "$*" >> '{}'
case " $* " in
  *' --json '*|*' --json')
    printf called > '{}'
    printf 'unexpected writeback json export\n' >&2
    exit 9
    ;;
  *) printf '[query-owner] CacheReplay
|item CacheReplay read=src/lib.rs:1:1
' ;;
esac
"#,
        provider_args_log.display(),
        json_export_marker.display()
    );
    let provider_path = bin_dir.join("rs-harness");
    std::fs::write(&provider_path, script).expect("write invalid cache provider");
    support::make_executable(&provider_path);
    support::write_activation(&root, &[support::provider("rust", Vec::new())]);

    let output = support::asp_command(&root)
        .env("PATH", &bin_dir)
        .args([
            "rust",
            "query",
            "src/lib.rs",
            "--term",
            "CacheReplay",
            "--workspace",
            ".",
            "--names-only",
            "--receipt-json",
        ])
        .output()
        .expect("run query with invalid cache manifest");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("CacheReplay"), "{stdout}");
    assert!(
        !json_export_marker.exists(),
        "invalid cache must skip hidden --json writeback provider export"
    );
    let provider_args =
        std::fs::read_to_string(&provider_args_log).expect("read provider args log");
    assert!(
        provider_args.contains("query src/lib.rs --term CacheReplay --names-only"),
        "{provider_args}"
    );
    assert!(
        !provider_args.contains("--workspace"),
        "workspace must be consumed before provider dispatch: {provider_args}"
    );
    assert_eq!(provider_args.lines().count(), 1, "{provider_args}");
    let receipt: Value = serde_json::from_slice(&output.stderr).expect("receipt");
    assert_eq!(receipt["route"], "local-native");
    assert_eq!(receipt["cacheStatus"], "miss");
    assert_eq!(receipt["cacheManifestStatus"], "invalid");
    assert_eq!(receipt["providerCommandCount"], 1);
    assert_eq!(receipt["providerProcessesSpawned"], 1);
    assert_eq!(receipt["dbWriteCount"], 0);
    assert!(receipt.get("cacheWritebackProviderCommandCount").is_none());
    assert!(receipt.get("cacheWritebackProviderCommands").is_none());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn client_query_names_only_miss_uses_single_packet_first_provider_call() {
    let root = support::temp_project_root("client-query-names-only-packet-first");
    let bin_dir = root.join(".bin");
    let provider_args_log = root.join("provider-args.log");
    let non_json_marker = root.join("non-json-provider-called");
    let packet_path = root.join("query-packet.json");
    support::write_cache_source_fixture(&root);
    std::fs::write(
        &packet_path,
        r#"{"schemaId":"agent.semantic-protocols.semantic-query-packet","schemaVersion":"1","protocolId":"agent.semantic-protocols.semantic-language","protocolVersion":"1","languageId":"rust","providerId":"rs-harness","binary":"rs-harness","namespace":"agent.semantic-protocols.languages.rust.rs-harness","method":"query/owner-items","projectRoot":".","ownerPath":"src/lib.rs","outputMode":"names","query":"CacheReplay","queryTerms":["CacheReplay"],"queryCoverage":[{"value":"CacheReplay","status":"hit","match":"exact","matchCount":1,"nextAction":"select-item"}],"matchMode":"exact","truncated":false,"matches":[{"kind":"struct","name":"CacheReplay","visibility":"private","doc":false,"location":{"path":"src/lib.rs","lineRange":"1:1"},"read":"src/lib.rs:1:1","truncated":false,"fields":{"syntaxNodeType":"struct_item"}}],"patchSafety":{"level":"read-safe","reason":"compact query packet is not a mutation authority","nextAction":"query --from-hook direct-source-read"}}"#,
    )
    .expect("write names-only query packet");
    std::fs::create_dir_all(&bin_dir).expect("create fake provider bin dir");
    let script = format!(
        r#"#!/bin/sh
printf '%s
' "$*" >> '{}'
case " $* " in
  *' --json '*|*' --json') /bin/cat '{}' ;;
  *)
    printf called > '{}'
    printf 'unexpected compact provider call\n' >&2
    exit 9
    ;;
esac
"#,
        provider_args_log.display(),
        packet_path.display(),
        non_json_marker.display()
    );
    let provider_path = bin_dir.join("rs-harness");
    std::fs::write(&provider_path, script).expect("write packet-first provider");
    support::make_executable(&provider_path);
    support::write_activation(&root, &[support::provider("rust", Vec::new())]);

    let first = support::asp_command(&root)
        .env("PATH", &bin_dir)
        .args([
            "rust",
            "query",
            "src/lib.rs",
            "--term",
            "CacheReplay",
            "--names-only",
            ".",
            "--receipt-json",
        ])
        .output()
        .expect("run names-only query packet-first miss");
    assert!(
        first.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(
        !non_json_marker.exists(),
        "packet-first query must not call provider a second time for compact stdout"
    );
    let first_stdout = String::from_utf8(first.stdout).expect("first stdout");
    assert!(
        first_stdout.contains("reason=cache-query-packet output=names"),
        "{first_stdout}"
    );
    assert!(
        first_stdout.contains("|item CacheReplay kind=struct"),
        "{first_stdout}"
    );
    let first_receipt: Value = serde_json::from_slice(&first.stderr).expect("first receipt");
    assert_eq!(first_receipt["route"], "local-native");
    assert_eq!(first_receipt["cacheStatus"], "miss");
    assert_eq!(first_receipt["providerCommandCount"], 1);
    assert_eq!(first_receipt["providerProcessesSpawned"], 1);
    assert_eq!(first_receipt["dbReadCount"], 2);
    assert_eq!(first_receipt["dbWriteCount"], 2);
    assert!(first_receipt["packetBytes"].as_u64().unwrap_or_default() > 0);
    assert!(
        first_receipt
            .get("cacheWritebackProviderCommandCount")
            .is_none()
    );
    assert!(
        first_receipt
            .get("cacheWritebackProviderCommands")
            .is_none()
    );
    let first_argv = first_receipt["providerCommands"][0]["argv"]
        .as_array()
        .expect("provider argv");
    assert!(first_argv.iter().any(|arg| arg.as_str() == Some("--json")));
    let provider_args =
        std::fs::read_to_string(&provider_args_log).expect("read provider args log");
    assert_eq!(provider_args.lines().count(), 1, "{provider_args}");
    assert!(
        provider_args.contains("query src/lib.rs --term CacheReplay"),
        "{provider_args}"
    );
    assert!(provider_args.contains("--json"), "{provider_args}");

    let second = support::asp_command(&root)
        .env("PATH", &bin_dir)
        .args([
            "rust",
            "query",
            "src/lib.rs",
            "--term",
            "CacheReplay",
            "--names-only",
            ".",
            "--receipt-json",
        ])
        .output()
        .expect("run names-only query packet cache hit");
    assert!(
        second.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&second.stderr)
    );
    let second_receipt: Value = serde_json::from_slice(&second.stderr).expect("second receipt");
    assert_eq!(second_receipt["route"], "local-cache");
    assert_eq!(second_receipt["cacheStatus"], "hit");
    assert_eq!(second_receipt["providerCommandCount"], 0);
    assert_eq!(second_receipt["providerProcessesSpawned"], 0);
    assert_eq!(second_receipt["dbReadCount"], 2);
    assert_eq!(second_receipt["dbWriteCount"], 0);
    let provider_args_after_hit =
        std::fs::read_to_string(&provider_args_log).expect("read provider args after hit");
    assert_eq!(provider_args_after_hit.lines().count(), 1);

    let _ = std::fs::remove_dir_all(root);
}
