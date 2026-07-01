use std::sync::mpsc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use agent_semantic_client_core::{
    CacheExportMethod, CacheGenerationId, ClientCacheFileHash, ClientCacheManifest, LanguageId,
    ProviderId, SemanticSchemaId, SemanticSchemaVersion,
};
use agent_semantic_client_db::{
    CLIENT_DB_SOURCE_INDEX_PROVIDER_ID, CLIENT_DB_SOURCE_INDEX_SCHEMA_ID,
    CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION, CLIENT_DB_SOURCE_INDEX_SCOPE_DIR_EVIDENCE_PREFIX,
    CLIENT_DB_SOURCE_INDEX_SCOPE_REGISTRY_EVIDENCE_PATH,
    CLIENT_DB_SOURCE_INDEX_SCOPE_WITNESS_SHA256, ClientDb, ClientDbGenerationLookup,
    ClientDbSourceIndexCandidateLookup, ClientDbSourceIndexImportAssemblyRequest,
    ClientDbSourceIndexImportFile, ClientDbSourceIndexImportRequest,
    ClientDbSourceIndexLookupState, ClientDbSourceIndexOwner, ClientDbSourceIndexPath,
    ClientDbSourceIndexQueryKey, ClientDbSourceIndexRefreshRequest,
    ClientDbSourceIndexRefreshResult, ClientDbSourceIndexScopeFile, ClientDbSourceIndexSource,
    ClientDbSourceIndexSourceKind, ClientDbStatus, ClientDbStructuralDependencyUsage,
    ClientDbStructuralIndexImport, ClientDbStructuralKind, ClientDbStructuralLocator,
    ClientDbStructuralName, ClientDbStructuralOwner, ClientDbStructuralPath,
    ClientDbStructuralQueryKey, ClientDbStructuralSource, ClientDbStructuralSymbol,
    assemble_source_index_import, build_source_index_import, client_db_source_index_file_count,
    client_db_source_index_generation_id, client_db_source_index_registry_evidence_hash,
    client_db_source_index_scope_dir_evidence_hash, source_index_evidence_graph,
    source_index_relative_path, source_index_scope_dirs, structural_index_evidence_graph,
};
use serde_json::json;

#[test]
fn schema_version_stays_on_first_release_contract() {
    assert_eq!(
        agent_semantic_client_db::AGENT_SEMANTIC_CLIENT_DB_SCHEMA_VERSION,
        1
    );
}

