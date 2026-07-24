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
    ClientDbSourceIndexImportAssemblyRequest, ClientDbSourceIndexImportFile,
    ClientDbSourceIndexImportRequest, ClientDbSourceIndexLookupState, ClientDbSourceIndexPath,
    ClientDbSourceIndexQueryKey, ClientDbSourceIndexRefreshRequest, ClientDbSourceIndexScopeFile,
    ClientDbSourceIndexSelector, ClientDbSourceIndexSelectorPayloadProof,
    ClientDbSourceIndexSource, build_source_index_import, source_index_import_with_file_hashes,
};

#[path = "engine_source_index/exact_selector_projection.rs"]
mod exact_selector_projection;
#[path = "engine_source_index/language_projection.rs"]
mod language_projection;
pub use agent_semantic_client_db::ClientDbSourceIndexImport;
use agent_semantic_client_db::ClientDbSourceIndexMembershipChangeSet;

#[path = "engine_source_index/merkle_overlay_refresh.rs"]
mod merkle_overlay_refresh;

#[path = "engine_source_index/snapshot_helper.rs"]
mod snapshot_helper;
#[cfg(unix)]
use snapshot_helper::client_dir_snapshot;

#[tokio::test(flavor = "current_thread")]
async fn db_engine_source_index_import_uses_canonical_snapshot_without_fts_control() {
    let client_dir = temp_root("db-engine-source-index-client");
    let project_root = temp_root("db-engine-source-index-project");
    let source_snapshot = crate::snapshot_fixture::source_snapshot_evidence();
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
            source_snapshot: source_snapshot.clone(),
            membership_change_set: ClientDbSourceIndexMembershipChangeSet::FullSnapshot,
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
            source_snapshot: source_snapshot.clone(),
            membership_change_set: ClientDbSourceIndexMembershipChangeSet::FullSnapshot,
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
        &source_snapshot,
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
    assert!(client_dir.join("facts.turso").exists());
    let _ = fs::remove_dir_all(client_dir);
    let _ = fs::remove_dir_all(project_root);
}

#[tokio::test(flavor = "current_thread")]
async fn db_engine_source_index_selector_payload_proof_roundtrips_to_lookup_candidate() {
    let client_dir = temp_root("db-engine-source-index-proof-client");
    let project_root = temp_root("db-engine-source-index-proof-project");
    let source_snapshot = crate::snapshot_fixture::source_snapshot_evidence();
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
            source_snapshot: source_snapshot.clone(),
            membership_change_set: ClientDbSourceIndexMembershipChangeSet::FullSnapshot,
        },
    )
    .expect("refresh source-index payload proof import");

    let rust_language_id = LanguageId::from("rust");
    let lookup = ClientDbEngine::lookup_source_index_read_model_from_client_dir(
        &client_dir,
        &source_snapshot,
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

    let other_language_id = LanguageId::from("gerbil-scheme");
    let other_language_lookup = ClientDbEngine::lookup_source_index_read_model_from_client_dir(
        &client_dir,
        &source_snapshot,
        "source_index_scope_payload_proof_fixture",
        Some(&other_language_id),
        8,
    )
    .await
    .expect("lookup source-index other-language read model");
    assert!(other_language_lookup.candidates.is_empty());

    let _ = fs::remove_dir_all(client_dir);
    let _ = fs::remove_dir_all(project_root);
}

