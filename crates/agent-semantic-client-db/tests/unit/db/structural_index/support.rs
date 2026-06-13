use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_client_core::{
    CacheArtifactId, CacheExportMethod, CacheGenerationId, ClientCacheFileHash,
    ClientCacheManifest, LanguageId, ProviderId,
};
use agent_semantic_client_db::{
    ClientDbStructuralDependencyUsage, ClientDbStructuralHash, ClientDbStructuralIndexImport,
    ClientDbStructuralIndexLookup, ClientDbStructuralKind, ClientDbStructuralLocator,
    ClientDbStructuralName, ClientDbStructuralOwner, ClientDbStructuralPath,
    ClientDbStructuralQueryKey, ClientDbStructuralSource, ClientDbStructuralSymbol,
};
use serde_json::json;

pub(super) fn structural_index(root: &std::path::Path) -> ClientDbStructuralIndexImport {
    structural_index_with_generation(root, "rust-main-1")
}

pub(super) fn structural_index_with_generation(
    root: &std::path::Path,
    generation_id: &str,
) -> ClientDbStructuralIndexImport {
    ClientDbStructuralIndexImport {
        generation_id: CacheGenerationId::from(generation_id),
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("rs-harness"),
        provider_version: Some(ClientDbStructuralName::from("0.1.0")),
        export_method: Some(CacheExportMethod::from("index/structural")),
        project_root: root.to_path_buf(),
        package_root: Some(ClientDbStructuralPath::from(".")),
        schema_id: "agent.semantic-protocols.semantic-structural-index".into(),
        schema_version: "1".into(),
        source_artifact_id: Some(CacheArtifactId::from(format!(
            "structural-index/{generation_id}.json"
        ))),
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

pub(super) fn add_cached_helper_file(import: &mut ClientDbStructuralIndexImport) {
    import.file_hashes.push(ClientCacheFileHash {
        path: "src/unchanged.rs".to_string(),
        sha256: "1".repeat(64),
    });
    import.owners.push(ClientDbStructuralOwner {
        owner_path: ClientDbStructuralPath::from("src/unchanged.rs"),
        owner_kind: ClientDbStructuralKind::from("source"),
        source_authority: ClientDbStructuralSource::from("native-parser"),
        start_line: Some(1),
        end_line: Some(20),
        query_keys: vec![ClientDbStructuralQueryKey::from("cached_helper")],
    });
    import.symbols.push(ClientDbStructuralSymbol {
        owner_path: ClientDbStructuralPath::from("src/unchanged.rs"),
        name: ClientDbStructuralName::from("cached_helper"),
        kind: ClientDbStructuralKind::from("function"),
        visibility: Some(ClientDbStructuralKind::from("private")),
        source_locator: Some(ClientDbStructuralLocator::from("src/unchanged.rs:4:4")),
        query_keys: vec![ClientDbStructuralQueryKey::from("cached_helper")],
    });
    import
        .dependency_usages
        .push(ClientDbStructuralDependencyUsage {
            owner_path: ClientDbStructuralPath::from("src/unchanged.rs"),
            package_name: ClientDbStructuralName::from("anyhow"),
            package_version: Some(ClientDbStructuralName::from("1.0.0")),
            api_name: Some(ClientDbStructuralName::from("Result")),
            import_path: Some(ClientDbStructuralPath::from("anyhow::Result")),
            manifest_path: Some(ClientDbStructuralPath::from("Cargo.toml")),
            lockfile_hash: Some(ClientDbStructuralHash::new(
                "sha256:".to_string() + &"2".repeat(64),
            )),
            source: ClientDbStructuralSource::from("manifest+native-parser"),
            source_locator: Some(ClientDbStructuralLocator::from("src/unchanged.rs:2:5")),
            query_keys: vec![ClientDbStructuralQueryKey::from("anyhow::Result")],
        });
}

pub(super) fn structural_index_packet(root: &std::path::Path) -> serde_json::Value {
    structural_index_packet_with_generation(root, "rust-main-1")
}

pub(super) fn structural_index_packet_with_generation(
    root: &std::path::Path,
    generation_id: &str,
) -> serde_json::Value {
    json!({
        "schemaId": "agent.semantic-protocols.semantic-structural-index",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "generationId": generation_id,
        "languageId": "rust",
        "providerId": "rs-harness",
        "providerVersion": "0.1.0",
        "exportMethod": "index/structural",
        "projectRoot": root.display().to_string(),
        "packageRoot": ".",
        "sourceAuthority": "native-parser",
        "sourceArtifactId": format!("structural-index/{generation_id}.json"),
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

pub(super) fn lookup(root: &std::path::Path, query: &str) -> ClientDbStructuralIndexLookup {
    ClientDbStructuralIndexLookup {
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("rs-harness"),
        project_root: root.to_path_buf(),
        query: ClientDbStructuralQueryKey::from(query),
        limit: 8,
    }
}

pub(super) fn manifest(root: &std::path::Path) -> ClientCacheManifest {
    manifest_with_generations(root, &["rust-main-1"])
}

pub(super) fn manifest_with_generations(
    root: &std::path::Path,
    generation_ids: &[&str],
) -> ClientCacheManifest {
    let generations = generation_ids
        .iter()
        .enumerate()
        .map(|(generation_ordinal, generation_id)| {
            json!({
                "generationId": generation_id,
                "languageId": "rust",
                "providerId": "rs-harness",
                "providerVersion": "0.1.0",
                "exportMethod": "index/structural",
                "projectRoot": root.display().to_string(),
                "packageRoot": ".",
                "schemaIds": ["agent.semantic-protocols.semantic-structural-index"],
                "cacheStatus": "miss",
                "rawSourceStored": false,
                "requestFingerprint": format!("fnv64:{generation_ordinal:016x}"),
                "fileHashes": [{"path": "src/lib.rs", "sha256": "0000000000000000000000000000000000000000000000000000000000000000"}],
                "artifactIds": [format!("structural-index/{generation_id}.json")]
            })
        })
        .collect::<Vec<_>>();
    serde_json::from_value(json!({
        "schemaId": "agent.semantic-protocols.client-cache-manifest",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.client",
        "protocolVersion": "1",
        "cacheRoot": root.display().to_string(),
        "generations": generations
    }))
    .expect("manifest")
}

pub(super) fn raw_source_like_columns(db_path: &std::path::Path) -> Vec<String> {
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

pub(super) fn path_strings(paths: &[ClientDbStructuralPath]) -> Vec<&str> {
    paths.iter().map(ClientDbStructuralPath::as_str).collect()
}

pub(super) fn temp_root(name: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("agent-semantic-client-db-{name}-{unique}"));
    std::fs::create_dir_all(&root).expect("create temp root");
    root
}
