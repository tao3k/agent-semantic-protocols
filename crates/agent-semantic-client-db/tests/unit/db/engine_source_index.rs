use std::{
    fs,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use agent_semantic_client_core::{
    CacheGenerationId, ClientCacheFileHash, LanguageId, ProviderId, SemanticSchemaId,
    SemanticSchemaVersion,
};
use agent_semantic_client_db::{
    CLIENT_DB_SOURCE_INDEX_PROVIDER_ID, CLIENT_DB_SOURCE_INDEX_SCHEMA_ID,
    CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION, ClientDbEngine,
    ClientDbSourceIndexClientDirLookupRequest, ClientDbSourceIndexImportAssemblyRequest,
    ClientDbSourceIndexImportFile, ClientDbSourceIndexImportRequest,
    ClientDbSourceIndexLookupState, ClientDbSourceIndexPath, ClientDbSourceIndexQueryKey,
    ClientDbSourceIndexRefreshRequest, ClientDbSourceIndexScopeFile, ClientDbSourceIndexSelector,
    ClientDbSourceIndexSelectorPayloadProof, ClientDbSourceIndexSource, build_source_index_import,
    source_index_import_with_file_hashes,
};

#[tokio::test(flavor = "current_thread")]
async fn db_engine_source_index_import_uses_active_turso_path_without_retired_db_control() {
    let client_dir = temp_root("db-engine-source-index-client");
    let project_root = temp_root("db-engine-source-index-project");
    let first_import = build_source_index_import(ClientDbSourceIndexImportRequest {
        generation_id: CacheGenerationId::from("source-index-active-turso-1"),
        project_root: project_root.clone(),
        schema_id: SemanticSchemaId::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_ID),
        schema_version: SemanticSchemaVersion::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION),
        selector_source: ClientDbSourceIndexSource::from(CLIENT_DB_SOURCE_INDEX_PROVIDER_ID),
        file_hashes: vec![ClientCacheFileHash {
            path: "src/source_index_active_turso.rs".to_string(),
            sha256: "1234567890abcdef".repeat(4),
            byte_len: 49,
            mtime_ms: 11,
        }],
        files: vec![ClientDbSourceIndexImportFile {
            relative_path: "src/source_index_active_turso.rs".to_string(),
            language_id: LanguageId::from("rust"),
            provider_id: ProviderId::from("rs-harness"),
            text: "pub fn source_index_active_turso_fixture() {}\n".to_string(),
            selectors: Vec::new(),
        }],
    })
    .expect("build first Turso source-index import");
    let first = ClientDbEngine::refresh_source_index_import_from_client_dir(
        &client_dir,
        ClientDbSourceIndexRefreshRequest {
            import: first_import,
            file_count: 1,
        },
    )
    .expect("refresh source-index through active Turso DB Engine path");
    assert_eq!(first.generation_id.as_str(), "source-index-active-turso-1");
    assert!(!first.reused_generation);
    assert_eq!(first.file_count, 1);
    assert_eq!(first.owner_count, 1);
    assert_eq!(first.selector_count, 1);
    let inspect = ClientDbEngine::inspect_client_dir(&client_dir);
    assert_eq!(inspect.source_index_generation_count, 1);
    assert_eq!(inspect.source_index_owner_count, 1);
    assert_eq!(inspect.source_index_selector_count, 1);

    let second_import = build_source_index_import(ClientDbSourceIndexImportRequest {
        generation_id: CacheGenerationId::from("source-index-active-turso-2"),
        project_root: project_root.clone(),
        schema_id: SemanticSchemaId::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_ID),
        schema_version: SemanticSchemaVersion::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION),
        selector_source: ClientDbSourceIndexSource::from(CLIENT_DB_SOURCE_INDEX_PROVIDER_ID),
        file_hashes: vec![ClientCacheFileHash {
            path: "src/source_index_active_turso.rs".to_string(),
            sha256: "1234567890abcdef".repeat(4),
            byte_len: 49,
            mtime_ms: 11,
        }],
        files: vec![ClientDbSourceIndexImportFile {
            relative_path: "src/source_index_active_turso.rs".to_string(),
            language_id: LanguageId::from("rust"),
            provider_id: ProviderId::from("rs-harness"),
            text: "pub fn source_index_active_turso_fixture() {}\n".to_string(),
            selectors: Vec::new(),
        }],
    })
    .expect("build second Turso source-index import");
    let second = ClientDbEngine::refresh_source_index_import_from_client_dir(
        &client_dir,
        ClientDbSourceIndexRefreshRequest {
            import: second_import,
            file_count: 1,
        },
    )
    .expect("reuse source-index generation through active Turso DB Engine path");
    assert_eq!(second.generation_id.as_str(), "source-index-active-turso-1");
    assert!(second.reused_generation);
    assert_eq!(second.file_count, 1);
    assert_eq!(second.owner_count, 1);
    assert_eq!(second.selector_count, 1);

    let rust_language_id = LanguageId::from("rust");
    let lookup = ClientDbEngine::lookup_source_index_read_model_from_client_dir(
        &client_dir,
        "source_index_active_turso_fixture",
        Some(&rust_language_id),
        8,
    )
    .await
    .expect("lookup active Turso source-index read model");
    assert_eq!(lookup.state, ClientDbSourceIndexLookupState::Hit);
    assert!(
        lookup.candidates.iter().any(|candidate| candidate.path
            == "src/source_index_active_turso.rs"
            && candidate.language_id.as_ref().map(|id| id.as_str()) == Some("rust")
            && candidate.provider_id.as_ref().map(|id| id.as_str()) == Some("rs-harness")
            && candidate.source_kind.as_str() == "turso-source-index"),
        "lookup={lookup:?}"
    );
    assert!(client_dir.join("client.turso").exists());
    let _ = fs::remove_dir_all(client_dir);
    let _ = fs::remove_dir_all(project_root);
}

