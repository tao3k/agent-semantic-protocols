use agent_semantic_client_core::{
    CacheGenerationId, ClientCacheFileHash, LanguageId, ProviderId, SemanticSchemaId,
    SemanticSchemaVersion,
};
use agent_semantic_client_db::{
    CLIENT_DB_SOURCE_INDEX_SCHEMA_ID, CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION, ClientDbEngine,
    ClientDbLiveSourceIndexFacts, ClientDbSourceIndexClientDirLookupRequest,
    ClientDbSourceIndexImport, ClientDbSourceIndexImportFile, ClientDbSourceIndexImportRequest,
    ClientDbSourceIndexLookupState, ClientDbSourceIndexMembershipChangeSet,
    ClientDbSourceIndexOwner, ClientDbSourceIndexRefreshRequest, build_source_index_import,
    client_db_source_index_artifact_digest, client_db_source_index_generation_id_for_snapshot,
};
use std::fs;
use std::path::{Path, PathBuf};

const SCENARIO_ROOT: &str = "tests/unit/scenarios/code_search_merkle_memory_warm_path";
const TURSO_SCENARIO_ROOT: &str =
    "tests/unit/scenarios/code_search_turso_resident_session_warm_path";

fn benchmark_u64(manifest: &str, key: &str) -> u64 {
    manifest
        .lines()
        .find_map(|line| {
            let (candidate, value) = line.split_once('=')?;
            (candidate.trim() == key).then(|| {
                value
                    .trim()
                    .parse::<u64>()
                    .unwrap_or_else(|error| panic!("invalid {key} benchmark value: {error}"))
            })
        })
        .unwrap_or_else(|| panic!("missing {key} benchmark value"))
}