#[tokio::test(flavor = "current_thread")]
async fn db_engine_source_index_scope_selector_receipt_roundtrips_to_lookup_candidate() {
    let client_dir = temp_root("db-engine-source-index-scope-proof-client");
    let project_root = temp_root("db-engine-source-index-scope-proof-project");
    let source_snapshot = crate::snapshot_fixture::source_snapshot_evidence();
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
            source_snapshot: source_snapshot.clone(),
            membership_change_set: ClientDbSourceIndexMembershipChangeSet::FullSnapshot,
        },
    )
    .expect("refresh source-index scope payload proof import");

    let rust_language_id = LanguageId::from("rust");
    let lookup = ClientDbEngine::lookup_source_index_read_model_from_client_dir(
        &client_dir,
        &source_snapshot,
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
    let source_snapshot = crate::snapshot_fixture::source_snapshot_evidence();
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
                membership_change_set: ClientDbSourceIndexMembershipChangeSet::FullSnapshot,
                import: source_index_import,
                file_count: 1,
                source_snapshot: source_snapshot.clone(),
            },
        )
        .expect("refresh source-index through active Turso DB Engine path");
        assert_eq!(refresh.generation_id.as_str(), generation_id);
        assert!(!refresh.reused_generation);
    }

    let lookup = ClientDbEngine::lookup_source_index_read_model_from_client_dir(
        &client_dir,
        &source_snapshot,
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
async fn db_engine_source_index_import_does_not_populate_turso_fts_search_documents() {
    let client_dir = temp_root("db-engine-source-index-fts-client");
    let project_root = temp_root("db-engine-source-index-fts-project");
    let source_snapshot = crate::snapshot_fixture::source_snapshot_evidence();
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
            import: source_index_import.clone(),
            file_count: 1,
            source_snapshot: source_snapshot.clone(),
            membership_change_set: ClientDbSourceIndexMembershipChangeSet::FullSnapshot,
        },
    )
    .expect("refresh source-index through Turso FTS search lane");
    assert_eq!(refresh.owner_count, 1);
    assert_eq!(refresh.selector_count, 1);

    let hits = ClientDbEngine::search_source_index_documents_from_client_dir(
        &client_dir,
        &source_snapshot,
        "source_index_fts_fixture",
        8,
    )
    .expect("search source-index documents through Turso stable search lane");
    assert!(hits.hits.is_empty(), "hits={hits:?}");

    let lookup = ClientDbEngine::lookup_source_index_read_model_from_client_dir(
        &client_dir,
        &source_snapshot,
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
    let source_snapshot = crate::snapshot_fixture::source_snapshot_evidence();
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
            source_snapshot: source_snapshot.clone(),
            membership_change_set: ClientDbSourceIndexMembershipChangeSet::FullSnapshot,
        },
    )
    .expect("refresh concurrent source-index fixture");

    let shared_client_dir = Arc::new(client_dir.clone());
    let handles = (0..12)
        .map(|worker| {
            let client_dir = Arc::clone(&shared_client_dir);
            let rust_language_id = rust_language_id.clone();
            let source_snapshot = source_snapshot.clone();
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
                        &source_snapshot,
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

#[cfg(unix)]
#[tokio::test(flavor = "current_thread")]
async fn db_engine_source_index_lookup_succeeds_without_client_dir_write_permission() {
    use std::os::unix::fs::PermissionsExt;

    let client_dir = temp_root("db-engine-source-index-read-only-client");
    let project_root = temp_root("db-engine-source-index-read-only-project");
    let source_snapshot = crate::snapshot_fixture::source_snapshot_evidence();
    let rust_language_id = LanguageId::from("rust");
    let source_index_import = build_source_index_import(ClientDbSourceIndexImportRequest {
        generation_id: CacheGenerationId::from("source-index-read-only-turso"),
        project_root: project_root.clone(),
        schema_id: SemanticSchemaId::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_ID),
        schema_version: SemanticSchemaVersion::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION),
        selector_source: ClientDbSourceIndexSource::from(CLIENT_DB_SOURCE_INDEX_PROVIDER_ID),
        file_hashes: vec![ClientCacheFileHash {
            path: "src/source_index_read_only.rs".to_string(),
            sha256: "4444444444444444".repeat(4),
            byte_len: 45,
            mtime_ms: 44,
        }],
        files: vec![ClientDbSourceIndexImportFile {
            relative_path: "src/source_index_read_only.rs".to_string(),
            language_id: rust_language_id.clone(),
            provider_id: ProviderId::from("rs-harness"),
            text: "pub fn source_index_read_only_fixture() {}\n".to_string(),
            selectors: Vec::new(),
        }],
    })
    .expect("build read-only Turso source-index import");
    ClientDbEngine::refresh_source_index_import_from_client_dir(
        &client_dir,
        ClientDbSourceIndexRefreshRequest {
            import: source_index_import,
            file_count: 1,
            source_snapshot: source_snapshot.clone(),
            membership_change_set: ClientDbSourceIndexMembershipChangeSet::FullSnapshot,
        },
    )
    .expect("refresh read-only source-index fixture");

    let before = client_dir_snapshot(&client_dir);
    let entries = fs::read_dir(&client_dir)
        .expect("read client directory before permission change")
        .map(|entry| entry.expect("read client entry").path())
        .collect::<Vec<_>>();
    for path in &entries {
        let mode = if path.is_dir() { 0o555 } else { 0o444 };
        fs::set_permissions(path, fs::Permissions::from_mode(mode))
            .expect("make client entry read-only");
    }
    fs::set_permissions(&client_dir, fs::Permissions::from_mode(0o555))
        .expect("make client directory read-only");

    let lookup_started_at = std::time::Instant::now();
    let lookup_result = ClientDbEngine::lookup_source_index_read_model_from_client_dir(
        &client_dir,
        &source_snapshot,
        "source_index_read_only_fixture",
        Some(&rust_language_id),
        8,
    )
    .await;
    let lookup_elapsed = lookup_started_at.elapsed();

    fs::set_permissions(&client_dir, fs::Permissions::from_mode(0o755))
        .expect("restore client directory permission");
    for path in &entries {
        let mode = if path.is_dir() { 0o755 } else { 0o644 };
        fs::set_permissions(path, fs::Permissions::from_mode(mode))
            .expect("restore client entry permission");
    }

    let lookup = lookup_result.expect("read-only source-index lookup succeeds");
    assert_eq!(lookup.state, ClientDbSourceIndexLookupState::Hit);
    assert_eq!(lookup.candidates.len(), 1);
    assert!(
        lookup_elapsed <= std::time::Duration::from_millis(100),
        "read-only source-index lookup exceeded 100ms: {lookup_elapsed:?}"
    );
    assert_eq!(client_dir_snapshot(&client_dir), before);

    let _ = fs::remove_dir_all(client_dir);
    let _ = fs::remove_dir_all(project_root);
}

