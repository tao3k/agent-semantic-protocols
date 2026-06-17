use serde_json::Value;
use std::env;
use std::process::Command;

use super::fixtures::valid_manifest;
use crate::provider_command::support::{
    CACHE_SOURCE_PATH, CACHE_SOURCE_SHA256, cache_root, make_executable, provider,
    temp_project_root, write_activation, write_cache_manifest, write_cache_source_fixture,
    write_echo_provider, write_marker_provider, write_stdout_stderr_provider,
};

const PRIME_DECISION_LINE: &str = "|decision purpose=decision-primer answer=false code=false capabilities=pipe,fzf,fd-query,rg-query,owner-items,selector-code,treesitter-query ladder=pipe>fzf>fd-query|rg-query>owner-items>selector-code history=asp-artifacts:directReadRisk,repeatedPrime,repeatedPipe,bestPath risk=broad-direct-read,manual-window-scan,repeat-prime next=\"asp rust search pipe '<question-or-feature-term>' --workspace . --view seeds\"";

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[test]
fn client_search_miss_writes_prompt_output_cache_for_next_hit() {
    let root = temp_project_root("client-search-writeback");
    let bin_dir = root.join(".bin");
    let called = root.join("provider-called-after-writeback");
    let called_after_invalidate = root.join("provider-called-after-invalidate");
    let different_args_called = root.join("provider-called-for-different-args");
    let stdout_text = format!("[search-prime] cached\n{PRIME_DECISION_LINE}\n|owner src/lib.rs\n");
    let stdout_after_invalidate =
        format!("[search-prime] after invalidate\n{PRIME_DECISION_LINE}\n|owner src/lib.rs\n");
    write_cache_source_fixture(&root);
    write_stdout_stderr_provider(&bin_dir, "rs-harness", &stdout_text, "");
    write_activation(&root, &[provider("rust", Vec::new())]);

    let first_output = Command::new(env!("CARGO_BIN_EXE_asp"))
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
    assert_eq!(first_receipt["sqliteReadCount"], 2);
    assert_eq!(first_receipt["sqliteWriteCount"], 2);
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

    let manifest_text = std::fs::read_to_string(cache_root(&root).join("cache-manifest.json"))
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
    let command_artifact_path = cache_root(&root)
        .parent()
        .expect("cache parent")
        .join("artifacts")
        .join(command_artifact_id);
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

    write_marker_provider(&bin_dir, "rs-harness", &called);
    let second_output = Command::new(env!("CARGO_BIN_EXE_asp"))
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
    assert_eq!(second_receipt["sqliteReadCount"], 2);
    assert_eq!(second_receipt["sqliteWriteCount"], 0);

    let invalidate_output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .env("PATH", &bin_dir)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
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

    write_stdout_stderr_provider(&bin_dir, "rs-harness", &stdout_after_invalidate, "");
    let third_output = Command::new(env!("CARGO_BIN_EXE_asp"))
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

    write_marker_provider(&bin_dir, "rs-harness", &called_after_invalidate);
    let fourth_output = Command::new(env!("CARGO_BIN_EXE_asp"))
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
    assert_eq!(fourth_receipt["sqliteReadCount"], 2);
    assert_eq!(fourth_receipt["sqliteWriteCount"], 0);

    write_marker_provider(&bin_dir, "rs-harness", &different_args_called);
    let fifth_output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .env("PATH", &bin_dir)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
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
        .env("PRJ_CACHE_HOME", root.join(".cache"))
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
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout"),
        "rs args=[search][prime][--view][seeds]\n"
    );
    let receipt: Value =
        serde_json::from_slice(&output.stderr).expect("stderr should be receipt JSON");
    assert_eq!(receipt["cacheStatus"], "warm-provider");
    assert_eq!(receipt["clientDbStatus"], "present");
    assert_eq!(receipt["clientDbGenerationCount"], 2);
    assert_eq!(receipt["providerCommandCount"], 1);
    assert_eq!(receipt["providerProcessesSpawned"], 1);
    let _ = std::fs::remove_dir_all(root);

    let search_root = temp_project_root("client-search-writeback-packet");
    let search_bin_dir = search_root.join(".bin");
    let search_provider_args_log = search_root.join("provider-args.log");
    let search_packet_path = search_root.join("search-packet.json");
    write_cache_source_fixture(&search_root);
    std::fs::write(
        &search_packet_path,
        attach_cache_file_hashes(
            r#"{"schemaId":"agent.semantic-protocols.semantic-search-packet","schemaVersion":"1","protocolId":"agent.semantic-protocols.semantic-language","protocolVersion":"1","languageId":"rust","providerId":"rs-harness","view":"prime","query":"CacheReplay","querySet":["CacheReplay"],"searchSynthesis":{"algorithm":"cache-packet-writeback","seeds":[{"kind":"owner","target":"src/lib.rs","targetRole":"path"},{"kind":"symbol","target":"CacheReplay","targetRole":"symbol","read":"src/lib.rs:1:5"},{"kind":"tests","target":"tests/cache_replay.rs","targetRole":"path"}]}}"#,
        ),
    )
    .expect("write search packet");
    std::fs::create_dir_all(&search_bin_dir).expect("create fake search provider bin dir");
    let search_script = format!(
        r#"#!/bin/sh
printf '%s
' "$*" >> '{}'
case " $* " in
  *' --json '*|*' --json') /bin/cat '{}' ;;
        *) printf '%s