#[tokio::test(flavor = "current_thread")]
async fn db_engine_source_index_selector_payload_proof_roundtrips_to_lookup_candidate() {
    let client_dir = temp_root("db-engine-source-index-proof-client");
    let project_root = temp_root("db-engine-source-index-proof-project");
    let selector =
        "rust://src/source_index_payload_proof.rs#item/function/source_index_payload_proof_fixture";
    let mut source_index_import = build_source_index_import(ClientDbSourceIndexImportRequest {
        generation_id: CacheGenerationId::from("source-index-payload-proof-turso"),
        project_root: project_root.clone(),
        schema_id: SemanticSchemaId::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_ID),
        schema_version: SemanticSchemaVersion::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION),
        selector_source: ClientDbSourceIndexSource::from(CLIENT_DB_SOURCE_INDEX_PROVIDER_ID),
        file_hashes: vec![ClientCacheFileHash {
            path: "src/source_index_payload_proof.rs".to_string(),
            sha256: "abcdef0123456789".repeat(4),
            byte_len: 49,
            mtime_ms: 17,
        }],
        files: vec![ClientDbSourceIndexImportFile {
            relative_path: "src/source_index_payload_proof.rs".to_string(),
            language_id: LanguageId::from("rust"),
            provider_id: ProviderId::from("rs-harness"),
            text: "pub fn source_index_payload_proof_fixture() {}\n".to_string(),
            selectors: Vec::new(),
        }],
    })
    .expect("build Turso source-index payload proof import");
    source_index_import.selectors[0].selector_id = selector.to_string();
    source_index_import.selectors[0].symbol =
        Some("source_index_payload_proof_fixture".to_string());
    source_index_import.selectors[0].kind = Some("function".to_string());
    source_index_import.selectors[0].payload_proof =
        Some(ClientDbSourceIndexSelectorPayloadProof {
            structural_selector: selector.to_string(),
            payload_kind: "code".to_string(),
            bounded: true,
        });

    ClientDbEngine::refresh_source_index_import_from_client_dir(
        &client_dir,
        ClientDbSourceIndexRefreshRequest {
            import: source_index_import,
            file_count: 1,
        },
    )
    .expect("refresh source-index payload proof import");

    let rust_language_id = LanguageId::from("rust");
    let lookup = ClientDbEngine::lookup_source_index_read_model_from_client_dir(
        &client_dir,
        "source_index_payload_proof_fixture",
        Some(&rust_language_id),
        8,
    )
    .await
    .expect("lookup source-index payload proof read model");
    let candidate = lookup
        .candidates
        .iter()
        .find(|candidate| candidate.path == "src/source_index_payload_proof.rs")
        .expect("source-index payload proof candidate");
    let proof = candidate
        .selector_proof
        .as_ref()
        .expect("candidate payload proof");
    assert_eq!(proof.structural_selector, selector);
    assert_eq!(proof.payload_kind, "code");
    assert!(proof.bounded);

    let _ = fs::remove_dir_all(client_dir);
    let _ = fs::remove_dir_all(project_root);
}