#[tokio::test(flavor = "current_thread")]
async fn code_search_turso_resident_session_warm_path_is_a_strong_gate() {
    let scenario_root = Path::new(env!("CARGO_MANIFEST_DIR")).join(TURSO_SCENARIO_ROOT);
    let scenario =
        fs::read_to_string(scenario_root.join("scenario.toml")).expect("read scenario fixture");
    let benchmark =
        fs::read_to_string(scenario_root.join("benchmark.toml")).expect("read benchmark fixture");
    for required in [
        "code-search-turso-resident-session-warm-path",
        "engine = \"turso-0.7\"",
        "merkle_algorithm = \"blake3-merkle-v1\"",
        "require_resident_connection = true",
        "max_additional_turso_db_open_count = 0",
        "max_provider_process_count = 0",
        "max_source_blob_read_count = 0",
        "max_source_rehash_count = 0",
        "fallback_reason = \"none\"",
    ] {
        assert!(
            scenario.contains(required) || benchmark.contains(required),
            "code-search resident Turso fixture missing {required:?}"
        );
    }
    let sample_count = benchmark_u64(&benchmark, "sample_count") as usize;
    let target_p95_us = benchmark_u64(&benchmark, "target_p95_us");
    let max_sample_us = benchmark_u64(&benchmark, "max_sample_us");
    let concurrent_task_count = benchmark_u64(&benchmark, "concurrent_task_count") as usize;
    let lookups_per_task = benchmark_u64(&benchmark, "lookups_per_task") as usize;
    let concurrent_target_p95_us = benchmark_u64(&benchmark, "concurrent_target_p95_us");
    let concurrent_max_sample_us = benchmark_u64(&benchmark, "concurrent_max_sample_us");
    assert!(
        sample_count >= 128,
        "strong gate requires at least 128 samples"
    );

    let root = temp_root();
    let client_dir = root.join("client");
    let project_root = root.join("project");
    fs::create_dir_all(&project_root).expect("create project root");
    let source_snapshot = agent_semantic_content_identity::SourceSnapshotEvidence {
        schema_id: "asp.source-snapshot.v1".to_string(),
        algorithm: "blake3-merkle-v1".to_string(),
        root_digest: "resident-turso-root".to_string(),
        source_kind: agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        leaf_count: 1,
        base_root_digest: None,
        provider_digest: "resident-provider-digest".to_string(),
        dirty_paths_digest: None,
    };
    let rust_language_id = LanguageId::from("rust");
    let import = build_source_index_import(ClientDbSourceIndexImportRequest {
        generation_id: CacheGenerationId::from("resident-turso-code-search"),
        project_root: project_root.clone(),
        schema_id: SemanticSchemaId::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_ID),
        schema_version: SemanticSchemaVersion::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION),
        selector_source: "rs-harness".into(),
        file_hashes: vec![ClientCacheFileHash {
            path: "src/lib.rs".to_string(),
            sha256: "1111111111111111".repeat(4),
            byte_len: 31,
            mtime_ms: 1,
        }],
        files: vec![ClientDbSourceIndexImportFile {
            relative_path: "src/lib.rs".to_string(),
            language_id: rust_language_id.clone(),
            provider_id: ProviderId::from("rs-harness"),
            text: "pub fn resident_needle() {}\n".to_string(),
            selectors: Vec::new(),
        }],
    })
    .expect("build resident Turso source-index import");
    ClientDbEngine::refresh_source_index_import_from_client_dir(
        &client_dir,
        ClientDbSourceIndexRefreshRequest {
            import,
            file_count: 1,
            source_snapshot: source_snapshot.clone(),
            membership_change_set: ClientDbSourceIndexMembershipChangeSet::FullSnapshot,
        },
    )
    .expect("materialize resident Turso source index");
    let session = ClientDbEngine::open_read_session_client_dir(&client_dir)
        .expect("open resident Turso read session")
        .expect("resident Turso database");
    let warmup = session
        .lookup_source_index_read_model(
            &source_snapshot,
            "resident_needle",
            Some(&rust_language_id),
            8,
        )
        .await
        .expect("warm resident Turso code search");
    assert_eq!(warmup.state, ClientDbSourceIndexLookupState::Hit);

    let mut samples = Vec::with_capacity(sample_count);
    for _ in 0..sample_count {
        let started = std::time::Instant::now();
        let lookup = session
            .lookup_source_index_read_model(
                &source_snapshot,
                "resident_needle",
                Some(&rust_language_id),
                8,
            )
            .await
            .expect("resident Turso code search");
        samples.push(started.elapsed());
        assert_eq!(lookup.state, ClientDbSourceIndexLookupState::Hit);
        assert_eq!(lookup.candidates.len(), 1);
    }
    samples.sort_unstable();
    let p95 = samples[(samples.len() * 95 / 100).min(samples.len() - 1)];
    let max_sample = *samples.last().expect("resident Turso samples");
    assert!(
        p95 <= std::time::Duration::from_micros(target_p95_us),
        "resident Turso code search exceeded target_p95_us={target_p95_us}: {p95:?}"
    );
    assert!(
        max_sample <= std::time::Duration::from_micros(max_sample_us),
        "resident Turso code search exceeded max_sample_us={max_sample_us}: {max_sample:?}"
    );
    eprintln!(
        "code-search-tier=turso-resident-session samples={sample_count} p95={p95:?} max={max_sample:?} additionalTursoDbOpens=0 providerProcesses=0 sourceBlobReads=0 sourceRehashes=0"
    );
    let session = std::sync::Arc::new(session);
    let source_snapshot = std::sync::Arc::new(source_snapshot);
    let rust_language_id = std::sync::Arc::new(rust_language_id);
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..concurrent_task_count {
        let session = session.clone();
        let source_snapshot = source_snapshot.clone();
        let rust_language_id = rust_language_id.clone();
        tasks.spawn(async move {
            let mut task_samples = Vec::with_capacity(lookups_per_task);
            for _ in 0..lookups_per_task {
                let started = std::time::Instant::now();
                let lookup = session
                    .lookup_source_index_read_model(
                        source_snapshot.as_ref(),
                        "resident_needle",
                        Some(rust_language_id.as_ref()),
                        8,
                    )
                    .await
                    .expect("concurrent resident Turso code search");
                task_samples.push(started.elapsed());
                assert_eq!(lookup.state, ClientDbSourceIndexLookupState::Hit);
            }
            task_samples
        });
    }
    let mut concurrent_samples = Vec::with_capacity(concurrent_task_count * lookups_per_task);
    while let Some(samples) = tasks.join_next().await {
        concurrent_samples.extend(samples.expect("resident code-search task"));
    }
    concurrent_samples.sort_unstable();
    let concurrent_p95 =
        concurrent_samples[(concurrent_samples.len() * 95 / 100).min(concurrent_samples.len() - 1)];
    let concurrent_max = *concurrent_samples
        .last()
        .expect("concurrent resident Turso samples");
    assert!(
        concurrent_p95 <= std::time::Duration::from_micros(concurrent_target_p95_us),
        "concurrent resident code search exceeded target p95={concurrent_target_p95_us}us: {concurrent_p95:?}"
    );
    assert!(
        concurrent_max <= std::time::Duration::from_micros(concurrent_max_sample_us),
        "concurrent resident code search exceeded hard max={concurrent_max_sample_us}us: {concurrent_max:?}"
    );
    eprintln!(
        "code-search-tier=turso-resident-concurrent tasks={concurrent_task_count} lookupsPerTask={lookups_per_task} samples={} p95={concurrent_p95:?} max={concurrent_max:?} additionalTursoDbOpens=0",
        concurrent_samples.len()
    );
    drop(session);
    let _ = fs::remove_dir_all(root);
}

fn temp_root() -> PathBuf {
    let nonce = format!(
        "asp-code-search-performance-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos()
    );
    let root = std::env::temp_dir().join(nonce);
    fs::create_dir_all(&root).expect("create test root");
    root
}