' '[search-prime] CacheReplay' {} '|seed owner:src/lib.rs' '|seed symbol:CacheReplay' ;;
esac
"#,
        search_provider_args_log.display(),
        search_packet_path.display(),
        shell_single_quote(PRIME_DECISION_LINE)
    );
    let search_provider_path = search_bin_dir.join("rs-harness");
    std::fs::write(&search_provider_path, search_script).expect("write search packet provider");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&search_provider_path)
            .expect("search provider metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&search_provider_path, permissions)
            .expect("chmod search provider");
    }
    write_activation(&search_root, &[provider("rust", Vec::new())]);

    let first_search = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&search_root)
        .env("PATH", &search_bin_dir)
        .env("PRJ_CACHE_HOME", search_root.join(".cache"))
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
        .expect("run search packet writeback miss");
    assert!(
        first_search.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&first_search.stderr)
    );
    let first_search_stdout = String::from_utf8(first_search.stdout).expect("first search stdout");
    assert!(
        first_search_stdout.contains("CacheReplay"),
        "{first_search_stdout}"
    );
    let first_search_receipt: Value =
        serde_json::from_slice(&first_search.stderr).expect("first search receipt");
    assert_eq!(first_search_receipt["route"], "local-native");
    assert_eq!(first_search_receipt["cacheStatus"], "miss");
    assert_eq!(first_search_receipt["clientDbStatus"], "present");
    assert_eq!(first_search_receipt["clientDbGenerationCount"], 1);
    assert_eq!(first_search_receipt["providerCommandCount"], 1);
    assert_eq!(first_search_receipt["providerProcessesSpawned"], 1);
    assert_eq!(first_search_receipt["sqliteReadCount"], 2);
    assert_eq!(first_search_receipt["sqliteWriteCount"], 2);
    let search_provider_args =
        std::fs::read_to_string(&search_provider_args_log).expect("read search provider args");
    assert!(
        search_provider_args.contains("search prime --view seeds"),
        "{search_provider_args}"
    );
    assert!(
        search_provider_args.contains("search prime --view seeds --json"),
        "{search_provider_args}"
    );
    let search_provider_arg_count = search_provider_args.lines().count();
    assert_eq!(search_provider_arg_count, 2);
    let search_manifest_text =
        std::fs::read_to_string(cache_root(&search_root).join("cache-manifest.json"))
            .expect("read search writeback manifest");
    assert!(
        search_manifest_text.contains("search/prime"),
        "{search_manifest_text}"
    );
    assert!(
        search_manifest_text.contains("prompt-output/"),
        "{search_manifest_text}"
    );
    let second_search = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&search_root)
        .env("PATH", &search_bin_dir)
        .env("PRJ_CACHE_HOME", search_root.join(".cache"))
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
        .expect("run search packet writeback hit");
    assert!(
        second_search.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&second_search.stderr)
    );
    let second_search_stdout =
        String::from_utf8(second_search.stdout).expect("second search stdout");
    assert!(
        second_search_stdout.contains("CacheReplay"),
        "{second_search_stdout}"
    );
    let second_search_receipt: Value =
        serde_json::from_slice(&second_search.stderr).expect("second search receipt");
    assert_eq!(second_search_receipt["route"], "local-cache");
    assert_eq!(second_search_receipt["cacheStatus"], "hit");
    assert_eq!(second_search_receipt["providerCommandCount"], 0);
    assert_eq!(second_search_receipt["providerProcessesSpawned"], 0);
    assert_eq!(second_search_receipt["sqliteReadCount"], 2);
    assert_eq!(second_search_receipt["sqliteWriteCount"], 0);
    let search_provider_args_after_hit = std::fs::read_to_string(&search_provider_args_log)
        .expect("read search provider args after hit");
    assert_eq!(
        search_provider_args_after_hit.lines().count(),
        search_provider_arg_count
    );
    let _ = std::fs::remove_dir_all(search_root);

    let query_root = temp_project_root("client-query-writeback-packet");
    let query_bin_dir = query_root.join(".bin");
    let provider_args_log = query_root.join("provider-args.log");
    let packet_path = query_root.join("query-packet.json");
    write_cache_source_fixture(&query_root);
    std::fs::write(
        &packet_path,
        r#"{"schemaId":"agent.semantic-protocols.semantic-query-packet","schemaVersion":"1","protocolId":"agent.semantic-protocols.semantic-language","protocolVersion":"1","languageId":"rust","providerId":"rs-harness","binary":"rs-harness","namespace":"agent.semantic-protocols.languages.rust.rs-harness","method":"query/owner-items","projectRoot":".","ownerPath":"src/lib.rs","outputMode":"outline","query":"CacheReplay","queryTerms":["CacheReplay"],"queryCoverage":[{"value":"CacheReplay","status":"hit","match":"exact","matchCount":1,"nextAction":"select-item"}],"matchMode":"exact","truncated":false,"matches":[{"kind":"struct","name":"CacheReplay","visibility":"private","doc":false,"location":{"path":"src/lib.rs","lineRange":"1:3"},"read":"src/lib.rs:1:3","code":"struct CacheReplay\nfield stdout: Vec<u8>","truncated":false,"projection":{"mode":"compact","syntax":"semantic-outline","sourceAuthority":"native-parser","sourceFingerprint":"src/lib.rs:1:3:44","exactRead":"src/lib.rs:1:3","losslessStructure":true,"nodes":[{"id":"query-cache-node","kind":"struct","role":"declaration","label":"struct CacheReplay","depth":0,"nativeId":"rust:struct:CacheReplay","read":"src/lib.rs:1:3","structuralFingerprint":"struct:declaration:CacheReplay"}],"renderedNodeIds":["query-cache-node"]}}],"patchSafety":{"safeForPatch":true,"reasons":[]}}"#,
    )
    .expect("write query packet");
    std::fs::create_dir_all(&query_bin_dir).expect("create fake provider bin dir");
    let script = format!(
        r#"#!/bin/sh
printf '%s
' "$*" >> '{}'
case " $* " in
  *' --json '*|*' --json') /bin/cat '{}' ;;
  *) printf '[query-owner] CacheReplay