#[test]
fn inspect_reports_missing_without_creating_db() {
    let root = temp_root("missing");
    let db_path = root.join("client.sqlite3");

    let report = ClientDb::inspect(&db_path);

    assert_eq!(report.status, ClientDbStatus::Missing);
    assert!(
        ClientDb::open_read_only_existing(&db_path)
            .expect("open missing read-only db")
            .is_none()
    );
    assert_eq!(report.generation_count, 0);
    assert_eq!(report.syntax_row_generation_count, 0);
    assert_eq!(report.syntax_row_match_count, 0);
    assert_eq!(report.syntax_row_capture_count, 0);
    assert!(!db_path.exists());
    assert!(report.runtime_pragmas.is_none());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn source_index_import_builder_owns_owner_selector_projection() {
    assert_eq!(
        CLIENT_DB_SOURCE_INDEX_SCHEMA_ID,
        "agent.semantic-protocols.semantic-source-index"
    );
    assert_eq!(CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION, "1");
    assert_eq!(CLIENT_DB_SOURCE_INDEX_PROVIDER_ID, "db-engine-source-index");
    assert_eq!(
        CLIENT_DB_SOURCE_INDEX_SCOPE_DIR_EVIDENCE_PREFIX,
        "@scope/dir/"
    );
    assert_eq!(
        CLIENT_DB_SOURCE_INDEX_SCOPE_REGISTRY_EVIDENCE_PATH,
        "@scope/registry"
    );
    assert_eq!(
        CLIENT_DB_SOURCE_INDEX_SCOPE_WITNESS_SHA256,
        "0000000000000000000000000000000000000000000000000000000000000000"
    );
    assert!(
        client_db_source_index_generation_id()
            .as_str()
            .starts_with("source-index-")
    );
    assert_eq!(client_db_source_index_file_count(7), 7);
    assert_eq!(client_db_source_index_file_count(usize::MAX), u32::MAX);
    let registry_evidence = client_db_source_index_registry_evidence_hash("provider-registry");
    assert_eq!(
        registry_evidence.path,
        CLIENT_DB_SOURCE_INDEX_SCOPE_REGISTRY_EVIDENCE_PATH
    );
    assert_eq!(registry_evidence.byte_len, "provider-registry".len() as u64);
    assert_eq!(registry_evidence.mtime_ms, 0);
    assert_eq!(registry_evidence.sha256.len(), 64);
    let scope_dir_evidence = client_db_source_index_scope_dir_evidence_hash("src", 3, 4);
    assert_eq!(scope_dir_evidence.path, "@scope/dir/src");
    assert_eq!(
        scope_dir_evidence.sha256,
        CLIENT_DB_SOURCE_INDEX_SCOPE_WITNESS_SHA256
    );
    assert_eq!(scope_dir_evidence.byte_len, 3);
    assert_eq!(scope_dir_evidence.mtime_ms, 4);

    let root = temp_root("source-index-import-builder");

    let import = build_source_index_import(ClientDbSourceIndexImportRequest {
        generation_id: CacheGenerationId::from("source-index-test"),
        project_root: root.clone(),
        schema_id: SemanticSchemaId::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_ID),
        schema_version: SemanticSchemaVersion::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION),
        selector_source: ClientDbSourceIndexSource::from(CLIENT_DB_SOURCE_INDEX_PROVIDER_ID),
        file_hashes: vec![ClientCacheFileHash {
            path: "src/lib.rs".to_string(),
            sha256: "0123456789abcdef".repeat(4),
            byte_len: 27,
            mtime_ms: 42,
        }],
        files: vec![ClientDbSourceIndexImportFile {
            relative_path: "src/lib.rs".to_string(),
            language_id: LanguageId::from("rust"),
            provider_id: ProviderId::from("rs-harness"),
            text: "pub fn source_index_fixture() {}\n".to_string(),
        }],
    })
    .expect("build source index import");

    assert_eq!(import.owners.len(), 1);
    assert_eq!(import.selectors.len(), 1);
    let owner = &import.owners[0];
    assert_eq!(owner.owner_path.as_str(), "src/lib.rs");
    assert_eq!(owner.line_count, Some(1));
    assert!(
        owner
            .query_keys
            .iter()
            .any(|key| key.as_str() == "source_index_fixture")
    );
    let selector = &import.selectors[0];
    assert_eq!(selector.selector_id, "rust://src/lib.rs#file");
    assert_eq!(selector.symbol.as_deref(), Some("lib"));
    assert_eq!(selector.end_line, 1);
    assert_eq!(selector.source.as_str(), "db-engine-source-index");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn source_index_import_assembly_owns_file_hashes_and_scope_evidence() {
    let root = temp_root("source-index-import-assembly");
    let src = root.join("src");
    std::fs::create_dir_all(&src).expect("create source dir");
    std::fs::write(src.join("lib.rs"), "pub fn db_owned_import_assembly() {}\n")
        .expect("write source file");

    let import = assemble_source_index_import(ClientDbSourceIndexImportAssemblyRequest {
        generation_id: CacheGenerationId::from("source-index-assembly"),
        project_root: root.clone(),
        schema_id: SemanticSchemaId::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_ID),
        schema_version: SemanticSchemaVersion::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION),
        selector_source: ClientDbSourceIndexSource::from(CLIENT_DB_SOURCE_INDEX_PROVIDER_ID),
        file_text_bytes_limit: 4096,
        previous_file_hashes: None,
        registry_fingerprint: "provider-registry".to_string(),
        extra_scope_dirs: Vec::new(),
        files: vec![ClientDbSourceIndexScopeFile {
            path: src.join("lib.rs"),
            language_id: LanguageId::from("rust"),
            provider_id: ProviderId::from("rs-harness"),
        }],
    })
    .expect("assemble source index import");

    assert_eq!(import.file_hashes.len(), 4);
    assert!(
        import
            .file_hashes
            .iter()
            .any(|hash| hash.path == "src/lib.rs")
    );
    assert!(
        import
            .file_hashes
            .iter()
            .any(|hash| { hash.path == CLIENT_DB_SOURCE_INDEX_SCOPE_REGISTRY_EVIDENCE_PATH })
    );
    assert!(
        import
            .file_hashes
            .iter()
            .any(|hash| hash.path == "@scope/dir/src")
    );
    assert!(
        import
            .file_hashes
            .iter()
            .any(|hash| hash.path == "@scope/dir/.")
    );
    assert_eq!(import.owners.len(), 1);
    assert!(
        import.owners[0]
            .query_keys
            .iter()
            .any(|key| key.as_str() == "db_owned_import_assembly")
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn source_index_refresh_import_reports_reused_generation() {
    let root = temp_root("source-index-refresh-import");
    let db_path = root.join("client.sqlite3");
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");

    let import = source_index_import_fixture(&root, "source-index-test");
    let first = db
        .refresh_source_index_import(ClientDbSourceIndexRefreshRequest {
            import: import.clone(),
            file_count: 1,
        })
        .expect("first source index refresh");
    let second = db
        .refresh_source_index_import(ClientDbSourceIndexRefreshRequest {
            import: source_index_import_fixture(&root, "source-index-next"),
            file_count: 1,
        })
        .expect("second source index refresh");

    assert_eq!(first.generation_id.as_str(), "source-index-test");
    assert!(!first.reused_generation);
    assert_eq!(first.owner_count, 1);
    assert_eq!(first.selector_count, 1);
    assert_eq!(second.generation_id.as_str(), "source-index-test");
    assert!(second.reused_generation);
    let projected = ClientDbSourceIndexRefreshResult::from_report(db_path.clone(), second.clone());
    assert_eq!(projected.db_path, db_path);
    assert_eq!(projected.generation_id, second.generation_id);
    assert!(projected.reused_generation);
    assert_eq!(projected.file_count, 1);
    assert_eq!(projected.owner_count, 1);
    assert_eq!(projected.selector_count, 1);
    let stats = db
        .source_index_stats(&first.generation_id)
        .expect("source index stats");
    let reused_projection = ClientDbSourceIndexRefreshResult::from_stats(
        root.join("client.sqlite3"),
        stats,
        usize::MAX,
        true,
    );
    assert_eq!(reused_projection.file_count, u32::MAX);
    assert!(reused_projection.reused_generation);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn source_index_scope_files_are_reconstructed_by_db_facade() {
    let root = temp_root("source-index-scope-files");
    let db_path = root.join("client.sqlite3");
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");
    let import = source_index_import_fixture(&root, "source-index-test");
    let schema_id = import.schema_id.clone();
    let schema_version = import.schema_version.clone();
    db.refresh_source_index_import(ClientDbSourceIndexRefreshRequest {
        import,
        file_count: 1,
    })
    .expect("refresh source index");

    let files = db
        .latest_source_index_scope_files(&root, &schema_id, &schema_version)
        .expect("scope files lookup")
        .expect("scope files");

    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path, root.join("src/lib.rs"));
    assert_eq!(files[0].language_id, LanguageId::from("rust"));
    assert_eq!(files[0].provider_id, ProviderId::from("rs-harness"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn source_index_path_rules_are_db_owned() {
    let root = temp_root("source-index-path-rules");
    let nested = root.join("src").join("nested");
    std::fs::create_dir_all(&nested).expect("create nested dir");
    let source_file = nested.join("lib.rs");
    std::fs::write(&source_file, "pub fn path_rules() {}\n").expect("write source");
    let external = root
        .parent()
        .expect("temp root parent")
        .join("outside")
        .join("mod.rs");

    assert_eq!(
        source_index_relative_path(&root, &source_file),
        "src/nested/lib.rs"
    );
    assert!(source_index_relative_path(&root, &external).ends_with("outside/mod.rs"));

    let dirs = source_index_scope_dirs(
        &root,
        &[ClientDbSourceIndexScopeFile {
            path: source_file,
            language_id: LanguageId::from("rust"),
            provider_id: ProviderId::from("rs-harness"),
        }],
    );
    assert!(dirs.contains("."));
    assert!(dirs.contains("src"));
    assert!(dirs.contains("src/nested"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn source_index_lookup_dto_contract_is_db_owned() {
    assert_eq!(
        ClientDbSourceIndexLookupState::MissingDb.as_str(),
        "missing-db"
    );
    assert_eq!(
        ClientDbSourceIndexLookupState::EmptyIndex.as_str(),
        "empty-index"
    );
    assert_eq!(ClientDbSourceIndexLookupState::Hit.as_str(), "hit");
    assert_eq!(ClientDbSourceIndexLookupState::Miss.as_str(), "miss");

    assert_eq!(
        ClientDbSourceIndexSourceKind::from(ClientDbSourceIndexSource::from("file")).as_str(),
        "file"
    );
    assert_eq!(
        ClientDbSourceIndexSourceKind::from(ClientDbSourceIndexSource::from("runtime")).as_str(),
        "runtime"
    );

    let candidate =
        agent_semantic_client_db::ClientDbSourceIndexCandidate::from(ClientDbSourceIndexOwner {
            owner_path: ClientDbSourceIndexPath::from("src/lib.rs"),
            language_id: Some(LanguageId::from("rust")),
            provider_id: Some(ProviderId::from("rs-harness")),
            source_kind: ClientDbSourceIndexSource::from("file"),
            line_count: Some(7),
            query_keys: vec![
                ClientDbSourceIndexQueryKey::from("source_index_fixture"),
                ClientDbSourceIndexQueryKey::from("lib"),
            ],
        });
    assert_eq!(candidate.path, "src/lib.rs");
    assert_eq!(candidate.language_id, Some(LanguageId::from("rust")));
    assert_eq!(candidate.provider_id, Some(ProviderId::from("rs-harness")));
    assert_eq!(candidate.source_kind.as_str(), "file");
    assert_eq!(candidate.line_count, Some(7));
    assert_eq!(
        candidate.query_keys,
        vec!["source_index_fixture".to_string(), "lib".to_string()]
    );
}

#[test]
fn source_index_candidate_lookup_is_db_owned_and_deduplicated() {
    let root = temp_root("source-index-candidate-lookup");
    let db_path = root.join("client.sqlite3");
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");
    let import = source_index_import_fixture(&root, "source-index-test");
    db.refresh_source_index_import(ClientDbSourceIndexRefreshRequest {
        import,
        file_count: 1,
    })
    .expect("refresh source index");

    let hit = db
        .lookup_source_index_candidates(&ClientDbSourceIndexCandidateLookup {
            project_root: root.clone(),
            language_id: Some(LanguageId::from("rust")),
            query_keys: vec![
                ClientDbSourceIndexQueryKey::from("source_index_fixture"),
                ClientDbSourceIndexQueryKey::from("lib"),
            ],
            limit: 8,
        })
        .expect("lookup source index candidates");

    assert_eq!(hit.state, ClientDbSourceIndexLookupState::Hit);
    assert_eq!(hit.candidates.len(), 1);
    assert_eq!(hit.candidates[0].path, "src/lib.rs");

    let miss = db
        .lookup_source_index_candidates(&ClientDbSourceIndexCandidateLookup {
            project_root: root.clone(),
            language_id: Some(LanguageId::from("rust")),
            query_keys: vec![ClientDbSourceIndexQueryKey::from("not_present")],
            limit: 8,
        })
        .expect("lookup source index miss");
    assert_eq!(miss.state, ClientDbSourceIndexLookupState::Miss);
    assert!(miss.candidates.is_empty());

    let empty_db_path = root.join("empty.sqlite3");
    let empty_db = ClientDb::open_or_create(&empty_db_path).expect("open empty db");
    let empty = empty_db
        .lookup_source_index_candidates(&ClientDbSourceIndexCandidateLookup {
            project_root: root.clone(),
            language_id: Some(LanguageId::from("rust")),
            query_keys: vec![ClientDbSourceIndexQueryKey::from("source_index_fixture")],
            limit: 8,
        })
        .expect("lookup empty source index");
    assert_eq!(empty.state, ClientDbSourceIndexLookupState::EmptyIndex);
    assert!(empty.candidates.is_empty());

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn evidence_graph_projects_source_index_without_line_identity() {
    let root = temp_root("evidence-graph-source");
    let import = source_index_import_fixture(&root, "source-index-test");

    let graph = source_index_evidence_graph(&import);

    assert_eq!(
        graph.schema_id,
        agent_semantic_client_db::CLIENT_DB_EVIDENCE_GRAPH_SCHEMA_ID
    );
    assert_eq!(
        graph.schema_version,
        agent_semantic_client_db::CLIENT_DB_EVIDENCE_GRAPH_SCHEMA_VERSION
    );
    assert_eq!(graph.generation_id, "source-index-test");
    assert_eq!(graph.project_root, root);
    assert!(graph.nodes.iter().any(|node| {
        node.kind == "source-owner" && node.id == "source-owner:source-index-test:src/lib.rs"
    }));
    let selector = graph
        .nodes
        .iter()
        .find(|node| node.kind == "selector")
        .expect("selector node");
    assert_eq!(selector.selector.as_deref(), Some("rust://src/lib.rs#file"));
    assert_eq!(selector.id, "selector:rust://src/lib.rs#file");
    assert!(!selector.id.contains(":1:"));
    let json = serde_json::to_value(&graph).expect("serialize evidence graph");
    assert_eq!(
        json["schemaId"],
        "agent.semantic-protocols.evidence-graph-read-model"
    );
    assert_eq!(json["schemaVersion"], "1");
    let schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../schemas/semantic-evidence-graph-read-model.v1.schema.json"
    ))
    .expect("parse evidence graph schema");
    assert_eq!(
        schema["properties"]["schemaId"]["const"],
        "agent.semantic-protocols.evidence-graph-read-model"
    );
    assert_eq!(
        schema["$id"],
        "https://agent-semantic-protocols.local/schemas/semantic-evidence-graph-read-model.v1.schema.json"
    );
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == "contains-selector"
            && edge.from == "source-owner:source-index-test:src/lib.rs"
            && edge.to == "selector:rust://src/lib.rs#file"
    }));
}

#[test]
fn evidence_graph_projects_structural_index_symbols_and_dependencies() {
    let root = temp_root("evidence-graph-structural");
    let import = ClientDbStructuralIndexImport {
        generation_id: CacheGenerationId::from("structural-index-test"),
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("rs-harness"),
        provider_version: None,
        export_method: None,
        project_root: root.clone(),
        package_root: None,
        schema_id: SemanticSchemaId::from("agent.semantic-protocols.semantic-structural-index"),
        schema_version: SemanticSchemaVersion::from("1"),
        source_artifact_id: None,
        file_hashes: vec![ClientCacheFileHash {
            path: "src/lib.rs".to_string(),
            sha256: "0123456789abcdef".repeat(4),
            byte_len: 27,
            mtime_ms: 42,
        }],
        owners: vec![ClientDbStructuralOwner {
            owner_path: ClientDbStructuralPath::from("src/lib.rs"),
            owner_kind: ClientDbStructuralKind::from("file"),
            source_authority: ClientDbStructuralSource::from("provider"),
            start_line: Some(1),
            end_line: Some(9),
            query_keys: vec![ClientDbStructuralQueryKey::from("lib")],
        }],
        symbols: vec![ClientDbStructuralSymbol {
            owner_path: ClientDbStructuralPath::from("src/lib.rs"),
            name: ClientDbStructuralName::from("EvidenceFixture"),
            kind: ClientDbStructuralKind::from("struct"),
            visibility: None,
            source_locator: Some(ClientDbStructuralLocator::from(
                "rust://src/lib.rs#item/struct/EvidenceFixture",
            )),
            query_keys: vec![ClientDbStructuralQueryKey::from("EvidenceFixture")],
        }],
        dependency_usages: vec![ClientDbStructuralDependencyUsage {
            owner_path: ClientDbStructuralPath::from("src/lib.rs"),
            package_name: ClientDbStructuralName::from("serde"),
            package_version: None,
            api_name: Some(ClientDbStructuralName::from("Serialize")),
            import_path: None,
            manifest_path: None,
            lockfile_hash: None,
            source: ClientDbStructuralSource::from("provider"),
            source_locator: Some(ClientDbStructuralLocator::from(
                "rust://src/lib.rs#use/serde/Serialize",
            )),
            query_keys: vec![ClientDbStructuralQueryKey::from("serde")],
        }],
    };

    let graph = structural_index_evidence_graph(&import);

    assert_eq!(graph.generation_id, "structural-index-test");
    assert!(graph.nodes.iter().any(|node| {
        node.kind == "structural-owner"
            && node.id == "structural-owner:structural-index-test:src/lib.rs"
    }));
    assert!(graph.nodes.iter().any(|node| {
        node.kind == "symbol"
            && node.selector.as_deref() == Some("rust://src/lib.rs#item/struct/EvidenceFixture")
            && !node.id.contains(":1:")
    }));
    assert!(graph.nodes.iter().any(|node| {
        node.kind == "dependency-usage"
            && node.label == "serde::Serialize"
            && node.selector.as_deref() == Some("rust://src/lib.rs#use/serde/Serialize")
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == "defines-symbol"
            && edge.from == "structural-owner:structural-index-test:src/lib.rs"
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == "uses-dependency"
            && edge.from == "structural-owner:structural-index-test:src/lib.rs"
    }));
}

fn source_index_import_fixture(
    root: &std::path::Path,
    generation_id: &str,
) -> agent_semantic_client_db::ClientDbSourceIndexImport {
    build_source_index_import(ClientDbSourceIndexImportRequest {
        generation_id: CacheGenerationId::from(generation_id),
        project_root: root.to_path_buf(),
        schema_id: SemanticSchemaId::from("agent.semantic-protocols.semantic-source-index"),
        schema_version: SemanticSchemaVersion::from("1"),
        selector_source: ClientDbSourceIndexSource::from("db-engine-source-index"),
        file_hashes: vec![ClientCacheFileHash {
            path: "src/lib.rs".to_string(),
            sha256: "0123456789abcdef".repeat(4),
            byte_len: 27,
            mtime_ms: 42,
        }],
        files: vec![ClientDbSourceIndexImportFile {
            relative_path: "src/lib.rs".to_string(),
            language_id: LanguageId::from("rust"),
            provider_id: ProviderId::from("rs-harness"),
            text: "pub fn source_index_fixture() {}\n".to_string(),
        }],
    })
    .expect("source index import fixture")
}

#[test]
fn open_creates_schema_and_imports_manifest_generations() {
    let root = temp_root("import");
    let db_path = root.join("client.sqlite3");
    let manifest = manifest(&root, false);
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");

    db.import_manifest(&manifest).expect("import manifest");
    let summary = db.summary().expect("db summary");
    let generation_hit = ClientDb::lookup_generation(&ClientDbGenerationLookup {
        db_path: db_path.clone(),
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("rs-harness"),
        project_root: root.clone(),
        export_method: CacheExportMethod::from("search/prime"),
        request_fingerprint: Some("fnv64:0123456789abcdef".to_string()),
    })
    .expect("lookup generation")
    .expect("generation hit");
    let report = ClientDb::inspect(&db_path);
    let read_db = ClientDb::open_read_only_existing(&db_path)
        .expect("open read-only db")
        .expect("db exists");
    let open_report = read_db.inspect_open().expect("inspect open db");
    let stored_schema_version: String = rusqlite::Connection::open(&db_path)
        .expect("open sqlite")
        .query_row(
            "SELECT value FROM schema_meta WHERE key = 'schemaVersion'",
            [],
            |row| row.get(0),
        )
        .expect("schema version");
    let journal_mode: String = rusqlite::Connection::open(&db_path)
        .expect("open sqlite")
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .expect("journal mode");

    assert_eq!(summary.generation_count, 1);
    assert_eq!(summary.syntax_row_generation_count, 0);
    assert_eq!(summary.syntax_row_match_count, 0);
    assert_eq!(summary.syntax_row_capture_count, 0);
    assert!(!summary.raw_source_stored);
    assert_eq!(report.status, ClientDbStatus::Present);
    assert_eq!(report.generation_count, 1);
    assert_eq!(report.syntax_row_generation_count, 0);
    assert_eq!(report.syntax_row_match_count, 0);
    assert_eq!(report.syntax_row_capture_count, 0);
    assert!(!report.raw_source_stored);
    assert_eq!(open_report, report);
    let runtime_pragmas = report.runtime_pragmas.expect("runtime pragmas");
    assert_eq!(runtime_pragmas.journal_mode.as_str(), "wal");
    assert_eq!(runtime_pragmas.busy_timeout_ms, 5000);
    assert!(runtime_pragmas.foreign_keys);
    assert_eq!(
        stored_schema_version,
        agent_semantic_client_db::AGENT_SEMANTIC_CLIENT_DB_SCHEMA_VERSION.to_string()
    );
    assert_eq!(generation_hit.file_hashes.len(), 1);
    assert_eq!(generation_hit.file_hashes[0].path, "src/lib.rs");
    assert_eq!(generation_hit.file_hashes[0].sha256, "0".repeat(64));
    assert_eq!(journal_mode.to_ascii_lowercase(), "wal");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn open_waits_for_transient_writer_lock() {
    let root = temp_root("writer-lock");
    let db_path = root.join("client.sqlite3");
    let db = ClientDb::open_or_create(&db_path).expect("create db");
    drop(db);

    let (ready_tx, ready_rx) = mpsc::channel();
    let locked_db_path = db_path.clone();
    let lock_handle = thread::spawn(move || {
        let conn = rusqlite::Connection::open(&locked_db_path).expect("open lock connection");
        conn.execute_batch("BEGIN IMMEDIATE")
            .expect("begin write lock");
        ready_tx.send(()).expect("send lock ready");
        thread::sleep(Duration::from_millis(200));
        conn.execute_batch("COMMIT").expect("release write lock");
    });

    ready_rx.recv().expect("wait for write lock");
    let reopened = ClientDb::open_or_create(&db_path).expect("open waits for writer lock");
    drop(reopened);
    lock_handle.join().expect("lock thread");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn has_generation_matches_language_provider_project_and_export_method() {
    let root = temp_root("has-generation");
    let db_path = root.join("client.sqlite3");
    let manifest = manifest(&root, false);
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");

    db.import_manifest(&manifest).expect("import manifest");

    assert!(
        ClientDb::has_generation(&ClientDbGenerationLookup {
            db_path: db_path.clone(),
            language_id: LanguageId::from("rust"),
            provider_id: ProviderId::from("rs-harness"),
            project_root: root.clone(),
            export_method: CacheExportMethod::from("search/prime"),
            request_fingerprint: None,
        })
        .expect("has generation")
    );
    let hit = ClientDb::lookup_generation(&ClientDbGenerationLookup {
        db_path: db_path.clone(),
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("rs-harness"),
        project_root: root.clone(),
        export_method: CacheExportMethod::from("search/prime"),
        request_fingerprint: None,
    })
    .expect("lookup generation")
    .expect("generation hit");
    assert_eq!(
        hit.request_fingerprint.as_deref(),
        Some("fnv64:0123456789abcdef")
    );
    assert_eq!(hit.schema_ids.len(), 1);
    assert_eq!(
        hit.schema_ids[0].as_str(),
        "agent.semantic-protocols.semantic-search-packet"
    );
    assert_eq!(hit.artifact_ids.len(), 1);
    assert_eq!(hit.artifact_ids[0].as_str(), "search/rust-main-1.json");
    assert!(
        !ClientDb::has_generation(&ClientDbGenerationLookup {
            db_path: db_path.clone(),
            language_id: LanguageId::from("rust"),
            provider_id: ProviderId::from("rs-harness"),
            project_root: root.clone(),
            export_method: CacheExportMethod::from("search/owner"),
            request_fingerprint: None,
        })
        .expect("missing generation")
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn invalidate_generations_clears_rows_without_deleting_db() {
    let root = temp_root("invalidate");
    let db_path = root.join("client.sqlite3");
    let manifest = manifest(&root, false);
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");

    db.import_manifest(&manifest).expect("import manifest");
    let invalidated = ClientDb::invalidate_generations(&db_path).expect("invalidate generations");
    let report = ClientDb::inspect(&db_path);
    let has_generation = ClientDb::has_generation(&ClientDbGenerationLookup {
        db_path: db_path.clone(),
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("rs-harness"),
        project_root: root.clone(),
        export_method: CacheExportMethod::from("search/prime"),
        request_fingerprint: None,
    })
    .expect("generation lookup after invalidate");

    assert_eq!(invalidated, 1);
    assert_eq!(report.status, ClientDbStatus::Present);
    assert_eq!(report.generation_count, 0);
    assert!(!has_generation);
    assert!(db_path.exists());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn invalidate_generations_is_noop_when_db_is_missing() {
    let root = temp_root("invalidate-missing");
    let db_path = root.join("client.sqlite3");

    let invalidated = ClientDb::invalidate_generations(&db_path).expect("invalidate missing db");

    assert_eq!(invalidated, 0);
    assert!(!db_path.exists());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn import_rejects_raw_source_generations() {
    let root = temp_root("raw-source");
    let db_path = root.join("client.sqlite3");
    let manifest = manifest(&root, true);
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");

    let error = db
        .import_manifest(&manifest)
        .expect_err("reject raw source");

    assert!(error.contains("rawSourceStored=true"), "{error}");
    let _ = std::fs::remove_dir_all(root);
}

fn manifest(root: &std::path::Path, raw_source_stored: bool) -> ClientCacheManifest {
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
                "exportMethod": "search/prime",
                "projectRoot": root.display().to_string(),
                "packageRoot": ".",
                "schemaIds": ["agent.semantic-protocols.semantic-search-packet"],
                "cacheStatus": "miss",
                "rawSourceStored": raw_source_stored,
                "requestFingerprint": "fnv64:0123456789abcdef",
                "fileHashes": [{
                    "path": "src/lib.rs",
                    "sha256": "0000000000000000000000000000000000000000000000000000000000000000",
                    "byteLen": 1,
                    "mtimeMs": 1
                }],
                "artifactIds": ["search/rust-main-1.json"]
            }
        ]
    }))
    .expect("manifest")
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