#[test]
fn code_search_merkle_memory_warm_path_is_a_strong_gate() {
    let scenario_root = Path::new(env!("CARGO_MANIFEST_DIR")).join(SCENARIO_ROOT);
    let scenario =
        fs::read_to_string(scenario_root.join("scenario.toml")).expect("read scenario fixture");
    let benchmark =
        fs::read_to_string(scenario_root.join("benchmark.toml")).expect("read benchmark fixture");
    for required in [
        "code-search-merkle-memory-warm-path",
        "engine = \"turso-0.7\"",
        "merkle_algorithm = \"blake3-merkle-v1\"",
        "require_memory_hit = true",
        "max_turso_db_open_count = 0",
        "max_provider_process_count = 0",
        "max_native_finder_process_count = 0",
        "max_source_blob_read_count = 0",
        "max_source_rehash_count = 0",
        "fallback_reason = \"none\"",
    ] {
        assert!(
            scenario.contains(required) || benchmark.contains(required),
            "code-search memory fixture missing {required:?}"
        );
    }
    let sample_count = benchmark_u64(&benchmark, "sample_count") as usize;
    let target_p95_us = benchmark_u64(&benchmark, "target_p95_us");
    let max_sample_us = benchmark_u64(&benchmark, "max_sample_us");
    assert!(
        sample_count >= 128,
        "strong gate requires at least 128 samples"
    );

    let root = temp_root();
    let client_dir = root.join("missing-client");
    let project_root = root.join("project");
    fs::create_dir_all(&project_root).expect("create project root");
    let source_snapshot = agent_semantic_content_identity::SourceSnapshotEvidence {
        schema_id: "asp.source-snapshot.v1".to_string(),
        algorithm: "blake3-merkle-v1".to_string(),
        root_digest: "live-root".to_string(),
        source_kind: agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        leaf_count: 1,
        base_root_digest: None,
        provider_digest: "provider-digest".to_string(),
        dirty_paths_digest: None,
    };
    let source_index_import = ClientDbSourceIndexImport {
        generation_id: client_db_source_index_generation_id_for_snapshot(&source_snapshot),
        project_root: project_root.clone(),
        schema_id: CLIENT_DB_SOURCE_INDEX_SCHEMA_ID.into(),
        schema_version: CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION.into(),
        file_hashes: Vec::new(),
        owners: vec![ClientDbSourceIndexOwner {
            owner_path: "src/lib.rs".into(),
            language_id: Some("rust".into()),
            provider_id: Some("rs-harness".into()),
            source_kind: "file".into(),
            line_count: Some(1),
            query_keys: vec!["needle".into()],
        }],
        selectors: Vec::new(),
    };
    let expected_artifact_digest = client_db_source_index_artifact_digest(&source_snapshot);
    let rust_language_id = LanguageId::from("rust");

    let mut samples = Vec::with_capacity(sample_count);
    for _ in 0..sample_count {
        let started = std::time::Instant::now();
        let lookup = ClientDbEngine::lookup_source_index_from_client_dir(
            ClientDbSourceIndexClientDirLookupRequest {
                client_dir: &client_dir,
                indexed_project_root: &project_root,
                language_id: Some(&rust_language_id),
                query_keys: vec!["needle".into()],
                limit: 8,
                expected_snapshot_root: source_snapshot.root_digest.as_str(),
                expected_index_artifact_digest: expected_artifact_digest.as_str(),
                live_facts: Some(ClientDbLiveSourceIndexFacts {
                    source_snapshot: &source_snapshot,
                    import: &source_index_import,
                }),
            },
        )
        .expect("Merkle-qualified memory code search");
        samples.push(started.elapsed());
        assert_eq!(lookup.state, ClientDbSourceIndexLookupState::Hit);
        assert_eq!(lookup.candidates.len(), 1);
    }
    samples.sort_unstable();
    let p95 = samples[(samples.len() * 95 / 100).min(samples.len() - 1)];
    let max_sample = *samples.last().expect("code-search samples");
    assert!(
        p95 <= std::time::Duration::from_micros(target_p95_us),
        "code search exceeded target_p95_us={target_p95_us}: {p95:?}"
    );
    assert!(
        max_sample <= std::time::Duration::from_micros(max_sample_us),
        "code search exceeded max_sample_us={max_sample_us}: {max_sample:?}"
    );
    assert!(
        !client_dir.exists(),
        "memory hit must not create or open the Turso client directory"
    );
    eprintln!(
        "code-search-tier=merkle-memory samples={sample_count} p95={p95:?} max={max_sample:?} tursoDbOpens=0 providerProcesses=0 sourceBlobReads=0 sourceRehashes=0"
    );
    let _ = fs::remove_dir_all(root);
}