|item CacheReplay read=src/lib.rs:1:1
' ;;
esac
"#,
        provider_args_log.display(),
        packet_path.display()
    );
    let provider_path = query_bin_dir.join("rs-harness");
    std::fs::write(&provider_path, script).expect("write query packet provider");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&provider_path)
            .expect("query provider metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&provider_path, permissions).expect("chmod query provider");
    }
    write_activation(&query_root, &[provider("rust", Vec::new())]);

    let first_query = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&query_root)
        .env("PATH", &query_bin_dir)
        .env("PRJ_CACHE_HOME", query_root.join(".cache"))
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
        .expect("run query writeback miss");
    assert!(
        first_query.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&first_query.stderr)
    );
    let first_stdout = String::from_utf8(first_query.stdout).expect("first query stdout");
    assert!(first_stdout.contains("CacheReplay"), "{first_stdout}");
    let first_receipt: Value =
        serde_json::from_slice(&first_query.stderr).expect("first query receipt");
    assert_eq!(first_receipt["route"], "local-native");
    assert_eq!(first_receipt["cacheStatus"], "miss");
    assert_eq!(first_receipt["clientDbStatus"], "present");
    assert_eq!(first_receipt["clientDbGenerationCount"], 1);
    assert_eq!(first_receipt["providerCommandCount"], 1);
    assert_eq!(first_receipt["providerProcessesSpawned"], 1);
    assert_eq!(first_receipt["cacheWritebackProviderCommandCount"], 1);
    assert_eq!(first_receipt["cacheWritebackProviderProcessesSpawned"], 1);
    assert!(
        first_receipt["cacheWritebackProviderElapsedMs"]
            .as_u64()
            .is_some()
    );
    let writeback_argv = first_receipt["cacheWritebackProviderCommands"][0]["argv"]
        .as_array()
        .expect("writeback provider argv");
    assert!(
        writeback_argv
            .iter()
            .any(|arg| arg.as_str() == Some("query"))
    );
    assert!(
        writeback_argv
            .iter()
            .any(|arg| arg.as_str() == Some("--json"))
    );
    assert_eq!(first_receipt["sqliteReadCount"], 2);
    assert_eq!(first_receipt["sqliteWriteCount"], 2);
    let provider_args =
        std::fs::read_to_string(&provider_args_log).expect("read provider args log");
    assert!(
        provider_args.contains("query src/lib.rs --term CacheReplay"),
        "{provider_args}"
    );
    assert!(
        provider_args.contains("query src/lib.rs --term CacheReplay --json"),
        "{provider_args}"
    );
    let provider_arg_count = provider_args.lines().count();
    assert_eq!(provider_arg_count, 2);
    let manifest_text =
        std::fs::read_to_string(cache_root(&query_root).join("cache-manifest.json"))
            .expect("read query writeback manifest");
    assert!(
        manifest_text.contains("query/owner-items"),
        "{manifest_text}"
    );
    assert!(manifest_text.contains("query/"), "{manifest_text}");
    assert!(manifest_text.contains(CACHE_SOURCE_PATH), "{manifest_text}");
    assert!(
        manifest_text.contains(CACHE_SOURCE_SHA256),
        "{manifest_text}"
    );

    let second_query = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&query_root)
        .env("PATH", &query_bin_dir)
        .env("PRJ_CACHE_HOME", query_root.join(".cache"))
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
        .expect("run query writeback hit");
    assert!(
        second_query.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&second_query.stderr)
    );
    let second_stdout = String::from_utf8(second_query.stdout).expect("second query stdout");
    assert!(second_stdout.contains("CacheReplay"), "{second_stdout}");
    let second_receipt: Value =
        serde_json::from_slice(&second_query.stderr).expect("second query receipt");
    assert_eq!(second_receipt["route"], "local-cache");
    assert_eq!(second_receipt["cacheStatus"], "hit");
    assert_eq!(second_receipt["providerCommandCount"], 0);
    assert_eq!(second_receipt["providerProcessesSpawned"], 0);
    assert_eq!(second_receipt["sqliteReadCount"], 2);
    assert_eq!(second_receipt["sqliteWriteCount"], 0);
    let provider_args_after_hit =
        std::fs::read_to_string(&provider_args_log).expect("read provider args after hit");
    assert_eq!(provider_args_after_hit.lines().count(), provider_arg_count);

    let _ = std::fs::remove_dir_all(query_root);
}

