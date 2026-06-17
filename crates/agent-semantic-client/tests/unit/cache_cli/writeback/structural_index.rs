use agent_semantic_client_core::{
    ClientMethod, ClientRequest, LanguageId, ProviderRegistrySnapshot,
};
use agent_semantic_client_db::{
    ClientDb, ClientDbStructuralIndexLookup, ClientDbStructuralQueryKey,
};
use serde_json::{Value, json};

use super::{gerbil_scheme_provider, rust_provider, temp_root};
use crate::cache_cli::writeback::write_prompt_output_cache_after_provider_success;

#[test]
fn structural_index_packet_writeback_applies_refresh_rows() {
    let _guard = crate::test_support::CACHE_TEST_LOCK
        .lock()
        .expect("cache test lock");
    let root = temp_root("structural-index-writeback");
    std::fs::create_dir_all(root.join(".git")).expect("create git marker");
    let snapshot = ProviderRegistrySnapshot {
        activation_path: root.join("activation.json"),
        providers: vec![rust_provider()],
    };
    let request = ClientRequest::new(ClientMethod::Search, &root)
        .with_language(LanguageId::from("rust"))
        .with_forwarded_args(vec!["prime".to_string(), ".".to_string()]);
    let first_packet = structural_index_packet(&root, "rust-index-1", "0", true);
    let first_packet_bytes = serde_json::to_vec(&first_packet).expect("first packet");
    let second_packet = structural_index_packet(&root, "rust-index-2", "2", false);
    let second_packet_bytes = serde_json::to_vec(&second_packet).expect("second packet");

    let first_probe = write_prompt_output_cache_after_provider_success(
        &root,
        &snapshot,
        &request,
        &first_packet_bytes,
        &[],
    )
    .expect("first structural writeback");
    let second_probe = write_prompt_output_cache_after_provider_success(
        &root,
        &snapshot,
        &request,
        &second_packet_bytes,
        &[],
    )
    .expect("second structural writeback");
    let cache_report = agent_semantic_client_core::ClientCacheManifest::inspect_project(&root);
    let cache_root = cache_report.cache_root.expect("cache root");
    let db_path = ClientDb::default_path(&cache_root);
    let db = ClientDb::open_read_only_existing(&db_path)
        .expect("open db")
        .expect("db exists");
    let summary = db.summary().expect("summary");
    let copied_symbols = db
        .lookup_structural_symbols(&ClientDbStructuralIndexLookup {
            language_id: LanguageId::from("rust"),
            provider_id: "rs-harness".into(),
            project_root: root.clone(),
            query: ClientDbStructuralQueryKey::from("cached_helper"),
            limit: 8,
        })
        .expect("lookup copied symbol");

    assert_eq!(first_probe.sqlite_write_count, 3);
    assert_eq!(second_probe.sqlite_write_count, 3);
    assert_eq!(summary.structural_index_generation_count, 2);
    assert_eq!(summary.structural_index_symbol_count, 4);
    assert_eq!(copied_symbols.len(), 1);
    assert_eq!(copied_symbols[0].owner_path.as_str(), "src/unchanged.rs");
    assert!(
        cache_root
            .parent()
            .expect("cache root parent")
            .join("artifacts/structural-index/rust-index-2.json")
            .exists()
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn gerbil_scheme_structural_index_packet_writeback_is_queryable() {
    let _guard = crate::test_support::CACHE_TEST_LOCK
        .lock()
        .expect("cache test lock");
    let root = temp_root("gerbil-structural-index-writeback");
    std::fs::create_dir_all(root.join(".git")).expect("create git marker");
    let snapshot = ProviderRegistrySnapshot {
        activation_path: root.join("activation.json"),
        providers: vec![gerbil_scheme_provider()],
    };
    let request = ClientRequest::new(ClientMethod::Search, &root)
        .with_language(LanguageId::from("gerbil-scheme"))
        .with_forwarded_args(vec!["structural".to_string(), "--json".to_string()]);
    let packet = gerbil_structural_index_packet(&root);
    let packet_bytes = serde_json::to_vec(&packet).expect("gerbil structural packet");

    let probe = write_prompt_output_cache_after_provider_success(
        &root,
        &snapshot,
        &request,
        &packet_bytes,
        &[],
    )
    .expect("gerbil structural writeback");
    let cache_report = agent_semantic_client_core::ClientCacheManifest::inspect_project(&root);
    let cache_root = cache_report.cache_root.expect("cache root");
    let db_path = ClientDb::default_path(&cache_root);
    let db = ClientDb::open_read_only_existing(&db_path)
        .expect("open db")
        .expect("db exists");
    let symbols = db
        .lookup_structural_symbols(&ClientDbStructuralIndexLookup {
            language_id: LanguageId::from("gerbil-scheme"),
            provider_id: "gerbil-scheme-harness".into(),
            project_root: root.clone(),
            query: ClientDbStructuralQueryKey::from("search-main"),
            limit: 8,
        })
        .expect("lookup gerbil structural symbol");

    assert_eq!(probe.sqlite_write_count, 3);
    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0].owner_path.as_str(), "src/commands/search.ss");
    assert!(
        cache_root
            .parent()
            .expect("cache root parent")
            .join("artifacts/structural-index/gerbil-structural-1.json")
            .exists()
    );
    let _ = std::fs::remove_dir_all(root);
}

