use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_client_core::{
    ClientCacheGeneration, ClientCacheManifest, LanguageId, ProviderId,
    syntax_query_ast_abi_fingerprint,
};
use agent_semantic_client_db::{ClientDb, ClientDbSyntaxQueryLookup};
use serde_json::json;

#[test]
fn syntax_query_replay_stores_symbols_not_source_text() {
    let root = temp_root("syntax-structural-rows");
    let db_path = root.join("client.sqlite3");
    let generation = syntax_generation(&root);
    let manifest = manifest_from_generation(&root, generation.clone());
    let mut packet = syntax_packet_with_symbols("parse_query", "main");
    let parse_source = "fn parse_query(input: &str) -> Query { input.parse().unwrap() }";
    let main_source = "fn main() { let _ = parse_query(\"body text should not cache\"); }";

    packet["matches"][0]["source"] = json!(parse_source);
    packet["matches"][0]["text"] = json!(parse_source);
    packet["matches"][0]["captures"][0]["source"] = json!(parse_source);
    packet["matches"][0]["captures"][0]["text"] = json!(parse_source);
    packet["matches"][0]["captures"][0]["fields"]["source"] = json!(parse_source);
    packet["matches"][0]["captures"][0]["fields"]["text"] = json!(parse_source);
    packet["matches"][1]["source"] = json!(main_source);
    packet["matches"][1]["text"] = json!(main_source);
    packet["matches"][1]["captures"][0]["source"] = json!(main_source);
    packet["matches"][1]["captures"][0]["text"] = json!(main_source);
    packet["matches"][1]["captures"][0]["fields"]["source"] = json!(main_source);
    packet["matches"][1]["captures"][0]["fields"]["text"] = json!(main_source);

    let packet_bytes = serde_json::to_vec(&packet).expect("packet bytes");
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");

    db.import_manifest(&manifest).expect("import manifest");
    db.import_semantic_tree_sitter_query_packet(&generation, &packet_bytes)
        .expect("import syntax rows");
    let replay = lookup_syntax_rows(&db_path, &root)
        .expect("lookup syntax rows")
        .expect("syntax rows");

    assert_eq!(replay.rows.len(), 2);
    assert_eq!(replay.rows[0].text, "parse_query");
    assert_eq!(replay.rows[1].text, "main");
    for row in &replay.rows {
        assert!(!row.text.contains("fn "));
        assert!(!row.text.contains('{'));
        assert!(!row.text.contains("parse().unwrap"));
        assert!(!row.text.contains("body text should not cache"));
    }
    let _ = std::fs::remove_dir_all(root);
}

fn syntax_generation(root: &std::path::Path) -> ClientCacheGeneration {
    serde_json::from_value(json!({
        "generationId": "syntax-structural",
        "languageId": "rust",
        "providerId": "rs-harness",
        "providerVersion": "0.1.0",
        "exportMethod": "query/tree-sitter",
        "projectRoot": root.display().to_string(),
        "packageRoot": ".",
        "schemaIds": ["agent.semantic-protocols.semantic-tree-sitter-query"],
        "cacheStatus": "hit",
        "rawSourceStored": false,
        "requestFingerprint": "fnv64:syntax-structural",
        "fileHashes": [],
        "artifactIds": ["semantic-tree-sitter-query/syntax-structural.json"]
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

fn syntax_packet_with_symbols(first_symbol: &str, second_symbol: &str) -> serde_json::Value {
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
                        "fields": {"symbol": first_symbol},
                        "field": "name"
                    }
                ]
            },
            {
                "id": "m2",
                "fields": {"nodeType": "function_item"},
                "captures": [
                    {
                        "id": "c2",
                        "name": "function.name",
                        "nodeType": "identifier",
                        "range": {"path": "src/main.rs", "lineRange": {"start": 20, "end": 20}},
                        "fields": {"name": second_symbol},
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
