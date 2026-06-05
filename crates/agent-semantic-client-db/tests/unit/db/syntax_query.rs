use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_client_core::{
    CacheExportMethod, ClientCacheGeneration, ClientCacheManifest, LanguageId, ProviderId,
};
use agent_semantic_client_db::{
    ClientDb, ClientDbGenerationLookup, ClientDbSyntaxQueryInputKind, ClientDbSyntaxQueryLookup,
};
use serde_json::json;

#[test]
fn lookup_generation_can_filter_by_request_fingerprint() {
    let root = temp_root("fingerprint");
    let db_path = root.join("client.sqlite3");
    let manifest = fingerprint_manifest(&root);
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");

    db.import_manifest(&manifest).expect("import manifest");

    let hit = ClientDb::lookup_generation(&ClientDbGenerationLookup {
        db_path: db_path.clone(),
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("rs-harness"),
        project_root: root.clone(),
        export_method: CacheExportMethod::from("query/tree-sitter"),
        request_fingerprint: Some("fnv64:exact-b".to_string()),
    })
    .expect("lookup generation")
    .expect("fingerprint hit");

    assert_eq!(
        hit.artifact_ids[0].as_str(),
        "semantic-tree-sitter-query/b.json"
    );
    assert!(
        ClientDb::lookup_generation(&ClientDbGenerationLookup {
            db_path: db_path.clone(),
            language_id: LanguageId::from("rust"),
            provider_id: ProviderId::from("rs-harness"),
            project_root: root.clone(),
            export_method: CacheExportMethod::from("query/tree-sitter"),
            request_fingerprint: Some("fnv64:missing".to_string()),
        })
        .expect("missing fingerprint")
        .is_none()
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn import_semantic_tree_sitter_query_packet_writes_replay_rows() {
    let root = temp_root("syntax-rows");
    let db_path = root.join("client.sqlite3");
    let generation = syntax_generation(
        &root,
        "syntax-row",
        "fnv64:syntax-row",
        "semantic-tree-sitter-query/syntax-row.json",
    );
    let manifest = manifest_from_generation(&root, generation.clone());
    let packet = syntax_packet();
    let packet_bytes = serde_json::to_vec(&packet).expect("packet bytes");
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");

    db.import_manifest(&manifest).expect("import manifest");
    db.import_semantic_tree_sitter_query_packet(&generation, &packet_bytes)
        .expect("import syntax rows");

    let replay = ClientDb::lookup_syntax_query_replay(&ClientDbSyntaxQueryLookup {
        db_path: db_path.clone(),
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("rs-harness"),
        project_root: root.clone(),
        request_fingerprint: "fnv64:syntax-row".to_string(),
    })
    .expect("lookup syntax rows")
    .expect("syntax rows");

    assert_eq!(replay.grammar_id, "tree-sitter-rust");
    assert_eq!(replay.grammar_profile_version, "2026-06-04.v1");
    assert_eq!(replay.input_form, "s-expression");
    assert_eq!(replay.input_kind, ClientDbSyntaxQueryInputKind::Inline);
    assert_eq!(replay.captures, vec!["function.name"]);
    assert_eq!(
        replay
            .artifact_id
            .as_ref()
            .map(|artifact| artifact.as_str()),
        Some("semantic-tree-sitter-query/syntax-row.json")
    );
    assert_eq!(
        replay.packet_bytes,
        Some(packet_bytes.len().min(u64::MAX as usize) as u64)
    );
    assert_eq!(replay.rows.len(), 2);
    assert_eq!(replay.rows[0].locator, "src/lib.rs:10:12");
    assert_eq!(replay.rows[0].text, "parse_query");
    assert_eq!(replay.rows[1].locator, "src/main.rs:20");
    assert_eq!(replay.rows[1].text, "main");
    let _ = std::fs::remove_dir_all(root);
}

fn fingerprint_manifest(root: &std::path::Path) -> ClientCacheManifest {
    serde_json::from_value(json!({
        "schemaId": "agent.semantic-protocols.client-cache-manifest",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.client",
        "protocolVersion": "1",
        "cacheRoot": root.display().to_string(),
        "generations": [
            {
                "generationId": "syntax-a",
                "languageId": "rust",
                "providerId": "rs-harness",
                "providerVersion": "0.1.0",
                "exportMethod": "query/tree-sitter",
                "projectRoot": root.display().to_string(),
                "packageRoot": ".",
                "schemaIds": ["agent.semantic-protocols.semantic-tree-sitter-query"],
                "cacheStatus": "miss",
                "rawSourceStored": false,
                "requestFingerprint": "fnv64:exact-a",
                "fileHashes": [],
                "artifactIds": ["semantic-tree-sitter-query/a.json"]
            },
            {
                "generationId": "syntax-b",
                "languageId": "rust",
                "providerId": "rs-harness",
                "providerVersion": "0.1.0",
                "exportMethod": "query/tree-sitter",
                "projectRoot": root.display().to_string(),
                "packageRoot": ".",
                "schemaIds": ["agent.semantic-protocols.semantic-tree-sitter-query"],
                "cacheStatus": "miss",
                "rawSourceStored": false,
                "requestFingerprint": "fnv64:exact-b",
                "fileHashes": [],
                "artifactIds": ["semantic-tree-sitter-query/b.json"]
            }
        ]
    }))
    .expect("manifest")
}

fn syntax_generation(
    root: &std::path::Path,
    generation_id: &str,
    request_fingerprint: &str,
    artifact_id: &str,
) -> ClientCacheGeneration {
    serde_json::from_value(json!({
        "generationId": generation_id,
        "languageId": "rust",
        "providerId": "rs-harness",
        "providerVersion": "0.1.0",
        "exportMethod": "query/tree-sitter",
        "projectRoot": root.display().to_string(),
        "packageRoot": ".",
        "schemaIds": ["agent.semantic-protocols.semantic-tree-sitter-query"],
        "cacheStatus": "hit",
        "rawSourceStored": false,
        "requestFingerprint": request_fingerprint,
        "fileHashes": [],
        "artifactIds": [artifact_id]
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
                "nativeFactRefs": ["rust:item:src/lib.rs:10:12:parse_query"],
                "captures": [
                    {
                        "id": "c1",
                        "name": "function.name",
                        "nodeType": "identifier",
                        "range": {"path": "src/lib.rs", "lineRange": "10:10"},
                        "nativeFactRefs": ["rust:item:src/lib.rs:10:12:parse_query"],
                        "fields": {"symbol": "parse_query"}
                    }
                ]
            },
            {
                "id": "m2",
                "captures": [
                    {
                        "id": "c2",
                        "name": "function.name",
                        "nodeType": "identifier",
                        "range": {"path": "src/main.rs", "lineRange": {"start": 20, "end": 20}},
                        "fields": {"name": "main"}
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

fn temp_root(name: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("agent-semantic-client-db-{name}-{unique}"));
    std::fs::create_dir_all(&root).expect("create temp root");
    root
}