#[tokio::test(flavor = "current_thread")]
async fn db_engine_source_index_scope_selector_receipt_roundtrips_to_lookup_candidate() {
    let client_dir = temp_root("db-engine-source-index-scope-proof-client");
    let project_root = temp_root("db-engine-source-index-scope-proof-project");
    let source_path = project_root.join("src/source_index_scope_payload_proof.rs");
    fs::create_dir_all(source_path.parent().expect("source parent")).expect("create source dir");
    fs::write(
        &source_path,
        "pub fn source_index_scope_payload_proof_fixture() {}\n",
    )
    .expect("write source fixture");
    let selector = "rust://src/source_index_scope_payload_proof.rs#item/function/source_index_scope_payload_proof_fixture";
    let import = source_index_import_with_file_hashes(
        ClientDbSourceIndexImportAssemblyRequest {
            generation_id: CacheGenerationId::from("source-index-scope-payload-proof-turso"),
            project_root: project_root.clone(),
            schema_id: SemanticSchemaId::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_ID),
            schema_version: SemanticSchemaVersion::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION),
            selector_source: ClientDbSourceIndexSource::from(CLIENT_DB_SOURCE_INDEX_PROVIDER_ID),
            file_text_bytes_limit: 4096,
            previous_file_hashes: None,
            registry_fingerprint: "scope-payload-proof-registry".to_string(),
            extra_scope_dirs: Vec::new(),
            files: vec![ClientDbSourceIndexScopeFile {
                path: source_path,
                language_id: LanguageId::from("rust"),
                provider_id: ProviderId::from("rs-harness"),
                selector_receipts: vec![ClientDbSourceIndexSelector {
                    owner_path: ClientDbSourceIndexPath::from(
                        "src/source_index_scope_payload_proof.rs",
                    ),
                    selector_id: selector.to_string(),
                    symbol: Some("source_index_scope_payload_proof_fixture".to_string()),
                    kind: Some("function".to_string()),
                    start_line: 1,
                    end_line: 1,
                    source: ClientDbSourceIndexSource::from(CLIENT_DB_SOURCE_INDEX_PROVIDER_ID),
                    query_keys: vec![ClientDbSourceIndexQueryKey::from(
                        "source_index_scope_payload_proof_fixture",
                    )],
                    payload_proof: Some(ClientDbSourceIndexSelectorPayloadProof {
                        structural_selector: selector.to_string(),
                        payload_kind: "code".to_string(),
                        bounded: true,
                    }),
                }],
            }],
        },
        vec![ClientCacheFileHash {
            path: "src/source_index_scope_payload_proof.rs".to_string(),
            sha256: "fedcba9876543210".repeat(4),
            byte_len: 53,
            mtime_ms: 19,
        }],
    )
    .expect("assemble source-index scope payload proof import");

    ClientDbEngine::refresh_source_index_import_from_client_dir(
        &client_dir,
        ClientDbSourceIndexRefreshRequest {
            import,
            file_count: 1,
        },
    )
    .expect("refresh source-index scope payload proof import");

    let rust_language_id = LanguageId::from("rust");
    let lookup = ClientDbEngine::lookup_source_index_read_model_from_client_dir(
        &client_dir,
        "source_index_scope_payload_proof_fixture",
        Some(&rust_language_id),
        8,
    )
    .await
    .expect("lookup source-index scope payload proof read model");
    let proof = lookup
        .candidates
        .iter()
        .find(|candidate| candidate.path == "src/source_index_scope_payload_proof.rs")
        .and_then(|candidate| candidate.selector_proof.as_ref())
        .expect("scope payload proof candidate");
    assert_eq!(proof.structural_selector, selector);
    assert_eq!(proof.payload_kind, "code");
    assert!(proof.bounded);

    let _ = fs::remove_dir_all(client_dir);
    let _ = fs::remove_dir_all(project_root);
}