#[test]
fn client_query_invalid_cache_manifest_skips_writeback_provider_export() {
    let root = temp_project_root("client-query-invalid-cache-no-writeback-export");
    let bin_dir = root.join(".bin");
    let provider_args_log = root.join("provider-args.log");
    let json_export_marker = root.join("json-export-called");
    write_cache_source_fixture(&root);
    std::fs::create_dir_all(cache_root(&root)).expect("create client cache dir");
    std::fs::write(cache_root(&root).join("cache-manifest.json"), "{not-json")
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
    make_executable(&provider_path);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .env("PATH", &bin_dir)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
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
    assert_eq!(receipt["sqliteWriteCount"], 0);
    assert!(receipt.get("cacheWritebackProviderCommandCount").is_none());
    assert!(receipt.get("cacheWritebackProviderCommands").is_none());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn client_query_names_only_miss_uses_single_packet_first_provider_call() {
    let root = temp_project_root("client-query-names-only-packet-first");
    let bin_dir = root.join(".bin");
    let provider_args_log = root.join("provider-args.log");
    let non_json_marker = root.join("non-json-provider-called");
    let packet_path = root.join("query-packet.json");
    write_cache_source_fixture(&root);
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
    make_executable(&provider_path);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let first = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .env("PATH", &bin_dir)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
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
    assert_eq!(first_receipt["sqliteReadCount"], 2);
    assert_eq!(first_receipt["sqliteWriteCount"], 2);
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

    let second = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .env("PATH", &bin_dir)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
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
    assert_eq!(second_receipt["sqliteReadCount"], 2);
    assert_eq!(second_receipt["sqliteWriteCount"], 0);
    let provider_args_after_hit =
        std::fs::read_to_string(&provider_args_log).expect("read provider args after hit");
    assert_eq!(provider_args_after_hit.lines().count(), 1);

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn client_syntax_query_writeback_hashes_locator_paths_and_replays_rows_without_artifact() {
    let root = temp_project_root("client-syntax-query-writeback-rows");
    let bin_dir = root.join(".bin");
    let provider_args_log = root.join("provider-args.log");
    let packet_path = root.join("syntax-query-packet.json");
    write_cache_source_fixture(&root);
    let packet = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-tree-sitter-query",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "languageId": "rust",
        "providerId": "rs-harness",
        "method": "query",
        "grammarId": "tree-sitter-rust",
        "grammarProfileVersion": "2026-06-04.v1",
        "query": {
            "input": "(function_item name: (identifier) @function.name)",
            "inputForm": "s-expression",
            "dialect": "tree-sitter-query",
            "compiledSource": "(function_item name: (identifier) @function.name)",
            "fields": {
                "selector": "src/lib.rs:1:3",
                "codeOutput": false,
                "captures": ["function.name"]
            }
        },
        "matches": [
            {
                "id": "m1",
                "range": {"path": "src/lib.rs", "lineRange": "1:3"},
                "captures": [
                    {
                        "id": "c1",
                        "name": "function.name",
                        "nodeType": "identifier",
                        "range": {"path": "src/lib.rs", "lineRange": "1:1"},
                        "fields": {"symbol": "parse_query"}
                    }
                ]
            }
        ],
        "truncated": false,
        "cache": {
            "artifactKind": "semantic-tree-sitter-query",
            "rawSourceStored": false
        }
    });
    std::fs::write(
        &packet_path,
        serde_json::to_string(&packet).expect("serialize syntax packet"),
    )
    .expect("write syntax packet");
    std::fs::create_dir_all(&bin_dir).expect("create fake provider bin dir");
    let script = format!(
        r#"#!/bin/sh
printf '%s
' "$*" >> '{}'
case " $* " in
  *' --json '*|*' --json') /bin/cat '{}' ;;
  *) printf '[query-treesitter] fake
