use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_client_core::{
    CacheExportMethod, ClientCacheGeneration, ClientCacheManifest, LanguageId, ProviderId,
    syntax_query_ast_abi_fingerprint,
};
use agent_semantic_client_db::{
    ClientDb, ClientDbGenerationLookup, ClientDbSyntaxQueryInputKind, ClientDbSyntaxQueryLookup,
};
use rusqlite::params;
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
    let summary = db.summary().expect("db summary after syntax import");

    let replay = ClientDb::lookup_syntax_query_replay(&ClientDbSyntaxQueryLookup {
        db_path: db_path.clone(),
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("rs-harness"),
        project_root: root.clone(),
        query_ast_fingerprint: syntax_query_ast_abi_fingerprint(
            "(function_item name: (identifier) @function.name)",
        )
        .expect("syntax query AST fingerprint"),
        selector: Some("src/lib.rs:1:80".to_string()),
    })
    .expect("lookup syntax rows")
    .expect("syntax rows");

    assert_eq!(replay.grammar_id, "tree-sitter-rust");
    assert_eq!(summary.syntax_row_generation_count, 1);
    assert_eq!(summary.syntax_row_match_count, 2);
    assert_eq!(summary.syntax_row_capture_count, 2);
    assert_eq!(replay.language_id.as_str(), "rust");
    assert_eq!(replay.grammar_profile_version, "2026-06-04.v1");
    assert_eq!(replay.input_form, "s-expression");
    assert_eq!(replay.input_kind, ClientDbSyntaxQueryInputKind::Inline);
    assert_eq!(
        replay.compiled_source,
        "(function_item name: (identifier) @function.name)"
    );
    assert!(
        replay
            .query_ast_fingerprint
            .starts_with("syntax-query-ast-abi:")
    );
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
    assert_eq!(replay.rows[0].match_locator, "src/lib.rs:10:12");
    assert_eq!(replay.rows[0].capture_locator, "src/lib.rs:10");
    assert_eq!(replay.rows[0].capture_name, "function.name");
    assert_eq!(replay.rows[0].capture_node_type.as_str(), "identifier");
    assert_eq!(replay.rows[0].item_node_type.as_str(), "function_item");
    assert_eq!(replay.rows[0].field.as_deref(), Some("name"));
    assert_eq!(replay.rows[0].text, "parse_query");
    assert_eq!(replay.rows[1].match_locator, "src/main.rs:20");
    assert_eq!(replay.rows[1].capture_locator, "src/main.rs:20");
    assert_eq!(replay.rows[1].capture_name, "function.name");
    assert_eq!(replay.rows[1].capture_node_type.as_str(), "identifier");
    assert_eq!(replay.rows[1].item_node_type.as_str(), "function_item");
    assert_eq!(replay.rows[1].field.as_deref(), Some("name"));
    assert_eq!(replay.rows[1].text, "main");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn reimport_manifest_preserves_semantic_tree_sitter_query_rows() {
    let root = temp_root("syntax-reimport-preserves-rows");
    let db_path = root.join("client.sqlite3");
    let generation = syntax_generation(
        &root,
        "syntax-row",
        "fnv64:syntax-row",
        "semantic-tree-sitter-query/syntax-row.json",
    );
    let manifest = manifest_from_generation(&root, generation.clone());
    let packet_bytes = serde_json::to_vec(&syntax_packet()).expect("packet bytes");
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");

    db.import_manifest(&manifest).expect("import manifest");
    db.import_semantic_tree_sitter_query_packet(&generation, &packet_bytes)
        .expect("import syntax rows");
    db.import_manifest(&manifest)
        .expect("reimport manifest without replacing parent row");

    let summary = db.summary().expect("summary after manifest reimport");
    assert_eq!(summary.syntax_row_generation_count, 1);
    assert_eq!(summary.syntax_row_match_count, 2);
    assert_eq!(summary.syntax_row_capture_count, 2);
    assert!(
        lookup_syntax_rows(&db_path, &root, "fnv64:syntax-row")
            .expect("lookup after manifest reimport")
            .is_some()
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn import_semantic_tree_sitter_query_packet_requires_ast_abi_plan() {
    let root = temp_root("syntax-rows-require-ast-abi");
    let db_path = root.join("client.sqlite3");
    let generation = syntax_generation(
        &root,
        "syntax-row",
        "fnv64:syntax-row",
        "semantic-tree-sitter-query/syntax-row.json",
    );
    let manifest = manifest_from_generation(&root, generation.clone());
    let mut packet = syntax_packet();
    packet["query"]["compiledSource"] = json!("(function_item name: (identifier) @function.name");
    let packet_bytes = serde_json::to_vec(&packet).expect("packet bytes");
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");

    db.import_manifest(&manifest).expect("import manifest");
    let error = db
        .import_semantic_tree_sitter_query_packet(&generation, &packet_bytes)
        .expect_err("invalid AST/ABI packet should not write rows");

    assert!(error.contains("syntax query rows require AST/ABI plan"));
    assert!(
        lookup_syntax_rows(&db_path, &root, "fnv64:syntax-row")
            .expect("lookup after invalid AST/ABI import")
            .is_none()
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn invalidate_generations_removes_semantic_tree_sitter_query_rows() {
    let root = temp_root("syntax-invalidate");
    let db_path = root.join("client.sqlite3");
    let generation = syntax_generation(
        &root,
        "syntax-row",
        "fnv64:syntax-row",
        "semantic-tree-sitter-query/syntax-row.json",
    );
    let manifest = manifest_from_generation(&root, generation.clone());
    let packet_bytes = serde_json::to_vec(&syntax_packet()).expect("packet bytes");
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");

    db.import_manifest(&manifest).expect("import manifest");
    db.import_semantic_tree_sitter_query_packet(&generation, &packet_bytes)
        .expect("import syntax rows");
    assert!(
        lookup_syntax_rows(&db_path, &root, "fnv64:syntax-row")
            .expect("lookup before invalidate")
            .is_some()
    );

    assert_eq!(
        ClientDb::invalidate_generations(&db_path).expect("invalidate"),
        1
    );
    assert!(
        lookup_syntax_rows(&db_path, &root, "fnv64:syntax-row")
            .expect("lookup after invalidate")
            .is_none()
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn repeated_semantic_tree_sitter_query_generation_refreshes_replay_rows() {
    let root = temp_root("syntax-refresh");
    let db_path = root.join("client.sqlite3");
    let old_generation = syntax_generation(
        &root,
        "syntax-row-old",
        "fnv64:syntax-row",
        "semantic-tree-sitter-query/syntax-row-old.json",
    );
    let new_generation = syntax_generation(
        &root,
        "syntax-row-new",
        "fnv64:syntax-row",
        "semantic-tree-sitter-query/syntax-row-new.json",
    );
    let old_packet = serde_json::to_vec(&syntax_packet_with_symbols("parse_query", "main"))
        .expect("old packet bytes");
    let new_packet = serde_json::to_vec(&syntax_packet_with_symbols("parse_query_v2", "main_v2"))
        .expect("new packet bytes");
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");

    db.import_manifest(&manifest_from_generation(&root, old_generation.clone()))
        .expect("import old manifest");
    db.import_semantic_tree_sitter_query_packet(&old_generation, &old_packet)
        .expect("import old rows");
    db.import_manifest(&manifest_from_generation(&root, new_generation.clone()))
        .expect("import new manifest");
    db.import_semantic_tree_sitter_query_packet(&new_generation, &new_packet)
        .expect("import new rows");

    let replay = lookup_syntax_rows(&db_path, &root, "fnv64:syntax-row")
        .expect("lookup syntax rows")
        .expect("refreshed rows");

    assert_eq!(replay.generation_id.as_str(), "syntax-row-new");
    assert_eq!(
        replay
            .artifact_id
            .as_ref()
            .map(agent_semantic_client_core::CacheArtifactId::as_str),
        Some("semantic-tree-sitter-query/syntax-row-new.json")
    );
    assert_eq!(replay.rows[0].text, "parse_query_v2");
    assert_eq!(replay.rows[1].text, "main_v2");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn stale_syntax_query_row_abi_flushes_only_syntax_rows() {
    let root = temp_root("syntax-row-abi-flush");
    let db_path = root.join("client.sqlite3");
    let generation = syntax_generation(
        &root,
        "syntax-row",
        "fnv64:syntax-row",
        "semantic-tree-sitter-query/syntax-row.json",
    );
    let packet_bytes = serde_json::to_vec(&syntax_packet()).expect("packet bytes");
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");

    db.import_manifest(&manifest_from_generation(&root, generation.clone()))
        .expect("import manifest");
    db.import_semantic_tree_sitter_query_packet(&generation, &packet_bytes)
        .expect("import syntax rows");
    assert!(
        lookup_syntax_rows(&db_path, &root, "fnv64:syntax-row")
            .expect("lookup before stale ABI")
            .is_some()
    );
    drop(db);

    let conn = rusqlite::Connection::open(&db_path).expect("open raw sqlite");
    conn.execute(
        "INSERT OR REPLACE INTO schema_meta (key, value) VALUES (?1, ?2)",
        params!["syntaxQueryRowAbiVersion", "stale-row-abi"],
    )
    .expect("mark stale row ABI");
    drop(conn);

    let _db = ClientDb::open_or_create(&db_path).expect("reopen db and flush stale rows");
    assert!(
        lookup_syntax_rows(&db_path, &root, "fnv64:syntax-row")
            .expect("lookup after stale ABI flush")
            .is_none()
    );
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
    syntax_packet_with_symbols("parse_query", "main")
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
                "nativeFactRefs": ["rust:item:src/lib.rs:10:12:parse_query"],
                "captures": [
                    {
                        "id": "c1",
                        "name": "function.name",
                        "nodeType": "identifier",
                        "range": {"path": "src/lib.rs", "lineRange": "10:10"},
                        "nativeFactRefs": ["rust:item:src/lib.rs:10:12:parse_query"],
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
    request_fingerprint: &str,
) -> Result<Option<agent_semantic_client_db::ClientDbSyntaxQueryReplay>, String> {
    let _ = request_fingerprint;
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