#[tokio::test(flavor = "current_thread")]
async fn db_engine_source_index_lookup_deduplicates_same_owner_across_generations() {
    let client_dir = temp_root("db-engine-source-index-dedup-client");
    let project_root = temp_root("db-engine-source-index-dedup-project");
    let rust_language_id = LanguageId::from("rust");

    for (generation_id, text, sha_prefix, mtime_ms) in [
        (
            "source-index-dedup-turso-1",
            "pub fn source_index_dedup_fixture() {}\n",
            "1111111111111111",
            11,
        ),
        (
            "source-index-dedup-turso-2",
            "pub fn source_index_dedup_fixture() { let latest_generation = true; }\n",
            "2222222222222222",
            22,
        ),
    ] {
        let source_index_import = build_source_index_import(ClientDbSourceIndexImportRequest {
            generation_id: CacheGenerationId::from(generation_id),
            project_root: project_root.clone(),
            schema_id: SemanticSchemaId::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_ID),
            schema_version: SemanticSchemaVersion::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION),
            selector_source: ClientDbSourceIndexSource::from(CLIENT_DB_SOURCE_INDEX_PROVIDER_ID),
            file_hashes: vec![ClientCacheFileHash {
                path: "src/source_index_dedup.rs".to_string(),
                sha256: sha_prefix.repeat(4),
                byte_len: text.len() as u64,
                mtime_ms,
            }],
            files: vec![ClientDbSourceIndexImportFile {
                relative_path: "src/source_index_dedup.rs".to_string(),
                language_id: rust_language_id.clone(),
                provider_id: ProviderId::from("rs-harness"),
                text: text.to_string(),
                selectors: Vec::new(),
            }],
        })
        .expect("build Turso source-index import");
        let refresh = ClientDbEngine::refresh_source_index_import_from_client_dir(
            &client_dir,
            ClientDbSourceIndexRefreshRequest {
                import: source_index_import,
                file_count: 1,
            },
        )
        .expect("refresh source-index through active Turso DB Engine path");
        assert_eq!(refresh.generation_id.as_str(), generation_id);
        assert!(!refresh.reused_generation);
    }

    let lookup = ClientDbEngine::lookup_source_index_read_model_from_client_dir(
        &client_dir,
        "source_index_dedup_fixture",
        Some(&rust_language_id),
        8,
    )
    .await
    .expect("lookup deduplicated Turso source-index read model");
    assert_eq!(lookup.state, ClientDbSourceIndexLookupState::Hit);
    assert_eq!(
        lookup
            .candidates
            .iter()
            .filter(|candidate| candidate.path == "src/source_index_dedup.rs")
            .count(),
        1,
        "lookup={lookup:?}"
    );
    assert_eq!(
        lookup
            .candidates
            .iter()
            .find(|candidate| candidate.path == "src/source_index_dedup.rs")
            .and_then(|candidate| candidate.line_count),
        Some(1),
        "lookup should attach line count from the latest generation owner row"
    );

    let _ = fs::remove_dir_all(client_dir);
    let _ = fs::remove_dir_all(project_root);
}

#[tokio::test(flavor = "current_thread")]
async fn db_engine_source_index_import_populates_turso_fts_search_documents() {
    let client_dir = temp_root("db-engine-source-index-fts-client");
    let project_root = temp_root("db-engine-source-index-fts-project");
    let rust_language_id = LanguageId::from("rust");
    let source_index_import = build_source_index_import(ClientDbSourceIndexImportRequest {
        generation_id: CacheGenerationId::from("source-index-fts-turso"),
        project_root: project_root.clone(),
        schema_id: SemanticSchemaId::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_ID),
        schema_version: SemanticSchemaVersion::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION),
        selector_source: ClientDbSourceIndexSource::from(CLIENT_DB_SOURCE_INDEX_PROVIDER_ID),
        file_hashes: vec![ClientCacheFileHash {
            path: "src/source_index_fts.rs".to_string(),
            sha256: "4444444444444444".repeat(4),
            byte_len: 82,
            mtime_ms: 44,
        }],
        files: vec![ClientDbSourceIndexImportFile {
            relative_path: "src/source_index_fts.rs".to_string(),
            language_id: rust_language_id.clone(),
            provider_id: ProviderId::from("rs-harness"),
            text: "pub fn source_index_fts_fixture() { let camel_case_identifier = true; }\n"
                .to_string(),
            selectors: Vec::new(),
        }],
    })
    .expect("build Turso source-index FTS import");

    let refresh = ClientDbEngine::refresh_source_index_import_from_client_dir(
        &client_dir,
        ClientDbSourceIndexRefreshRequest {
            import: source_index_import,
            file_count: 1,
        },
    )
    .expect("refresh source-index through Turso FTS search lane");
    assert_eq!(refresh.owner_count, 1);
    assert_eq!(refresh.selector_count, 1);

    let hits = ClientDbEngine::search_source_index_documents_from_client_dir(
        &client_dir,
        "source_index_fts_fixture",
        8,
    )
    .expect("search source-index documents through Turso stable search lane");
    assert!(
        hits.iter().any(|hit| {
            hit.source == "stable"
                && hit
                    .document_id
                    .starts_with("source-index:source-index-fts-turso:")
                && hit.entity_id.as_deref() == Some("src/source_index_fts.rs")
                && hit
                    .selector
                    .as_deref()
                    .is_some_and(|selector| selector == "rust://src/source_index_fts.rs#file")
                && hit.document.contains("source_index_fts_fixture")
        }),
        "hits={hits:?}"
    );

    let lookup = ClientDbEngine::lookup_source_index_read_model_from_client_dir(
        &client_dir,
        "source_index_fts_fixture",
        Some(&rust_language_id),
        8,
    )
    .await
    .expect("lookup source-index read model after Turso FTS smoke");
    assert_eq!(lookup.state, ClientDbSourceIndexLookupState::Hit);
    assert!(
        lookup
            .candidates
            .iter()
            .any(|candidate| candidate.source_kind.as_str() == "turso-source-index"),
        "lookup={lookup:?}"
    );

    let _ = fs::remove_dir_all(client_dir);
    let _ = fs::remove_dir_all(project_root);
}

