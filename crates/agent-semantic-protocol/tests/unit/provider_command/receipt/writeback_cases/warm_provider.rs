use super::syntax_replay::attach_cache_file_hashes;
use crate::provider_command::receipt::fixtures::valid_manifest;
use crate::provider_command::receipt::writeback::support::{
    PRIME_DECISION_LINE, shell_single_quote,
};
use crate::provider_command::support;
use serde_json::Value;

#[test]
fn client_search_receipt_reports_warm_provider_when_matching_generation_exists() {
    let root = support::temp_project_root("client-search-receipt-warm-provider");
    let bin_dir = root.join(".bin");
    support::write_echo_provider(&bin_dir, "rs-harness", "rs");
    support::write_activation(&root, &[support::provider("rust", Vec::new())]);
    support::write_cache_manifest(&root, valid_manifest(&root));

    let import_output = support::asp_command(&root)
        .env("PATH", &bin_dir)
        .args(["cache", "import"])
        .output()
        .expect("run asp cache import");
    assert!(
        import_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&import_output.stderr)
    );

    let output = support::asp_command(&root)
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

    let search_root = support::temp_project_root("client-search-writeback-packet");
    let search_bin_dir = search_root.join(".bin");
    let search_provider_args_log = search_root.join("provider-args.log");
    let search_packet_path = search_root.join("search-packet.json");
    support::write_cache_source_fixture(&search_root);
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
    support::write_activation(&search_root, &[support::provider("rust", Vec::new())]);

    let first_search = support::asp_command(&search_root)
        .env("PATH", &search_bin_dir)
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
    assert_eq!(first_search_receipt["dbReadCount"], 2);
    assert_eq!(first_search_receipt["dbWriteCount"], 2);
    let search_provider_args =
        std::fs::read_to_string(&search_provider_args_log).expect("read search provider args");
    assert!(
        search_provider_args.contains("search prime --view seeds --json"),
        "{search_provider_args}"
    );
    let search_provider_arg_count = search_provider_args.lines().count();
    assert_eq!(search_provider_arg_count, 1);
    let search_manifest_text =
        std::fs::read_to_string(support::cache_root(&search_root).join("cache-manifest.json"))
            .expect("read search writeback manifest");
    assert!(
        search_manifest_text.contains("search/prime"),
        "{search_manifest_text}"
    );
    assert!(
        search_manifest_text.contains("search-output/"),
        "{search_manifest_text}"
    );
    assert!(
        search_manifest_text.contains("analysis-metadata/"),
        "{search_manifest_text}"
    );
    let second_search = support::asp_command(&search_root)
        .env("PATH", &search_bin_dir)
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
    assert_eq!(second_search_receipt["dbReadCount"], 2);
    assert_eq!(second_search_receipt["dbWriteCount"], 0);
    let search_provider_args_after_hit = std::fs::read_to_string(&search_provider_args_log)
        .expect("read search provider args after hit");
    assert_eq!(
        search_provider_args_after_hit.lines().count(),
        search_provider_arg_count
    );
    let _ = std::fs::remove_dir_all(search_root);

    let query_root = support::temp_project_root("client-query-writeback-packet");
    let query_bin_dir = query_root.join(".bin");
    let provider_args_log = query_root.join("provider-args.log");
    let packet_path = query_root.join("query-packet.json");
    support::write_cache_source_fixture(&query_root);
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
    support::write_activation(&query_root, &[support::provider("rust", Vec::new())]);

    let first_query = support::asp_command(&query_root)
        .env("PATH", &query_bin_dir)
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
    assert_eq!(first_receipt["dbReadCount"], 2);
    assert_eq!(first_receipt["dbWriteCount"], 2);
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
        std::fs::read_to_string(support::cache_root(&query_root).join("cache-manifest.json"))
            .expect("read query writeback manifest");
    assert!(
        manifest_text.contains("query/owner-items"),
        "{manifest_text}"
    );
    assert!(manifest_text.contains("query/"), "{manifest_text}");
    assert!(
        manifest_text.contains(support::CACHE_SOURCE_PATH),
        "{manifest_text}"
    );
    assert!(
        manifest_text.contains(support::CACHE_SOURCE_SHA256),
        "{manifest_text}"
    );

    let second_query = support::asp_command(&query_root)
        .env("PATH", &query_bin_dir)
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
    assert_eq!(second_receipt["dbReadCount"], 2);
    assert_eq!(second_receipt["dbWriteCount"], 0);
    let provider_args_after_hit =
        std::fs::read_to_string(&provider_args_log).expect("read provider args after hit");
    assert_eq!(provider_args_after_hit.lines().count(), provider_arg_count);

    let _ = std::fs::remove_dir_all(query_root);
}
