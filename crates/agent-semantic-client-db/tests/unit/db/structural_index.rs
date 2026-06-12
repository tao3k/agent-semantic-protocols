use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_client_core::{
    CacheArtifactId, CacheExportMethod, CacheGenerationId, ClientCacheFileHash,
    ClientCacheManifest, LanguageId, ProviderId,
};
use agent_semantic_client_db::{
    ClientDb, ClientDbStructuralDependencyUsage, ClientDbStructuralHash,
    ClientDbStructuralIndexImport, ClientDbStructuralIndexLookup, ClientDbStructuralKind,
    ClientDbStructuralLocator, ClientDbStructuralName, ClientDbStructuralOwner,
    ClientDbStructuralPath, ClientDbStructuralQueryKey, ClientDbStructuralSource,
    ClientDbStructuralSymbol,
};
use serde_json::json;

#[test]
fn structural_index_imports_queryable_rows_without_source_text() {
    let root = temp_root("structural-index");
    let db_path = root.join("client.sqlite3");
    let manifest = manifest(&root);
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");

    db.import_manifest(&manifest).expect("import manifest");
    let stats = db
        .replace_structural_index(&structural_index(&root))
        .expect("replace structural index");
    let summary = db.summary().expect("summary");
    let symbols = db
        .lookup_structural_symbols(&lookup(&root, "parse_config"))
        .expect("lookup symbols");
    let dependencies = db
        .lookup_structural_dependency_usages(&lookup(&root, "serde_json::from_str"))
        .expect("lookup dependencies");
    let source_text_columns = raw_source_like_columns(&db_path);

    assert_eq!(stats.owner_count, 1);
    assert_eq!(stats.symbol_count, 1);
    assert_eq!(stats.dependency_usage_count, 1);
    assert_eq!(summary.structural_index_generation_count, 1);
    assert_eq!(summary.structural_index_owner_count, 1);
    assert_eq!(summary.structural_index_symbol_count, 1);
    assert_eq!(summary.structural_index_dependency_usage_count, 1);
    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0].name.as_str(), "parse_config");
    assert_eq!(symbols[0].owner_path.as_str(), "src/lib.rs");
    assert_eq!(dependencies.len(), 1);
    assert_eq!(dependencies[0].package_name.as_str(), "serde_json");
    assert_eq!(
        dependencies[0]
            .api_name
            .as_ref()
            .map(ClientDbStructuralName::as_str),
        Some("from_str")
    );
    assert!(source_text_columns.is_empty(), "{source_text_columns:?}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn structural_index_requires_file_hash_evidence() {
    let root = temp_root("structural-index-no-hash");
    let db_path = root.join("client.sqlite3");
    let manifest = manifest(&root);
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");
    let mut import = structural_index(&root);

    db.import_manifest(&manifest).expect("import manifest");
    import.file_hashes.clear();
    let error = db
        .replace_structural_index(&import)
        .expect_err("reject missing hashes");

    assert!(error.contains("file hash evidence"), "{error}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn semantic_structural_index_packet_imports_provider_rows() {
    let root = temp_root("structural-index-packet");
    let db_path = root.join("client.sqlite3");
    let manifest = manifest(&root);
    let packet = serde_json::to_vec(&structural_index_packet(&root)).expect("packet bytes");
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");

    db.import_manifest(&manifest).expect("import manifest");
    let stats = db
        .import_semantic_structural_index_packet(&manifest.generations[0], &packet)
        .expect("import structural packet");
    let symbols = db
        .lookup_structural_symbols(&lookup(&root, "crate::parse_config"))
        .expect("lookup qualified symbol");
    let dependencies = db
        .lookup_structural_dependency_usages(&lookup(&root, "serde_json::from_str"))
        .expect("lookup dependencies");

    assert_eq!(stats.owner_count, 1);
    assert_eq!(stats.symbol_count, 1);
    assert_eq!(stats.dependency_usage_count, 1);
    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0].name.as_str(), "parse_config");
    assert_eq!(
        dependencies[0]
            .source_locator
            .as_ref()
            .map(ClientDbStructuralLocator::as_str),
        Some("src/lib.rs:8:8")
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn semantic_structural_index_packet_rejects_raw_source_row_fields() {
    let root = temp_root("structural-index-raw-source-row");
    let db_path = root.join("client.sqlite3");
    let manifest = manifest(&root);
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");

    db.import_manifest(&manifest).expect("import manifest");
    for (rows_field, raw_source_field) in [
        ("owners", "content"),
        ("symbols", "code"),
        ("dependencyUsages", "snippet"),
    ] {
        let mut packet = structural_index_packet(&root);
        packet
            .get_mut(rows_field)
            .and_then(serde_json::Value::as_array_mut)
            .and_then(|rows| rows.first_mut())
            .and_then(serde_json::Value::as_object_mut)
            .expect("row object")
            .insert(
                raw_source_field.to_string(),
                json!("pub fn cached_source_body() {}"),
            );
        let packet = serde_json::to_vec(&packet).expect("packet bytes");
        let error = db
            .import_semantic_structural_index_packet(&manifest.generations[0], &packet)
            .expect_err("reject raw source row field");

        assert!(error.contains("raw source field"), "{error}");
        assert!(error.contains(raw_source_field), "{error}");
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn invalidating_cache_generation_cascades_structural_index_rows() {
    let root = temp_root("structural-index-cascade");
    let db_path = root.join("client.sqlite3");
    let manifest = manifest(&root);
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");

    db.import_manifest(&manifest).expect("import manifest");
    db.replace_structural_index(&structural_index(&root))
        .expect("replace structural index");
    drop(db);

    let invalidated = ClientDb::invalidate_generations(&db_path).expect("invalidate generations");
    let db = ClientDb::open_read_only_existing(&db_path)
        .expect("open db")
        .expect("db exists");
    let summary = db.summary().expect("summary");

    assert_eq!(invalidated, 1);
    assert_eq!(summary.generation_count, 0);
    assert_eq!(summary.structural_index_generation_count, 0);
    assert_eq!(summary.structural_index_owner_count, 0);
    assert_eq!(summary.structural_index_symbol_count, 0);
    assert_eq!(summary.structural_index_dependency_usage_count, 0);
    let _ = std::fs::remove_dir_all(root);
}

fn structural_index(root: &std::path::Path) -> ClientDbStructuralIndexImport {
    ClientDbStructuralIndexImport {
        generation_id: CacheGenerationId::from("rust-main-1"),
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("rs-harness"),
        provider_version: Some(ClientDbStructuralName::from("0.1.0")),
        export_method: Some(CacheExportMethod::from("index/structural")),
        project_root: root.to_path_buf(),
        package_root: Some(ClientDbStructuralPath::from(".")),
        schema_id: "agent.semantic-protocols.semantic-structural-index".into(),
        schema_version: "1".into(),
        source_artifact_id: Some(CacheArtifactId::from("structural-index/rust-main-1.json")),
        file_hashes: vec![ClientCacheFileHash {
            path: "src/lib.rs".to_string(),
            sha256: "0".repeat(64),
        }],
        owners: vec![ClientDbStructuralOwner {
            owner_path: ClientDbStructuralPath::from("src/lib.rs"),
            owner_kind: ClientDbStructuralKind::from("source"),
            source_authority: ClientDbStructuralSource::from("native-parser"),
            start_line: Some(1),
            end_line: Some(40),
            query_keys: vec![
                ClientDbStructuralQueryKey::from("config"),
                ClientDbStructuralQueryKey::from("parse_config"),
            ],
        }],
        symbols: vec![ClientDbStructuralSymbol {
            owner_path: ClientDbStructuralPath::from("src/lib.rs"),
            name: ClientDbStructuralName::from("parse_config"),
            kind: ClientDbStructuralKind::from("function"),
            visibility: Some(ClientDbStructuralKind::from("public")),
            source_locator: Some(ClientDbStructuralLocator::from("src/lib.rs:3:12")),
            query_keys: vec![
                ClientDbStructuralQueryKey::from("parse"),
                ClientDbStructuralQueryKey::from("config"),
            ],
        }],
        dependency_usages: vec![ClientDbStructuralDependencyUsage {
            owner_path: ClientDbStructuralPath::from("src/lib.rs"),
            package_name: ClientDbStructuralName::from("serde_json"),
            package_version: Some(ClientDbStructuralName::from("1.0.0")),
            api_name: Some(ClientDbStructuralName::from("from_str")),
            import_path: Some(ClientDbStructuralPath::from("serde_json::from_str")),
            manifest_path: Some(ClientDbStructuralPath::from("Cargo.toml")),
            lockfile_hash: Some(ClientDbStructuralHash::new(
                "sha256:".to_string() + &"1".repeat(64),
            )),
            source: ClientDbStructuralSource::from("manifest+native-parser"),
            source_locator: Some(ClientDbStructuralLocator::from("src/lib.rs:8:8")),
            query_keys: vec![
                ClientDbStructuralQueryKey::from("serde_json::from_str"),
                ClientDbStructuralQueryKey::from("json parse"),
            ],
        }],
    }
}

fn structural_index_packet(root: &std::path::Path) -> serde_json::Value {
    json!({
        "schemaId": "agent.semantic-protocols.semantic-structural-index",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "generationId": "rust-main-1",
        "languageId": "rust",
        "providerId": "rs-harness",
        "providerVersion": "0.1.0",
        "exportMethod": "index/structural",
        "projectRoot": root.display().to_string(),
        "packageRoot": ".",
        "sourceAuthority": "native-parser",
        "sourceArtifactId": "structural-index/rust-main-1.json",
        "rawSourceStored": false,
        "fileHashes": [
            {
                "path": "src/lib.rs",
                "sha256": "0000000000000000000000000000000000000000000000000000000000000000",
                "source": "provider"
            }
        ],
        "owners": [
            {
                "ownerPath": "src/lib.rs",
                "ownerKind": "source",
                "sourceAuthority": "native-parser",
                "location": {"path": "src/lib.rs", "lineRange": "1:40"},
                "queryKeys": ["parse_config", "config"]
            }
        ],
        "symbols": [
            {
                "ownerPath": "src/lib.rs",
                "name": "parse_config",
                "qualifiedName": "crate::parse_config",
                "kind": "function",
                "visibility": "public",
                "sourceLocator": "src/lib.rs:3:12",
                "queryKeys": ["parse", "config"]
            }
        ],
        "dependencyUsages": [
            {
                "ownerPath": "src/lib.rs",
                "packageName": "serde_json",
                "packageVersion": "1.0.0",
                "apiName": "from_str",
                "importPath": "serde_json::from_str",
                "manifestPath": "Cargo.toml",
                "lockfileHash": "sha256:1111111111111111111111111111111111111111111111111111111111111111",
                "source": "manifest+native-parser",
                "sourceLocator": "src/lib.rs:8:8",
                "queryKeys": ["serde_json::from_str", "json parse"]
            }
        ]
    })
}

fn lookup(root: &std::path::Path, query: &str) -> ClientDbStructuralIndexLookup {
    ClientDbStructuralIndexLookup {
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("rs-harness"),
        project_root: root.to_path_buf(),
        query: ClientDbStructuralQueryKey::from(query),
        limit: 8,
    }
}

fn manifest(root: &std::path::Path) -> ClientCacheManifest {
    serde_json::from_value(json!({
        "schemaId": "agent.semantic-protocols.client-cache-manifest",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.client",
        "protocolVersion": "1",
        "cacheRoot": root.display().to_string(),
        "generations": [
            {
                "generationId": "rust-main-1",
                "languageId": "rust",
                "providerId": "rs-harness",
                "providerVersion": "0.1.0",
                "exportMethod": "index/structural",
                "projectRoot": root.display().to_string(),
                "packageRoot": ".",
                "schemaIds": ["agent.semantic-protocols.semantic-structural-index"],
                "cacheStatus": "miss",
                "rawSourceStored": false,
                "requestFingerprint": "fnv64:0123456789abcdef",
                "fileHashes": [{"path": "src/lib.rs", "sha256": "0000000000000000000000000000000000000000000000000000000000000000"}],
                "artifactIds": ["structural-index/rust-main-1.json"]
            }
        ]
    }))
    .expect("manifest")
}

fn raw_source_like_columns(db_path: &std::path::Path) -> Vec<String> {
    let conn = rusqlite::Connection::open(db_path).expect("open sqlite");
    let mut statement = conn
        .prepare(
            "SELECT m.name, p.name
             FROM sqlite_master m
             JOIN pragma_table_info(m.name) p
             WHERE m.type = 'table'
               AND m.name LIKE 'structural_index_%'
               AND (
                    p.name LIKE '%source_text%'
                    OR p.name LIKE '%code%'
                    OR p.name LIKE '%snippet%'
                    OR p.name LIKE '%window%'
               )
             ORDER BY m.name, p.name",
        )
        .expect("prepare table info");
    statement
        .query_map([], |row| {
            Ok(format!(
                "{}.{}",
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?
            ))
        })
        .expect("query table info")
        .map(|row| row.expect("table info row"))
        .collect()
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