|syntax-capture function.name read=src/lib.rs:1
' ;;
esac
"#,
        provider_args_log.display(),
        packet_path.display()
    );
    let provider_path = bin_dir.join("rs-harness");
    std::fs::write(&provider_path, script).expect("write syntax provider");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&provider_path)
            .expect("syntax provider metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&provider_path, permissions).expect("chmod syntax provider");
    }
    write_activation(&root, &[provider("rust", Vec::new())]);

    let query = "(function_item name: (identifier) @function.name)";
    let first = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .env("PATH", &bin_dir)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "query",
            "--treesitter-query",
            query,
            "--selector",
            "src/lib.rs:1:3",
            ".",
            "--receipt-json",
        ])
        .output()
        .expect("run syntax query writeback miss");
    assert!(
        first.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    let first_receipt: Value = serde_json::from_slice(&first.stderr).expect("first receipt");
    assert_eq!(first_receipt["route"], "local-native");
    assert_eq!(first_receipt["cacheStatus"], "miss");
    assert_eq!(first_receipt["providerCommandCount"], 1);
    assert_eq!(first_receipt["providerProcessesSpawned"], 1);
    assert_eq!(first_receipt["sqliteReadCount"], 2);
    assert_eq!(first_receipt["sqliteWriteCount"], 3);
    assert_eq!(first_receipt["clientDbSyntaxRowGenerationCount"], 1);
    assert_eq!(first_receipt["clientDbSyntaxRowMatchCount"], 1);
    assert_eq!(first_receipt["clientDbSyntaxRowCaptureCount"], 1);
    let manifest_text = std::fs::read_to_string(cache_root(&root).join("cache-manifest.json"))
        .expect("read syntax writeback manifest");
    assert!(manifest_text.contains(CACHE_SOURCE_PATH), "{manifest_text}");
    assert!(
        manifest_text.contains(CACHE_SOURCE_SHA256),
        "{manifest_text}"
    );
    let provider_args =
        std::fs::read_to_string(&provider_args_log).expect("read syntax provider args");
    assert!(
        provider_args.contains("query --treesitter-query"),
        "{provider_args}"
    );
    assert!(provider_args.contains(" --json"), "{provider_args}");
    let provider_arg_count = provider_args.lines().count();
    assert_eq!(provider_arg_count, 2);

    let second = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .env("PATH", &bin_dir)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "query",
            "--treesitter-query",
            query,
            "--selector",
            "src/lib.rs:1:3",
            ".",
            "--receipt-json",
        ])
        .output()
        .expect("run syntax query artifact hit");
    assert!(
        second.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&second.stderr)
    );
    let second_stdout = String::from_utf8(second.stdout).expect("second stdout");
    assert!(second_stdout.contains("parse_query"), "{second_stdout}");
    let second_receipt: Value = serde_json::from_slice(&second.stderr).expect("second receipt");
    assert_eq!(second_receipt["route"], "local-cache");
    assert_eq!(second_receipt["cacheStatus"], "hit");
    assert_eq!(second_receipt["providerCommandCount"], 0);
    assert_eq!(second_receipt["providerProcessesSpawned"], 0);
    assert_eq!(second_receipt["sqliteReadCount"], 2);
    assert_eq!(second_receipt["sqliteWriteCount"], 0);
    assert_eq!(second_receipt["clientDbSyntaxRowGenerationCount"], 1);
    assert_eq!(second_receipt["clientDbSyntaxRowMatchCount"], 1);
    assert_eq!(second_receipt["clientDbSyntaxRowCaptureCount"], 1);
    assert!(
        second_receipt["syntaxArtifactId"]
            .as_str()
            .is_some_and(|id| id.starts_with("semantic-tree-sitter-query/"))
    );
    assert!(second_receipt["packetBytes"].as_u64().unwrap_or_default() > 0);

    let syntax_artifact_dir = cache_root(&root)
        .parent()
        .expect("cache root parent")
        .join("artifacts/semantic-tree-sitter-query");
    let mut removed_artifact_count = 0;
    for entry in std::fs::read_dir(&syntax_artifact_dir).expect("read syntax artifact dir") {
        let path = entry.expect("syntax artifact entry").path();
        std::fs::remove_file(path).expect("remove syntax artifact");
        removed_artifact_count += 1;
    }
    assert!(removed_artifact_count > 0);

    let third = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .env("PATH", &bin_dir)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "query",
            "--treesitter-query",
            query,
            "--selector",
            "src/lib.rs:1:3",
            ".",
            "--receipt-json",
        ])
        .output()
        .expect("run syntax query row hit");
    assert!(
        third.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&third.stderr)
    );
    let third_stdout = String::from_utf8(third.stdout).expect("third stdout");
    assert!(third_stdout.contains("parse_query"), "{third_stdout}");
    let third_receipt: Value = serde_json::from_slice(&third.stderr).expect("third receipt");
    assert_eq!(third_receipt["route"], "local-cache");
    assert_eq!(third_receipt["cacheStatus"], "hit");
    assert_eq!(third_receipt["providerCommandCount"], 0);
    assert_eq!(third_receipt["providerProcessesSpawned"], 0);
    assert_eq!(third_receipt["sqliteReadCount"], 3);
    assert_eq!(third_receipt["sqliteWriteCount"], 0);
    assert_eq!(third_receipt["clientDbSyntaxRowGenerationCount"], 1);
    assert_eq!(third_receipt["clientDbSyntaxRowMatchCount"], 1);
    assert_eq!(third_receipt["clientDbSyntaxRowCaptureCount"], 1);
    assert!(
        third_receipt["syntaxArtifactId"]
            .as_str()
            .is_some_and(|id| id.starts_with("semantic-tree-sitter-query/"))
    );
    assert!(third_receipt["packetBytes"].as_u64().unwrap_or_default() > 0);
    let provider_args_after_row_hit =
        std::fs::read_to_string(&provider_args_log).expect("read provider args after row hit");
    assert_eq!(
        provider_args_after_row_hit.lines().count(),
        provider_arg_count
    );

    let _ = std::fs::remove_dir_all(root);
}

fn attach_cache_file_hashes(packet: &str) -> String {
    let mut packet: Value = serde_json::from_str(packet).expect("packet JSON");
    packet["cache"] = serde_json::json!({
        "fileHashes": [
            {
                "path": CACHE_SOURCE_PATH,
                "sha256": CACHE_SOURCE_SHA256,
            }
        ]
    });
    serde_json::to_string(&packet).expect("serialize packet JSON")
}