#[tokio::test(flavor = "current_thread")]
async fn db_engine_source_index_concurrent_inspect_and_lookup_survives_turso_file_locks() {
    let client_dir = temp_root("db-engine-source-index-concurrent-client");
    let project_root = temp_root("db-engine-source-index-concurrent-project");
    let rust_language_id = LanguageId::from("rust");
    let source_index_import = build_source_index_import(ClientDbSourceIndexImportRequest {
        generation_id: CacheGenerationId::from("source-index-concurrent-turso"),
        project_root: project_root.clone(),
        schema_id: SemanticSchemaId::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_ID),
        schema_version: SemanticSchemaVersion::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION),
        selector_source: ClientDbSourceIndexSource::from(CLIENT_DB_SOURCE_INDEX_PROVIDER_ID),
        file_hashes: vec![ClientCacheFileHash {
            path: "src/source_index_concurrent.rs".to_string(),
            sha256: "3333333333333333".repeat(4),
            byte_len: 47,
            mtime_ms: 33,
        }],
        files: vec![ClientDbSourceIndexImportFile {
            relative_path: "src/source_index_concurrent.rs".to_string(),
            language_id: rust_language_id.clone(),
            provider_id: ProviderId::from("rs-harness"),
            text: "pub fn source_index_concurrent_fixture() {}\n".to_string(),
            selectors: Vec::new(),
        }],
    })
    .expect("build concurrent Turso source-index import");
    ClientDbEngine::refresh_source_index_import_from_client_dir(
        &client_dir,
        ClientDbSourceIndexRefreshRequest {
            import: source_index_import,
            file_count: 1,
        },
    )
    .expect("refresh concurrent source-index fixture");

    let shared_client_dir = Arc::new(client_dir.clone());
    let handles = (0..12)
        .map(|worker| {
            let client_dir = Arc::clone(&shared_client_dir);
            let rust_language_id = rust_language_id.clone();
            std::thread::spawn(move || -> Result<(), String> {
                if worker % 2 == 0 {
                    let inspect = ClientDbEngine::inspect_client_dir(client_dir.as_ref());
                    if inspect.source_index_owner_count == 0 {
                        return Err(format!("inspect lost source-index rows: {inspect:?}"));
                    }
                    return Ok(());
                }
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|error| format!("failed to build test runtime: {error}"))?;
                let lookup = runtime.block_on(
                    ClientDbEngine::lookup_source_index_read_model_from_client_dir(
                        client_dir.as_ref(),
                        "source_index_concurrent_fixture",
                        Some(&rust_language_id),
                        8,
                    ),
                )?;
                if lookup.state != ClientDbSourceIndexLookupState::Hit {
                    return Err(format!("concurrent lookup missed source-index: {lookup:?}"));
                }
                Ok(())
            })
        })
        .collect::<Vec<_>>();
    for handle in handles {
        handle
            .join()
            .expect("concurrent source-index worker panicked")
            .expect("concurrent source-index worker failed");
    }

    let _ = fs::remove_dir_all(client_dir);
    let _ = fs::remove_dir_all(project_root);
}