fn structural_index_packet(
    root: &std::path::Path,
    generation_id: &str,
    lib_hash_digit: &str,
    include_unchanged_rows: bool,
) -> Value {
    let mut owners = vec![json!({
        "ownerPath": "src/lib.rs",
        "ownerKind": "source",
        "sourceAuthority": "native-parser",
        "location": {"path": "src/lib.rs", "lineRange": "1:40"},
        "queryKeys": ["parse_config", "config"]
    })];
    let mut symbols = vec![json!({
        "ownerPath": "src/lib.rs",
        "name": "parse_config",
        "qualifiedName": "crate::parse_config",
        "kind": "function",
        "visibility": "public",
        "sourceLocator": "src/lib.rs:3:12",
        "queryKeys": ["parse", "config"]
    })];
    let mut dependency_usages = vec![json!({
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
    })];
    if include_unchanged_rows {
        owners.push(json!({
            "ownerPath": "src/unchanged.rs",
            "ownerKind": "source",
            "sourceAuthority": "native-parser",
            "location": {"path": "src/unchanged.rs", "lineRange": "1:20"},
            "queryKeys": ["cached_helper"]
        }));
        symbols.push(json!({
            "ownerPath": "src/unchanged.rs",
            "name": "cached_helper",
            "kind": "function",
            "visibility": "private",
            "sourceLocator": "src/unchanged.rs:4:4",
            "queryKeys": ["cached_helper"]
        }));
        dependency_usages.push(json!({
            "ownerPath": "src/unchanged.rs",
            "packageName": "anyhow",
            "packageVersion": "1.0.0",
            "apiName": "Result",
            "importPath": "anyhow::Result",
            "manifestPath": "Cargo.toml",
            "lockfileHash": "sha256:2222222222222222222222222222222222222222222222222222222222222222",
            "source": "manifest+native-parser",
            "sourceLocator": "src/unchanged.rs:2:5",
            "queryKeys": ["anyhow::Result"]
        }));
    }
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
                "sha256": lib_hash_digit.repeat(64),
                "source": "provider"
            },
            {
                "path": "src/unchanged.rs",
                "sha256": "1".repeat(64),
                "source": "provider"
            }
        ],
        "owners": owners,
        "symbols": symbols,
        "dependencyUsages": dependency_usages
    })
}

fn gerbil_structural_index_packet(root: &std::path::Path) -> Value {
    json!({
        "schemaId": "agent.semantic-protocols.semantic-structural-index",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "generationId": "gerbil-structural-1",
        "languageId": "gerbil-scheme",
        "providerId": "gerbil-scheme-harness",
        "providerVersion": "0.1.0",
        "exportMethod": "index/structural",
        "projectRoot": root.display().to_string(),
        "packageRoot": ".",
        "sourceAuthority": "native-parser",
        "sourceArtifactId": "structural-index/gerbil-structural-1.json",
        "rawSourceStored": false,
        "fileHashes": [
            {
                "path": "src/commands/search.ss",
                "sha256": "3".repeat(64),
                "byteLen": 0,
                "mtimeMs": 0,
                "source": "native-parser-fingerprint"
            }
        ],
        "owners": [
            {
                "ownerPath": "src/commands/search.ss",
                "ownerKind": "source",
                "sourceAuthority": "native-parser",
                "location": {
                    "path": "src/commands/search.ss",
                    "lineRange": "1:426"
                },
                "queryKeys": [
                    "src/commands/search.ss",
                    ":parser/facade",
                    "search-main"
                ]
            }
        ],
        "symbols": [
            {
                "ownerPath": "src/commands/search.ss",
                "name": "search-main",
                "qualifiedName": "gerbil-scheme-language-project-harness/src/commands/search::search-main",
                "kind": "def",
                "visibility": "public",
                "sourceLocator": "src/commands/search.ss:20:40",
                "queryKeys": [
                    "search-main",
                    "gerbil-scheme-language-project-harness/src/commands/search::search-main",
                    "def",
                    "src/commands/search.ss"
                ]
            }
        ],
        "dependencyUsages": [
            {
                "ownerPath": "src/commands/search.ss",
                "packageName": ":parser/facade",
                "apiName": ":parser/facade",
                "importPath": ":parser/facade",
                "manifestPath": "gerbil.pkg",
                "source": "native-parser-import",
                "sourceLocator": "src/commands/search.ss:1:1",
                "queryKeys": [
                    ":parser/facade",
                    "src/commands/search.ss",
                    "native-parser-import"
                ]
            }
        ]
    })
}
