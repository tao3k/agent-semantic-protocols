use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_client_core::{
    CacheExportMethod, ClientCacheGeneration, ClientCacheManifest, LanguageId, ProviderId,
    syntax_query_ast_abi_fingerprint,
};
use agent_semantic_client_db::{ClientDb, ClientDbGenerationLookup, ClientDbSyntaxQueryLookup};
use serde_json::json;

#[test]
fn import_semantic_tree_sitter_query_packet_rejects_code_output_rows() {
    let root = temp_root("syntax-rows-reject-code-output");
    let db_path = root.join("client.sqlite3");
    let generation = syntax_generation(&root);
    let manifest = manifest_from_generation(&root, generation.clone());
    let mut packet = syntax_packet();
    packet["query"]["fields"]["codeOutput"] = json!(true);
    let packet_bytes = serde_json::to_vec(&packet).expect("packet bytes");
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");

    db.import_manifest(&manifest).expect("import manifest");
    let error = db
        .import_semantic_tree_sitter_query_packet(&generation, &packet_bytes)
        .expect_err("--code syntax packet should not write rows");

    assert!(error.contains("syntax query rows do not store --code packet output"));
    assert!(
        lookup_syntax_rows(&db_path, &root)
            .expect("lookup after code-output import")
            .is_none()
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn flush_syntax_query_rows_keeps_cache_generations() {
    let root = temp_root("syntax-row-flush");
    let db_path = root.join("client.sqlite3");
    let generation = syntax_generation(&root);
    let manifest = manifest_from_generation(&root, generation.clone());
    let packet_bytes = serde_json::to_vec(&syntax_packet()).expect("packet bytes");
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");

    db.import_manifest(&manifest).expect("import manifest");
    db.import_semantic_tree_sitter_query_packet(&generation, &packet_bytes)
        .expect("import syntax rows");
    let summary = db.summary().expect("summary before row flush");
    assert_eq!(summary.syntax_row_generation_count, 1);
    assert_eq!(summary.syntax_row_match_count, 1);
    assert_eq!(summary.syntax_row_capture_count, 1);
    assert!(
        lookup_syntax_rows(&db_path, &root)
            .expect("lookup before row flush")
            .is_some()
    );

    assert_eq!(
        ClientDb::flush_syntax_query_rows(&db_path).expect("flush syntax rows"),
        1
    );
    assert!(
        lookup_syntax_rows(&db_path, &root)
            .expect("lookup after row flush")
            .is_none()
    );
    let report = ClientDb::inspect(&db_path);
    assert_eq!(report.syntax_row_generation_count, 0);
    assert_eq!(report.syntax_row_match_count, 0);
    assert_eq!(report.syntax_row_capture_count, 0);
    assert!(
        ClientDb::lookup_generation(&ClientDbGenerationLookup {
            db_path: db_path.clone(),
            language_id: LanguageId::from("rust"),
            provider_id: ProviderId::from("rs-harness"),
            project_root: root.clone(),
            export_method: CacheExportMethod::from("query/tree-sitter"),
            request_fingerprint: Some("fnv64:syntax-row".to_string()),
        })
        .expect("lookup generation after row flush")
        .is_some()
    );
    let _ = std::fs::remove_dir_all(root);
}

fn syntax_generation(root: &std::path::Path) -> ClientCacheGeneration {
    serde_json::from_value(json!({
        "generationId": "syntax-row",
        "languageId": "rust",
        "providerId": "rs-harness",
        "providerVersion": "0.1.0",
        "exportMethod": "query/tree-sitter",
        "projectRoot": root.display().to_string(),
        "packageRoot": ".",
        "schemaIds": ["agent.semantic-protocols.semantic-tree-sitter-query"],
        "cacheStatus": "hit",
        "rawSourceStored": false,
        "requestFingerprint": "fnv64:syntax-row",
        "fileHashes": [],
        "artifactIds": ["semantic-tree-sitter-query/syntax-row.json"]
    }))
    .expect("syntax generation")
}

fn manifest_from_generation(
    root: &std::path::Path,
    generation: ClientCacheGeneration,
) -> ClientCacheManifest {
    ClientCacheManifest {
        schema_id: "agent.semantic-protocols.client-cache-manifest".into(),
        schema_version: "1".into(),
        protocol_id: "agent.semantic-protocols.client".into(),
        protocol_version: "1".into(),
        cache_root: root.display().to_string().into(),
        generations: vec![generation],
    }
}

fn syntax_packet() -> serde_json::Value {
    json!({
        "schemaId": "agent.semantic-protocols.semantic-tree-sitter-query",
        "method": "query",
        "languageId": "rust",
        "providerId": "rs-harness",
        "grammarId": "tree-sitter-rust",
        "grammarProfileVersion": "2026-06-04.v1",
        "query": {
            "input": "(function_item name: (identifier) @function.name)",
            "inputForm": "s-expression",
            "dialect": "tree-sitter-query",
            "compiledSource": "(function_item name: (identifier) @function.name)",
            "fields": {
                "selector": "src/lib.rs:1:80",
                "codeOutput": false,
                "captures": ["function.name"]
            }
        },
        "matches": [
            {
                "id": "m1",
                "range": {"path": "src/lib.rs", "lineRange": "10:12"},
                "fields": {"nodeType": "function_item"},
                "captures": [
                    {
                        "id": "c1",
                        "name": "function.name",
                        "nodeType": "identifier",
                        "range": {"path": "src/lib.rs", "lineRange": "10:10"},
                        "fields": {"symbol": "parse_query"},
                        "field": "name"
                    }
                ]
            }
        ],
        "truncated": false,
        "cache": {
            "artifactKind": "semantic-tree-sitter-query",
            "rawSourceStored": false
        }
    })
}

fn lookup_syntax_rows(
    db_path: &std::path::Path,
    root: &std::path::Path,
) -> Result<Option<agent_semantic_client_db::ClientDbSyntaxQueryReplay>, String> {
    ClientDb::lookup_syntax_query_replay(&ClientDbSyntaxQueryLookup {
        db_path: db_path.to_path_buf(),
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("rs-harness"),
        project_root: root.to_path_buf(),
        query_ast_fingerprint: syntax_query_ast_abi_fingerprint(
            "(function_item name: (identifier) @function.name)",
        )
        .expect("syntax query AST fingerprint"),
        selector: Some("src/lib.rs:1:80".to_string()),
    })
}

fn temp_root(name: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("agent-semantic-client-db-{name}-{unique}"));
    std::fs::create_dir_all(&root).expect("create temp root");
    root
}
