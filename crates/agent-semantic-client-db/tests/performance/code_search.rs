// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use agent_semantic_client_core::ClientCacheFileHash;
use agent_semantic_client_core::LanguageId;
use agent_semantic_client_core::ProviderId;
use agent_semantic_client_core::SemanticSchemaId;
use agent_semantic_client_core::SemanticSchemaVersion;
use agent_semantic_client_db::CLIENT_DB_SOURCE_INDEX_SCHEMA_ID;
use agent_semantic_client_db::CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION;
use agent_semantic_client_db::ClientDbEngine;
use agent_semantic_client_db::ClientDbLiveSourceIndexFacts;
use agent_semantic_client_db::ClientDbSourceIndexClientDirLookupRequest;
use agent_semantic_client_db::ClientDbSourceIndexImport;
use agent_semantic_client_db::ClientDbSourceIndexImportFile;
use agent_semantic_client_db::ClientDbSourceIndexImportRequest;
use agent_semantic_client_db::ClientDbSourceIndexLookupState;
use agent_semantic_client_db::ClientDbSourceIndexOwner;
use agent_semantic_client_db::ClientDbSourceIndexRefreshRequest;
use agent_semantic_client_db::build_source_index_import;
use agent_semantic_client_db::client_db_source_index_generation_id_for_snapshot;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;

const SCENARIO_ROOT: &str = "tests/unit/scenarios/code_search_merkle_memory_warm_path";
const TURSO_SCENARIO_ROOT: &str =
    "tests/unit/scenarios/code_search_turso_resident_session_warm_path";
