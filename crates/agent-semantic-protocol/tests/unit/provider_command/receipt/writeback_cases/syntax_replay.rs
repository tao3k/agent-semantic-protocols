use crate::provider_command::support;
use serde_json::Value;
#[test]
fn client_syntax_query_writeback_hashes_locator_paths_and_replays_rows_without_artifact() {
    let root = support::temp_project_root("client-syntax-query-writeback-rows");
    let bin_dir = root.join(".bin");
    let provider_args_log = root.join("provider-args.log");
    let packet_path = root.join("syntax-query-packet.json");
    support::write_cache_source_fixture(&root);
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
    support::write_activation(&root, &[support::provider("rust", Vec::new())]);

    let query = "(function_item name: (identifier) @function.name)";
    let first = support::asp_command(&root)
        .env("PATH", &bin_dir)
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
    assert_eq!(first_receipt["dbReadCount"], 2);
    assert_eq!(first_receipt["dbWriteCount"], 3);
    assert_eq!(first_receipt["clientDbSyntaxRowGenerationCount"], 1);
    assert_eq!(first_receipt["clientDbSyntaxRowMatchCount"], 1);
    assert_eq!(first_receipt["clientDbSyntaxRowCaptureCount"], 1);
    let manifest_text =
        std::fs::read_to_string(support::cache_root(&root).join("cache-manifest.json"))
            .expect("read syntax writeback manifest");
    assert!(
        manifest_text.contains(support::CACHE_SOURCE_PATH),
        "{manifest_text}"
    );
    assert!(
        manifest_text.contains(support::CACHE_SOURCE_SHA256),
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

    let second = support::asp_command(&root)
        .env("PATH", &bin_dir)
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
    assert_eq!(second_receipt["dbReadCount"], 2);
    assert_eq!(second_receipt["dbWriteCount"], 0);
    assert_eq!(second_receipt["clientDbSyntaxRowGenerationCount"], 1);
    assert_eq!(second_receipt["clientDbSyntaxRowMatchCount"], 1);
    assert_eq!(second_receipt["clientDbSyntaxRowCaptureCount"], 1);
    assert!(
        second_receipt["syntaxArtifactId"]
            .as_str()
            .is_some_and(|id| id.starts_with("semantic-tree-sitter-query/"))
    );
    assert!(second_receipt["packetBytes"].as_u64().unwrap_or_default() > 0);

    let syntax_artifact_dir = support::artifacts_root(&root).join("semantic-tree-sitter-query");
    let mut removed_artifact_count = 0;
    for entry in std::fs::read_dir(&syntax_artifact_dir).expect("read syntax artifact dir") {
        let path = entry.expect("syntax artifact entry").path();
        std::fs::remove_file(path).expect("remove syntax artifact");
        removed_artifact_count += 1;
    }
    assert!(removed_artifact_count > 0);

    let third = support::asp_command(&root)
        .env("PATH", &bin_dir)
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
    assert_eq!(third_receipt["dbReadCount"], 3);
    assert_eq!(third_receipt["dbWriteCount"], 0);
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
                "path": support::CACHE_SOURCE_PATH,
                "sha256": support::CACHE_SOURCE_SHA256,
            }
        ]
    });
    serde_json::to_string(&packet).expect("serialize packet JSON")
}