#[tokio::test(flavor = "current_thread")]
async fn db_engine_source_index_refresh_lookup_pressure_returns_busy_instead_of_lock_errors() {
    let client_dir = temp_root("db-engine-source-index-pressure-client");
    let project_root = temp_root("db-engine-source-index-pressure-project");
    let rust_language_id = LanguageId::from("rust");
    let initial_import = build_source_index_import(ClientDbSourceIndexImportRequest {
        generation_id: CacheGenerationId::from("source-index-pressure-turso-initial"),
        project_root: project_root.clone(),
        schema_id: SemanticSchemaId::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_ID),
        schema_version: SemanticSchemaVersion::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION),
        selector_source: ClientDbSourceIndexSource::from(CLIENT_DB_SOURCE_INDEX_PROVIDER_ID),
        file_hashes: vec![ClientCacheFileHash {
            path: "src/source_index_pressure.rs".to_string(),
            sha256: "aaaaaaaaaaaaaaaa".repeat(4),
            byte_len: 58,
            mtime_ms: 1,
        }],
        files: vec![ClientDbSourceIndexImportFile {
            relative_path: "src/source_index_pressure.rs".to_string(),
            language_id: rust_language_id.clone(),
            provider_id: ProviderId::from("rs-harness"),
            text: "pub fn source_index_pressure_fixture() { let initial = true; }\n".to_string(),
            selectors: Vec::new(),
        }],
    })
    .expect("build initial pressure source-index import");
    ClientDbEngine::refresh_source_index_import_from_client_dir(
        &client_dir,
        ClientDbSourceIndexRefreshRequest {
            import: initial_import,
            file_count: 1,
        },
    )
    .expect("refresh initial pressure source-index fixture");

    let shared_client_dir = Arc::new(client_dir.clone());
    let shared_project_root = Arc::new(project_root.clone());
    let completed_lookup_count = Arc::new(AtomicUsize::new(0));
    let busy_lookup_count = Arc::new(AtomicUsize::new(0));

    let writer_client_dir = Arc::clone(&shared_client_dir);
    let writer_project_root = Arc::clone(&shared_project_root);
    let writer_language_id = rust_language_id.clone();
    let writer = std::thread::spawn(move || -> Result<(), String> {
        for round in 0_u64..6 {
            let text = format!(
                "pub fn source_index_pressure_fixture() {{ let generation_{round} = true; }}\n"
            );
            let import = build_source_index_import(ClientDbSourceIndexImportRequest {
                generation_id: CacheGenerationId::from(format!(
                    "source-index-pressure-turso-{round}"
                )),
                project_root: writer_project_root.as_ref().clone(),
                schema_id: SemanticSchemaId::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_ID),
                schema_version: SemanticSchemaVersion::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION),
                selector_source: ClientDbSourceIndexSource::from(
                    CLIENT_DB_SOURCE_INDEX_PROVIDER_ID,
                ),
                file_hashes: vec![ClientCacheFileHash {
                    path: "src/source_index_pressure.rs".to_string(),
                    sha256: format!("{round:016x}").repeat(4),
                    byte_len: text.len() as u64,
                    mtime_ms: round + 2,
                }],
                files: vec![ClientDbSourceIndexImportFile {
                    relative_path: "src/source_index_pressure.rs".to_string(),
                    language_id: writer_language_id.clone(),
                    provider_id: ProviderId::from("rs-harness"),
                    text,
                    selectors: Vec::new(),
                }],
            })?;
            ClientDbEngine::refresh_source_index_import_from_client_dir(
                writer_client_dir.as_ref(),
                ClientDbSourceIndexRefreshRequest {
                    import,
                    file_count: 1,
                },
            )?;
        }
        Ok(())
    });

    let readers = (0..8)
        .map(|_| {
            let client_dir = Arc::clone(&shared_client_dir);
            let rust_language_id = rust_language_id.clone();
            let completed_lookup_count = Arc::clone(&completed_lookup_count);
            let busy_lookup_count = Arc::clone(&busy_lookup_count);
            std::thread::spawn(move || -> Result<(), String> {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|error| format!("failed to build pressure runtime: {error}"))?;
                for _ in 0..12 {
                    let lookup = runtime.block_on(
                        ClientDbEngine::lookup_source_index_read_model_from_client_dir(
                            client_dir.as_ref(),
                            "source_index_pressure_fixture",
                            Some(&rust_language_id),
                            8,
                        ),
                    )?;
                    match lookup.state {
                        ClientDbSourceIndexLookupState::Hit
                        | ClientDbSourceIndexLookupState::Miss
                        | ClientDbSourceIndexLookupState::Busy => {
                            completed_lookup_count.fetch_add(1, Ordering::Relaxed);
                            if lookup.state == ClientDbSourceIndexLookupState::Busy {
                                busy_lookup_count.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                        ClientDbSourceIndexLookupState::MissingDb
                        | ClientDbSourceIndexLookupState::EmptyIndex => {
                            return Err(format!(
                                "pressure lookup saw invalid state after initial refresh: {lookup:?}"
                            ));
                        }
                    }
                }
                Ok(())
            })
        })
        .collect::<Vec<_>>();

    writer
        .join()
        .expect("pressure source-index writer panicked")
        .expect("pressure source-index writer failed");
    for reader in readers {
        reader
            .join()
            .expect("pressure source-index reader panicked")
            .expect("pressure source-index reader failed");
    }

    assert!(
        completed_lookup_count.load(Ordering::Relaxed) >= 8,
        "pressure test should complete concurrent lookup attempts"
    );
    let final_lookup = ClientDbEngine::lookup_source_index_read_model_from_client_dir(
        &client_dir,
        "source_index_pressure_fixture",
        Some(&rust_language_id),
        8,
    )
    .await
    .expect("final pressure lookup should not fail");
    assert_eq!(final_lookup.state, ClientDbSourceIndexLookupState::Hit);
    assert!(
        busy_lookup_count.load(Ordering::Relaxed) <= completed_lookup_count.load(Ordering::Relaxed),
        "busy count must be bounded by completed lookups"
    );

    let _ = fs::remove_dir_all(client_dir);
    let _ = fs::remove_dir_all(project_root);
}