#[tokio::test(flavor = "current_thread")]
async fn db_engine_source_index_refresh_lookup_pressure_returns_busy_instead_of_lock_errors() {
    let client_dir = temp_root("db-engine-source-index-pressure-client");
    let project_root = temp_root("db-engine-source-index-pressure-project");
    let rust_language_id = LanguageId::from("rust");
    let initial_source_snapshot = crate::snapshot_fixture::source_snapshot_evidence_for(1);
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
            source_snapshot: initial_source_snapshot.clone(),
            membership_change_set: ClientDbSourceIndexMembershipChangeSet::FullSnapshot,
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
                    membership_change_set: ClientDbSourceIndexMembershipChangeSet::FullSnapshot,
                    import,
                    file_count: 1,
                    source_snapshot: crate::snapshot_fixture::source_snapshot_evidence_for(
                        round + 2,
                    ),
                },
            )?;
        }
        Ok(())
    });

    let readers = (0..8)
        .map(|_| {
            let client_dir = Arc::clone(&shared_client_dir);
            let rust_language_id = rust_language_id.clone();
            let source_snapshot = initial_source_snapshot.clone();
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
                            &source_snapshot,
                            "source_index_pressure_fixture",
                            Some(&rust_language_id),
                            8,
                        ),
                    )?;
                    match lookup.state {
                        ClientDbSourceIndexLookupState::Hit
                        | ClientDbSourceIndexLookupState::Miss
                        | ClientDbSourceIndexLookupState::Busy
                        | ClientDbSourceIndexLookupState::ColdRequired => {
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
    let final_lookup_deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    let final_source_snapshot = crate::snapshot_fixture::source_snapshot_evidence_for(7);
    let final_lookup = loop {
        let lookup = ClientDbEngine::lookup_source_index_read_model_from_client_dir(
            &client_dir,
            &final_source_snapshot,
            "source_index_pressure_fixture",
            Some(&rust_language_id),
            8,
        )
        .await
        .expect("final pressure lookup should not fail");
        if lookup.state != ClientDbSourceIndexLookupState::Busy {
            break lookup;
        }
        assert!(
            std::time::Instant::now() < final_lookup_deadline,
            "final pressure lookup remained busy past the test deadline"
        );
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    };
    assert_eq!(final_lookup.state, ClientDbSourceIndexLookupState::Hit);
    assert!(
        busy_lookup_count.load(Ordering::Relaxed) <= completed_lookup_count.load(Ordering::Relaxed),
        "busy count must be bounded by completed lookups"
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

#[tokio::test(flavor = "current_thread")]
async fn db_engine_source_index_refresh_rebuilds_noncanonical_snapshot_schema() {
    let client_dir = temp_root("db-engine-source-index-canonical-schema-client");
    let project_root = temp_root("db-engine-source-index-canonical-schema-project");
    let source_snapshot = crate::snapshot_fixture::source_snapshot_evidence();
    fs::create_dir_all(&client_dir).expect("create client dir");
    let db_path = client_dir.join("facts.turso");
    {
        let db_path_string = db_path.display().to_string();
        let database = turso::Builder::new_local(&db_path_string)
            .experimental_index_method(true)
            .build()
            .await
            .expect("create noncanonical source-index database");
        let connection = database
            .connect()
            .expect("connect noncanonical source-index database");
        connection
            .execute(
                "CREATE TABLE asp_source_index_scope_v1 (obsolete_marker TEXT NOT NULL)",
                (),
            )
            .await
            .expect("create noncanonical source-index scope table");
    }
    let cold_lookup = ClientDbEngine::lookup_source_index_read_model_from_client_dir(
        &client_dir,
        &source_snapshot,
        "canonical_schema",
        Some(&LanguageId::from("rust")),
        1,
    )
    .await
    .expect("read-only lookup of noncanonical source-index namespace");
    assert_eq!(
        cold_lookup.state,
        ClientDbSourceIndexLookupState::ColdRequired
    );
    {
        let db_path_string = db_path.display().to_string();
        let database = turso::Builder::new_local(&db_path_string)
            .experimental_index_method(true)
            .build()
            .await
            .expect("reopen noncanonical source-index database after lookup");
        let connection = database
            .connect()
            .expect("connect noncanonical source-index database after lookup");
        let mut rows = connection
            .query("PRAGMA table_info(asp_source_index_scope_v1)", ())
            .await
            .expect("inspect noncanonical schema after read-only lookup");
        let mut saw_obsolete_marker = false;
        while let Some(row) = rows.next().await.expect("read noncanonical scope column") {
            saw_obsolete_marker |= row
                .get::<String>(1)
                .expect("read noncanonical source-index column")
                == "obsolete_marker";
        }
        assert!(
            saw_obsolete_marker,
            "read-only lookup must not mutate a noncanonical derived namespace"
        );
    }
    let source_index_import = build_source_index_import(ClientDbSourceIndexImportRequest {
        generation_id: CacheGenerationId::from("source-index-canonical-schema-turso"),
        project_root: project_root.clone(),
        schema_id: SemanticSchemaId::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_ID),
        schema_version: SemanticSchemaVersion::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION),
        selector_source: ClientDbSourceIndexSource::from(CLIENT_DB_SOURCE_INDEX_PROVIDER_ID),
        file_hashes: vec![ClientCacheFileHash {
            path: "src/canonical_schema.rs".to_string(),
            sha256: "5555555555555555".repeat(4),
            byte_len: 1,
            mtime_ms: 55,
        }],
        files: vec![ClientDbSourceIndexImportFile {
            relative_path: "src/canonical_schema.rs".to_string(),
            language_id: LanguageId::from("rust"),
            provider_id: ProviderId::from("rs-harness"),
            text: "fn canonical_schema() {}\n".to_string(),
            selectors: Vec::new(),
        }],
    })
    .expect("build canonical source-index import");
    ClientDbEngine::refresh_source_index_import_from_client_dir(
        &client_dir,
        ClientDbSourceIndexRefreshRequest {
            import: source_index_import,
            file_count: 1,
            source_snapshot: source_snapshot.clone(),
            membership_change_set: ClientDbSourceIndexMembershipChangeSet::FullSnapshot,
        },
    )
    .expect("bootstrap canonical source-index schema");
    let lookup = ClientDbEngine::lookup_source_index_read_model_from_client_dir(
        &client_dir,
        &source_snapshot,
        "canonical_schema",
        Some(&LanguageId::from("rust")),
        1,
    )
    .await
    .expect("lookup after canonical schema bootstrap");
    assert_eq!(lookup.state, ClientDbSourceIndexLookupState::Hit);
    {
        let db_path_string = db_path.display().to_string();
        let database = turso::Builder::new_local(&db_path_string)
            .experimental_index_method(true)
            .build()
            .await
            .expect("reopen canonical source-index database");
        let connection = database
            .connect()
            .expect("connect canonical source-index database");
        let mut rows = connection
            .query("PRAGMA table_info(asp_source_index_scope_v1)", ())
            .await
            .expect("inspect canonical source-index scope columns");
        let mut columns = Vec::new();
        while let Some(row) = rows.next().await.expect("read source-index scope column") {
            columns.push(row.get::<String>(1).expect("read source-index column name"));
        }
        assert!(
            columns
                .iter()
                .any(|column| column == "source_snapshot_json")
        );
        assert!(
            columns
                .iter()
                .any(|column| column == "selector_fingerprint")
        );
        assert!(!columns.iter().any(|column| column == "obsolete_marker"));
    }
    let _ = fs::remove_dir_all(client_dir);
    let _ = fs::remove_dir_all(project_root);
}
#[path = "engine_source_index/active_fact.rs"]
mod active_fact;