static PERFORMANCE_GATE: Mutex<()> = Mutex::new(());

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
    let _performance_gate = PERFORMANCE_GATE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
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
        "generation_publication_boundary_ms = 500",
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
    let generation_publication_boundary = std::time::Duration::from_millis(benchmark_u64(
        &benchmark,
        "generation_publication_boundary_ms",
    ));
    assert!(
        sample_count >= 128,
        "strong gate requires at least 128 samples"
    );

    let root = temp_root();
    let client_dir = root.join("client");
    let fixture =
        agent_semantic_client_db::fixture::SourceIndexFixture::for_client_dir(&client_dir);
    let project_root = root.join("project");
    fs::create_dir_all(&project_root).expect("create project root");
    let fixture_source = b"pub fn resident_needle() {}\n".to_vec();
    let fixture_owner_path = "src/lib.rs".to_owned();
    let fixture_source_path = project_root.join(&fixture_owner_path);
    tokio::fs::create_dir_all(
        fixture_source_path
            .parent()
            .expect("fixture source path has a parent"),
    )
    .await
    .expect("create fixture source directory");
    tokio::fs::write(&fixture_source_path, &fixture_source)
        .await
        .expect("write fixture source bytes");
    let fixture_sha256 = format!(
        "{:x}",
        <sha2::Sha256 as sha2::Digest>::digest(&fixture_source)
    );
    let workspace_snapshot =
        agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes([(
            fixture_owner_path.clone(),
            blake3::hash(&fixture_source).to_hex().to_string(),
        )]);
    let source_snapshot = workspace_snapshot.evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        "22".repeat(32),
    );
    let source_blobs =
        agent_semantic_client_db::ClientDbSourceIndexSourceBlobs::from_normalized([(
            agent_semantic_client_db::ClientDbSourceIndexPath::try_from(
                fixture_owner_path.as_str(),
            )
            .expect("normalize fixture owner path"),
            fixture_source.clone(),
        )]);
    let rust_language_id = LanguageId::from("rust");
    let import = build_source_index_import(ClientDbSourceIndexImportRequest {
        generation_id: source_snapshot.root_digest.clone().into(),
        project_root: project_root.clone(),
        schema_id: SemanticSchemaId::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_ID),
        schema_version: SemanticSchemaVersion::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION),
        selector_source: "asp-rust".into(),
        file_hashes: vec![ClientCacheFileHash {
            path: fixture_owner_path.clone(),
            sha256: fixture_sha256,
            byte_len: fixture_source.len() as u64,
            mtime_ms: 1,
        }],
        files: vec![ClientDbSourceIndexImportFile {
            relations: Vec::new(),
            relative_path: fixture_owner_path.clone(),
            language_id: rust_language_id.clone(),
            provider_id: ProviderId::from("asp-rust"),
            text: String::from_utf8(fixture_source.clone())
                .expect("fixture source bytes are UTF-8"),
            selectors: Vec::new(),
        }],
        source_blobs: source_blobs.clone(),
    })
    .expect("build resident Turso source-index import");
    let owner_snapshot =
        agent_semantic_client_db::runtime_server_workspace::WorkspaceOwnerSnapshot {
            authority: None,
            owner_path: fixture_owner_path.clone(),
            bytes: fixture_source.clone(),
            content_digest: format!("blake3-256:{}", blake3::hash(&fixture_source).to_hex()),
            native_syntax_diagnostic: None,
            selectors: Vec::new(),
        };
    fixture
        .commit_source_index_generation(
            ClientDbSourceIndexRefreshRequest {
                import: import.clone(),
                file_count: 1,
                source_snapshot: source_snapshot.clone(),
            },
            &source_blobs,
        )
        .expect("materialize resident Turso source index");
    let workspace_identity = "workspace-code-search-performance";
    let runtime_registry = std::sync::Arc::new(
        agent_semantic_client_db::runtime_server_workspace::RuntimeServerWorkspaceRegistry::new(
            client_dir.join("runtime"),
        )
        .expect("create resident workspace registry"),
    );
    let second_workspace_owner_snapshot = owner_snapshot.clone();
    let canonical_materialization =
        agent_semantic_client_db::runtime_server_workspace::WorkspaceCanonicalMaterialization::new(
            workspace_identity,
            source_snapshot.clone(),
            &import,
            [1, 0],
            vec![owner_snapshot],
            Vec::new(),
        )
        .expect("assemble canonical workspace materialization");
    let second_workspace_identity = "workspace-code-search-performance-second";
    let second_workspace_materialization =
        agent_semantic_client_db::runtime_server_workspace::WorkspaceCanonicalMaterialization::new(
            second_workspace_identity,
            source_snapshot.clone(),
            &import,
            [1, 0],
            vec![second_workspace_owner_snapshot],
            Vec::new(),
        )
        .expect("assemble second canonical workspace materialization");
    let canonical_runtime_project_root =
        std::path::PathBuf::from(&canonical_materialization.project_root);
    let canonical_materialization_replay = canonical_materialization
        .clone()
        .into_validated(workspace_identity)
        .expect("validate replay canonical materialization before timing");
    let canonical_materialization = canonical_materialization
        .into_validated(workspace_identity)
        .expect("validate canonical materialization before timing");
    let second_workspace_materialization = second_workspace_materialization
        .into_validated(second_workspace_identity)
        .expect("validate second canonical materialization before timing");
    runtime_registry
        .prepare_resident_workspace_scope(workspace_identity, &canonical_runtime_project_root)
        .await
        .expect("prepare first resident workspace scope before query timing");
    runtime_registry
        .prepare_resident_workspace_scope(
            second_workspace_identity,
            &canonical_runtime_project_root,
        )
        .await
        .expect("prepare second resident workspace scope before query timing");
    let restore_started = tokio::time::Instant::now();
    let recovery_receipt = runtime_registry
        .ensure_canonical_generation(
            "code-search-performance-restore",
            workspace_identity,
            canonical_materialization,
        )
        .await
        .expect("restore canonical generation into resident memory and mmap");
    let restore_elapsed = restore_started.elapsed();
    let replay_started = tokio::time::Instant::now();
    let replay_receipt = runtime_registry
        .ensure_canonical_generation(
            "code-search-performance-replay",
            workspace_identity,
            canonical_materialization_replay,
        )
        .await
        .expect("reuse canonical resident generation");
    let replay_elapsed = replay_started.elapsed();
    // A resident replay must not be modeled by cloning and resubmitting the
    // canonical projection payload. That benchmark shape was itself the
    // amplification bug. Keep these compatibility fields tied to the single
    // replay until the registry exposes an identity-only resident handle.
    let replay_pressure_p99 = replay_elapsed;
    let replay_pressure_max = replay_elapsed;
    let second_workspace_started = tokio::time::Instant::now();
    let second_workspace_receipt = runtime_registry
        .ensure_canonical_generation(
            "code-search-performance-second-workspace",
            second_workspace_identity,
            second_workspace_materialization,
        )
        .await
        .expect("restore second workspace canonical generation");
    let second_workspace_elapsed = second_workspace_started.elapsed();
    for (workspace_identity, expected_digest) in [
        (
            workspace_identity,
            recovery_receipt.generation_digest.as_str(),
        ),
        (
            second_workspace_identity,
            second_workspace_receipt.generation_digest.as_str(),
        ),
    ] {
        let mut durability = runtime_registry
            .subscribe_generation_durability(workspace_identity, &canonical_runtime_project_root)
            .expect("subscribe workspace durability receipt");
        loop {
            if let Some(receipt) = durability.borrow().clone()
                && receipt.generation_digest == expected_digest
                && receipt.state
                    == agent_semantic_client_db::runtime_server_workspace::WorkspaceGenerationDurabilityState::DurableReady
            {
                receipt.validate().expect("validate durable terminal receipt");
                break;
            }
            durability
                .changed()
                .await
                .expect("workspace durability lane remains available");
        }
    }
    let mut cold_publication_samples = vec![restore_elapsed, second_workspace_elapsed];
    let mut resident_publication_service_samples = vec![
        std::time::Duration::from_micros(recovery_receipt.resident_publication_elapsed_micros),
        std::time::Duration::from_micros(
            second_workspace_receipt.resident_publication_elapsed_micros,
        ),
    ];
    for sample_index in 0..64usize {
        let sample_workspace_identity =
            format!("workspace-code-search-cold-pressure-{sample_index}");
        let sample_owner =
            agent_semantic_client_db::runtime_server_workspace::WorkspaceOwnerSnapshot {
                authority: None,
                owner_path: fixture_owner_path.clone(),
                bytes: fixture_source.clone(),
                content_digest: format!("blake3-256:{}", blake3::hash(&fixture_source).to_hex()),
                native_syntax_diagnostic: None,
                selectors: Vec::new(),
            };
        let sample_materialization =
            agent_semantic_client_db::runtime_server_workspace::WorkspaceCanonicalMaterialization::new(
                sample_workspace_identity.clone(),
                source_snapshot.clone(),
                &import,
                [1, 0],
                vec![sample_owner],
                Vec::new(),
            )
            .expect("assemble cold pressure canonical materialization")
            .into_validated(&sample_workspace_identity)
            .expect("validate cold pressure canonical materialization");
        runtime_registry
            .prepare_resident_workspace_scope(
                &sample_workspace_identity,
                &canonical_runtime_project_root,
            )
            .await
            .expect("prepare cold pressure resident workspace scope");
        let sample_started = tokio::time::Instant::now();
        let sample_receipt = runtime_registry
            .ensure_canonical_generation(
                format!("code-search-cold-pressure-{sample_index}"),
                sample_workspace_identity.clone(),
                sample_materialization,
            )
            .await
            .expect("publish cold pressure resident generation");
        cold_publication_samples.push(sample_started.elapsed());
        resident_publication_service_samples.push(std::time::Duration::from_micros(
            sample_receipt.resident_publication_elapsed_micros,
        ));
        let mut durability = runtime_registry
            .subscribe_generation_durability(
                &sample_workspace_identity,
                &canonical_runtime_project_root,
            )
            .expect("subscribe cold pressure durability receipt");
        loop {
            if let Some(receipt) = durability.borrow().clone()
                && receipt.generation_digest == sample_receipt.generation_digest
                && receipt.state
                    == agent_semantic_client_db::runtime_server_workspace::WorkspaceGenerationDurabilityState::DurableReady
            {
                receipt
                    .validate()
                    .expect("validate cold pressure durable receipt");
                break;
            }
            durability
                .changed()
                .await
                .expect("cold pressure durability lane remains available");
        }
    }
    cold_publication_samples.sort_unstable();
    let cold_publication_p95 = cold_publication_samples
        [(cold_publication_samples.len() * 95 / 100).min(cold_publication_samples.len() - 1)];
    let cold_publication_max = *cold_publication_samples
        .last()
        .expect("cold publication samples");
    resident_publication_service_samples.sort_unstable();
    let resident_publication_service_p95 = resident_publication_service_samples
        [(resident_publication_service_samples.len() * 95 / 100)
            .min(resident_publication_service_samples.len() - 1)];
    let resident_publication_service_max = *resident_publication_service_samples
        .last()
        .expect("resident publication service samples");
    eprintln!(
        "code-search-tier=canonical-restore elapsed={restore_elapsed:?} replayElapsed={replay_elapsed:?} secondWorkspaceElapsed={second_workspace_elapsed:?} coldPublicationSamples={} coldPublicationP95={cold_publication_p95:?} coldPublicationMax={cold_publication_max:?} residentPublicationServiceP95={resident_publication_service_p95:?} residentPublicationServiceMax={resident_publication_service_max:?} replayPressureSamples={} replayPressureP99={replay_pressure_p99:?} replayPressureMax={replay_pressure_max:?} rootDepth=1,0 receipt={recovery_receipt:?} replayReceipt={replay_receipt:?} secondWorkspaceReceipt={second_workspace_receipt:?}",
        cold_publication_samples.len(),
        1,
    );
    assert!(
        cold_publication_p95 < generation_publication_boundary
            && cold_publication_max < generation_publication_boundary
            && resident_publication_service_max < generation_publication_boundary
            && replay_pressure_max < generation_publication_boundary,
        "canonical publication exceeded the configured durable Ready boundary: boundary={generation_publication_boundary:?} coldP95={cold_publication_p95:?} coldMax={cold_publication_max:?} readyServiceP95={resident_publication_service_p95:?} readyServiceMax={resident_publication_service_max:?} replayMax={replay_pressure_max:?}"
    );
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
    let worker_threads = std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(1);
    let mut concurrent_samples = tokio::task::spawn_blocking(move || {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(worker_threads)
            .enable_all()
            .build()
            .expect("build adaptive resident query pressure runtime");
        runtime.block_on(async move {
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
            let mut samples = Vec::with_capacity(concurrent_task_count * lookups_per_task);
            while let Some(task_samples) = tasks.join_next().await {
                samples.extend(task_samples.expect("resident code-search task"));
            }
            samples
        })
    })
    .await
    .expect("adaptive resident query pressure task");
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
    let _performance_gate = PERFORMANCE_GATE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
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
        relations: Vec::new(),
        generation_id: client_db_source_index_generation_id_for_snapshot(&source_snapshot),
        project_root: project_root.clone(),
        schema_id: CLIENT_DB_SOURCE_INDEX_SCHEMA_ID.into(),
        schema_version: CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION.into(),
        file_hashes: Vec::new(),
        owners: vec![ClientDbSourceIndexOwner {
            owner_path: "src/lib.rs".into(),
            language_id: Some("rust".into()),
            provider_id: Some("asp-rust".into()),
            source_kind: "file".into(),
            line_count: Some(1),
            query_keys: vec!["needle".into()],
        }],
        selectors: Vec::new(),
        source_blobs: Default::default(),
    };
    let expected_artifact_digest =
        agent_semantic_search_projection::source_index_artifact_digest(&source_snapshot);
    let rust_language_id = LanguageId::from("rust");

    let warmup = ClientDbEngine::lookup_source_index_from_client_dir(
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
    .expect("prewarm Merkle-qualified memory code search");
    assert_eq!(warmup.state, ClientDbSourceIndexLookupState::Hit);

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