#[tokio::test(flavor = "current_thread")]
async fn db_engine_source_index_lookup_converges_historical_graph_entity_schema() {
    let client_dir = temp_root("db-engine-source-index-graph-migration-client");
    let project_root = temp_root("db-engine-source-index-graph-migration-project");
    fs::create_dir_all(&client_dir).expect("create client dir");
    let db_path = client_dir.join("client.turso");
    {
        let db_path_string = db_path.display().to_string();
        let database = turso::Builder::new_local(&db_path_string)
            .experimental_index_method(true)
            .build()
            .await
            .expect("create historical graph entity fixture database");
        let connection = database
            .connect()
            .expect("connect historical graph entity fixture database");
        connection
            .execute(
                "CREATE TABLE asp_graph_entity (
                    id TEXT PRIMARY KEY,
                    kind TEXT NOT NULL,
                    label TEXT NOT NULL
                )",
                (),
            )
            .await
            .expect("create historical graph entity schema");
    }

    let rust_language_id = LanguageId::from("rust");
    let lookup = ClientDbEngine::lookup_source_index_from_client_dir(
        ClientDbSourceIndexClientDirLookupRequest {
            client_dir: &client_dir,
            indexed_project_root: &project_root,
            language_id: Some(&rust_language_id),
            query_keys: vec![ClientDbSourceIndexQueryKey::from(
                "source_index_graph_migration_fixture",
            )],
            limit: 8,
        },
    )
    .expect("lookup source-index should converge historical graph entity schema");
    assert_eq!(lookup.state, ClientDbSourceIndexLookupState::EmptyIndex);
    assert!(lookup.candidates.is_empty());
    {
        let db_path_string = db_path.display().to_string();
        let database = turso::Builder::new_local(&db_path_string)
            .experimental_index_method(true)
            .build()
            .await
            .expect("reopen historical graph entity fixture database");
        let connection = database
            .connect()
            .expect("connect historical graph entity fixture database after lookup");
        let mut rows = connection
            .query("PRAGMA table_list", ())
            .await
            .expect("inspect source-index tables after read-only lookup");
        let mut found_source_index_table = false;
        while let Some(row) = rows.next().await.expect("read table list row") {
            let table_name = row.get::<String>(1).expect("read table list name");
            if table_name.starts_with("asp_source_index_") {
                found_source_index_table = true;
                break;
            }
        }
        assert!(
            !found_source_index_table,
            "source-index lookup must not bootstrap source-index tables"
        );
    }

    let _ = fs::remove_dir_all(client_dir);
    let _ = fs::remove_dir_all(project_root);
}

#[tokio::test(flavor = "current_thread")]
async fn db_engine_source_index_bootstrap_converges_historical_owner_schema_columns() {
    let client_dir = temp_root("db-engine-source-index-migration-client");
    let project_root = temp_root("db-engine-source-index-migration-project");
    fs::create_dir_all(&client_dir).expect("create client dir");
    let db_path = client_dir.join("client.turso");
    {
        let db_path_string = db_path.display().to_string();
        let database = turso::Builder::new_local(&db_path_string)
            .experimental_index_method(true)
            .build()
            .await
            .expect("create historical source-index fixture database");
        let connection = database
            .connect()
            .expect("connect historical source-index fixture database");
        connection
            .execute(
                "CREATE TABLE asp_source_index_owner (
                    generation_id TEXT NOT NULL,
                    owner_path TEXT NOT NULL,
                    PRIMARY KEY (generation_id, owner_path)
                )",
                (),
            )
            .await
            .expect("create historical source-index owner schema");
    }

    let source_index_import = build_source_index_import(ClientDbSourceIndexImportRequest {
        generation_id: CacheGenerationId::from("source-index-migrated-turso"),
        project_root: project_root.clone(),
        schema_id: SemanticSchemaId::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_ID),
        schema_version: SemanticSchemaVersion::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION),
        selector_source: ClientDbSourceIndexSource::from(CLIENT_DB_SOURCE_INDEX_PROVIDER_ID),
        file_hashes: vec![ClientCacheFileHash {
            path: "src/source_index_migrated_turso.rs".to_string(),
            sha256: "abcdef1234567890".repeat(4),
            byte_len: 51,
            mtime_ms: 12,
        }],
        files: vec![ClientDbSourceIndexImportFile {
            relative_path: "src/source_index_migrated_turso.rs".to_string(),
            language_id: LanguageId::from("rust"),
            provider_id: ProviderId::from("rs-harness"),
            text: "pub fn source_index_migrated_turso_fixture() {}\n".to_string(),
            selectors: Vec::new(),
        }],
    })
    .expect("build migrated Turso source-index import");

    let refresh = ClientDbEngine::refresh_source_index_import_from_client_dir(
        &client_dir,
        ClientDbSourceIndexRefreshRequest {
            import: source_index_import,
            file_count: 1,
        },
    )
    .expect("refresh source-index through migrated Turso owner schema");
    assert_eq!(
        refresh.generation_id.as_str(),
        "source-index-migrated-turso"
    );
    assert_eq!(refresh.owner_count, 1);
    assert_eq!(refresh.selector_count, 1);

    let rust_language_id = LanguageId::from("rust");
    let lookup = ClientDbEngine::lookup_source_index_read_model_from_client_dir(
        &client_dir,
        "source_index_migrated_turso_fixture",
        Some(&rust_language_id),
        8,
    )
    .await
    .expect("lookup migrated Turso source-index read model");
    assert_eq!(lookup.state, ClientDbSourceIndexLookupState::Hit);
    assert!(
        lookup.candidates.iter().any(|candidate| candidate.path
            == "src/source_index_migrated_turso.rs"
            && candidate.language_id.as_ref().map(|id| id.as_str()) == Some("rust")
            && candidate.provider_id.as_ref().map(|id| id.as_str()) == Some("rs-harness")),
        "lookup={lookup:?}"
    );
    let facade_lookup = ClientDbEngine::lookup_source_index_from_client_dir(
        ClientDbSourceIndexClientDirLookupRequest {
            client_dir: &client_dir,
            indexed_project_root: &project_root,
            language_id: Some(&rust_language_id),
            query_keys: vec![ClientDbSourceIndexQueryKey::from(
                "source_index_migrated_turso_fixture",
            )],
            limit: 8,
        },
    )
    .expect("lookup source-index through facade with migrated Turso owner schema");
    assert_eq!(facade_lookup.state, ClientDbSourceIndexLookupState::Hit);
    assert!(
        facade_lookup
            .candidates
            .iter()
            .any(|candidate| candidate.path == "src/source_index_migrated_turso.rs"),
        "facade_lookup={facade_lookup:?}"
    );

    let _ = fs::remove_dir_all(client_dir);
    let _ = fs::remove_dir_all(project_root);
}

fn temp_root(label: &str) -> PathBuf {
    let mut root = std::env::temp_dir();
    let unique = format!(
        "asp-client-db-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos()
    );
    root.push(unique);
    fs::create_dir_all(&root).expect("create temp root");
    root
}
