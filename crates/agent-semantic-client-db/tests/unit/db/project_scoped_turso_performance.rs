//! Explicit, ignored system scenarios for the project-scoped Turso cutover.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use agent_semantic_client_core::{CacheExportMethod, ClientCacheManifest, LanguageId, ProviderId};
use agent_semantic_client_db::ClientDbEngine;
use serde_json::json;

const LANGUAGE_ID: &str = "rust";
const PROVIDER_ID: &str = "rs-harness";
const EXPORT_METHOD: &str = "search/prime";

#[test]
#[ignore = "project-scoped schema v2 cutover correctness gate"]
fn s0_partition_correctness_cutover_gate() {
    let root = scenario_root("partition-correctness");
    let client_dir = project_client_dir(&root);
    let workspace_a = root.join("workspaces/workspace-a");
    let workspace_b = root.join("workspaces/workspace-b");
    fs::create_dir_all(workspace_a.join("src")).expect("create workspace A");
    fs::create_dir_all(workspace_b.join("src")).expect("create workspace B");

    let generation_id = "shared-generation";
    let mut write_session = ClientDbEngine::open_write_session_client_dir(&client_dir)
        .expect("open shared project DB write session");
    write_session
        .import_manifest(&scenario_manifest(
            &client_dir,
            &workspace_a,
            [(generation_id, "workspace-a")],
        ))
        .expect("import workspace A generation");
    write_session
        .import_manifest(&scenario_manifest(
            &client_dir,
            &workspace_b,
            [(generation_id, "workspace-b")],
        ))
        .expect("import workspace B generation");

    let read_session = ClientDbEngine::open_read_session_client_dir(&client_dir)
        .expect("open shared project DB read session")
        .expect("shared project DB exists");
    let workspace_a_hit = lookup_generation(&read_session, &workspace_a, "fnv64:workspace-a")
        .expect("lookup workspace A");
    let workspace_b_hit = lookup_generation(&read_session, &workspace_b, "fnv64:workspace-b")
        .expect("lookup workspace B");

    let receipt = json!({
        "schemaId": "agent.semantic-protocols.project-turso-performance-receipt",
        "schemaVersion": "1",
        "scenario": "S0",
        "dbPath": client_dir.join("facts.turso"),
        "sameGenerationId": generation_id,
        "workspaceAHit": workspace_a_hit.is_some(),
        "workspaceBHit": workspace_b_hit.is_some(),
        "partitionIsolation": workspace_a_hit.is_some() && workspace_b_hit.is_some(),
    });
    println!("{receipt}");

    cleanup_scenario_root(&root);
    assert!(
        workspace_a_hit.is_some(),
        "project DB overwrote workspace A when workspace B reused the generation id"
    );
    assert!(
        workspace_b_hit.is_some(),
        "project DB did not retain workspace B generation"
    );
}

#[test]
#[ignore = "explicit project-scoped Turso performance scenario"]
fn s1_cold_bootstrap_first_import_receipt() {
    let sample_count = scenario_usize("ASP_TURSO_S1_SAMPLES", 16);
    let max_p95_us = scenario_u64("ASP_TURSO_S1_MAX_P95_US", 2_000_000);
    let mut samples_us = Vec::with_capacity(sample_count);

    for sample_index in 0..sample_count {
        let root = scenario_root(&format!("cold-bootstrap-{sample_index}"));
        let client_dir = project_client_dir(&root);
        let workspace = root.join("workspaces/workspace-a");
        fs::create_dir_all(workspace.join("src")).expect("create cold workspace");
        let manifest = scenario_manifest(
            &client_dir,
            &workspace,
            [("cold-generation", "cold-bootstrap")],
        );

        let started = Instant::now();
        let mut write_session = ClientDbEngine::open_write_session_client_dir(&client_dir)
            .expect("bootstrap cold project DB");
        write_session
            .import_manifest(&manifest)
            .expect("import first cold manifest");
        samples_us.push(elapsed_us(started));
        cleanup_scenario_root(&root);
    }

    let metrics = latency_metrics(&samples_us);
    let receipt = json!({
        "schemaId": "agent.semantic-protocols.project-turso-performance-receipt",
        "schemaVersion": "1",
        "scenario": "S1",
        "sampleCount": sample_count,
        "p50Us": metrics.p50_us,
        "p95Us": metrics.p95_us,
        "p99Us": metrics.p99_us,
        "maxP95Us": max_p95_us,
    });
    println!("{receipt}");

    assert!(
        metrics.p95_us <= max_p95_us,
        "cold bootstrap p95 {}us exceeded configured gate {}us",
        metrics.p95_us,
        max_p95_us
    );
}

#[test]
fn active_generation_pointer_latest_import_wins() {
    let root = scenario_root("active-pointer-latest-wins");
    let client_dir = project_client_dir(&root);
    let workspace = root.join("workspaces/workspace-a");
    fs::create_dir_all(workspace.join("src")).expect("create pointer workspace");
    let manifest = scenario_manifest(
        &client_dir,
        &workspace,
        [
            ("pointer-generation-old", "shared-request"),
            ("pointer-generation-new", "shared-request"),
        ],
    );
    let mut writer = ClientDbEngine::open_write_session_client_dir(&client_dir)
        .expect("open active-pointer writer");
    writer
        .import_manifest(&manifest)
        .expect("import competing active generations");
    let reader = ClientDbEngine::open_read_session_client_dir(&client_dir)
        .expect("open active-pointer reader")
        .expect("active-pointer DB exists");
    let hit = lookup_generation(&reader, &workspace, "fnv64:shared-request")
        .expect("lookup active generation")
        .expect("active generation exists");

    cleanup_scenario_root(&root);
    assert_eq!(
        hit.file_hashes.first().map(|file_hash| file_hash.mtime_ms),
        Some(1),
        "active pointer did not select the last generation imported in the transaction"
    );
}

#[test]
#[ignore = "explicit project-scoped Turso performance scenario"]
fn s2_warm_lookup_receipt() {
    let generation_count = scenario_usize("ASP_TURSO_S2_GENERATIONS", 1_000);
    let lookup_count = scenario_usize("ASP_TURSO_S2_LOOKUPS", 1_000);
    let max_p95_us = scenario_u64("ASP_TURSO_S2_MAX_P95_US", 10_000);
    let default_max_write_wall_ms = ((generation_count as u64)
        .saturating_mul(500)
        .saturating_add(999))
        / 1_000;
    let max_write_wall_ms =
        scenario_u64("ASP_TURSO_S2_MAX_WRITE_WALL_MS", default_max_write_wall_ms);
    let root = scenario_root("warm-lookup");
    let client_dir = project_client_dir(&root);
    let workspace = root.join("workspaces/workspace-a");
    fs::create_dir_all(workspace.join("src")).expect("create warm workspace");

    let generations = (0..generation_count)
        .map(|index| (format!("warm-generation-{index}"), format!("warm-{index}")))
        .collect::<Vec<_>>();
    let manifest = scenario_manifest(
        &client_dir,
        &workspace,
        generations
            .iter()
            .map(|(generation, label)| (generation.as_str(), label.as_str())),
    );
    let write_started = Instant::now();
    let mut write_session =
        ClientDbEngine::open_write_session_client_dir(&client_dir).expect("open warm project DB");
    write_session
        .import_manifest(&manifest)
        .expect("seed warm generations");
    let write_elapsed_us = elapsed_us(write_started);
    let write_wall_ms = write_elapsed_us.saturating_add(999) / 1_000;
    let write_operations_per_second = if write_elapsed_us == 0 {
        0
    } else {
        ((generation_count as u128 * 1_000_000) / u128::from(write_elapsed_us)) as u64
    };

    let read_session = ClientDbEngine::open_read_session_client_dir(&client_dir)
        .expect("open warm read session")
        .expect("warm project DB exists");
    let mut samples_us = Vec::with_capacity(lookup_count);
    let scenario_started = Instant::now();
    for lookup_index in 0..lookup_count {
        let generation_index = lookup_index % generation_count;
        let request_fingerprint = format!("fnv64:warm-{generation_index}");
        let started = Instant::now();
        let hit = lookup_generation(&read_session, &workspace, &request_fingerprint)
            .expect("run warm generation lookup");
        samples_us.push(elapsed_us(started));
        assert!(
            hit.is_some(),
            "warm lookup missed generation {generation_index}"
        );
    }
    let scenario_elapsed_us = elapsed_us(scenario_started);
    let metrics = latency_metrics(&samples_us);
    let operations_per_second = if scenario_elapsed_us == 0 {
        0
    } else {
        ((lookup_count as u128 * 1_000_000) / u128::from(scenario_elapsed_us)) as u64
    };
    let receipt = json!({
        "schemaId": "agent.semantic-protocols.project-turso-performance-receipt",
        "schemaVersion": "1",
        "scenario": "S2",
        "lookupPath": "active-generation-pointer-v1",
        "dbPath": ClientDbEngine::turso_path_for_client_dir(&client_dir).display().to_string(),
        "generationCount": generation_count,
        "lookupCount": lookup_count,
        "writeWallMs": write_wall_ms,
        "writeOperationsPerSecond": write_operations_per_second,
        "maxWriteWallMs": max_write_wall_ms,
        "p50Us": metrics.p50_us,
        "p95Us": metrics.p95_us,
        "p99Us": metrics.p99_us,
        "operationsPerSecond": operations_per_second,
        "maxP95Us": max_p95_us,
    });
    println!("{receipt}");

    cleanup_scenario_root(&root);
    assert!(
        write_wall_ms <= max_write_wall_ms,
        "warm generation write wall {}ms exceeded configured gate {}ms",
        write_wall_ms,
        max_write_wall_ms
    );
    assert!(
        metrics.p95_us <= max_p95_us,
        "warm lookup p95 {}us exceeded configured gate {}us",
        metrics.p95_us,
        max_p95_us
    );
}

#[test]
#[ignore = "explicit project-scoped Turso performance scenario"]
fn s5_turso_0_7_atomic_promotion_does_not_open_sqlite() {
    let root = scenario_root("turso-0-7-full-cutover");
    let workspace = root.join("workspaces/workspace-a");
    let legacy_client_dir = workspace.join("live/client");
    let manifest = scenario_manifest(
        &legacy_client_dir,
        &workspace,
        [("cutover-generation", "cutover")],
    );
    let mut legacy_writer = ClientDbEngine::open_write_session_client_dir(&legacy_client_dir)
        .expect("create legacy client DB fixture");
    legacy_writer
        .import_manifest(&manifest)
        .expect("seed legacy cache manifest");
    drop(legacy_writer);
    let legacy_db_path = ClientDbEngine::turso_path_for_client_dir(&legacy_client_dir);
    let legacy_search_projection_path = legacy_client_dir.join("search-projection.turso");
    let legacy_format_receipt_path = legacy_db_path.with_file_name("facts.turso.format.v1.json");
    let legacy_search_format_receipt_path =
        legacy_db_path.with_file_name("search-projection.turso.format.v1.json");
    fs::remove_file(&legacy_format_receipt_path).expect("remove legacy facts format receipt");
    fs::remove_file(&legacy_search_format_receipt_path)
        .expect("remove legacy search-projection format receipt");
    let legacy_bytes = fs::read(&legacy_db_path).expect("snapshot preserved legacy DB");
    let legacy_search_bytes =
        fs::read(&legacy_search_projection_path).expect("snapshot preserved legacy search DB");
    let legacy_runtime_error =
        match ClientDbEngine::open_read_session_client_dir(&legacy_client_dir) {
            Ok(_) => panic!("normal runtime must reject an unreceipted legacy DB"),
            Err(error) => error,
        };

    let project_client_dir = project_client_dir(&root);
    let incomplete_target = root.join("projects/by-id/project-performance/live/incomplete-client");
    let incomplete_error =
        ClientDbEngine::migrate_project_client_dir_to_turso_0_7(&incomplete_target, |writer| {
            writer
                .import_manifest(&manifest)
                .map_err(|error| format!("replay incomplete v1 cache manifest: {error}"))?;
            Ok(
                agent_semantic_client_db::engine::ClientDbTurso07ReplayCoverage {
                    cache_manifest: agent_semantic_client_db::engine::
                        ClientDbTurso07ReplayFamilyReceipt::matched(
                            1,
                            "fnv64:cutover-generation",
                        ),
                    syntax_query: agent_semantic_client_db::engine::
                        ClientDbTurso07ReplayFamilyReceipt::compare(
                            1,
                            0,
                            "fnv64:legacy-syntax",
                            "fnv64:missing",
                        ),
                    source_index: agent_semantic_client_db::engine::
                        ClientDbTurso07ReplayFamilyReceipt::enumerated_empty(),
                    structural_index: agent_semantic_client_db::engine::
                        ClientDbTurso07ReplayFamilyReceipt::enumerated_empty(),
                    provider_command: agent_semantic_client_db::engine::
                        ClientDbTurso07ReplayFamilyReceipt::enumerated_empty(),
                    artifact_event: agent_semantic_client_db::engine::
                        ClientDbTurso07ReplayFamilyReceipt::enumerated_empty(),
                    artifact_pointer: agent_semantic_client_db::engine::
                        ClientDbTurso07ReplayFamilyReceipt::enumerated_empty(),
                    retired_derived_projection: agent_semantic_client_db::engine::
                        ClientDbTurso07RetiredDerivedReceipt::enumerated_empty(),
                },
            )
        })
        .expect_err("incomplete replayer coverage must not promote");
    assert!(incomplete_error.contains("unverified v1 replayers"));
    assert!(!incomplete_target.exists());

    let migration = ClientDbEngine::migrate_legacy_project_client_dir_to_turso_0_7(
        &legacy_client_dir,
        &project_client_dir,
    )
    .expect("fully replay and atomically promote legacy DB into Turso 0.7");

    let reader = ClientDbEngine::open_read_session_client_dir(&project_client_dir)
        .expect("open promoted Turso 0.7 DB")
        .expect("promoted Turso 0.7 DB exists");
    let hit = lookup_generation(&reader, &workspace, "fnv64:cutover")
        .expect("query replayed Turso 0.7 generation");
    drop(reader);

    let target_db_path = ClientDbEngine::turso_path_for_client_dir(&project_client_dir);
    let format_receipt_path = target_db_path.with_file_name("facts.turso.format.v1.json");
    let format_receipt: serde_json::Value = serde_json::from_slice(
        &fs::read(&format_receipt_path).expect("read Turso 0.7 format receipt"),
    )
    .expect("decode Turso 0.7 format receipt");
    let search_projection_format_receipt: serde_json::Value = serde_json::from_slice(
        &fs::read(&migration.search_projection_format_receipt_path)
            .expect("read search-projection Turso 0.7 format receipt"),
    )
    .expect("decode search-projection Turso 0.7 format receipt");
    let migration_receipt: serde_json::Value = serde_json::from_slice(
        &fs::read(&migration.migration_receipt_path).expect("read Turso 0.7 migration receipt"),
    )
    .expect("decode Turso 0.7 migration receipt");
    let preserved_legacy_bytes = fs::read(&legacy_db_path).expect("read preserved legacy input");
    let preserved_legacy_search_bytes =
        fs::read(&legacy_search_projection_path).expect("read preserved legacy search input");
    let receipt = json!({
        "schemaId": "agent.semantic-protocols.project-turso-performance-receipt",
        "schemaVersion": "1",
        "scenario": "S5",
        "migrationPhase": "atomic-project-promotion",
        "physicalFormat": "turso-0.7-native",
        "logicalSchemaVersion": 1,
        "sourcePreserved":
            preserved_legacy_bytes == legacy_bytes
                && preserved_legacy_search_bytes == legacy_search_bytes,
        "normalRuntimeRejectedLegacy":
            legacy_runtime_error.contains("full staging migration is required"),
        "incompleteCoverageBlocked": incomplete_error.contains("unverified v1 replayers"),
        "targetHasTurso07FormatReceipt":
            format_receipt["physicalFormat"] == "turso-0.7-native"
                && format_receipt["tursoVersion"] == "0.7"
                && search_projection_format_receipt["physicalFormat"] == "turso-0.7-native"
                && search_projection_format_receipt["tursoVersion"] == "0.7",
        "migrationReceiptVerified":
            migration_receipt["families"]["cacheManifest"]["status"] == "verified"
                && migration_receipt["families"]["cacheManifest"]["sourceRecordCount"] == 1
                && migration_receipt["families"]["cacheManifest"]["targetRecordCount"] == 1
                && migration_receipt["families"]["syntaxQuery"]["status"] == "verified"
                && migration_receipt["families"]["syntaxQuery"]["sourceRecordCount"] == 0,
        "verifiedGenerationCount": usize::from(hit.is_some()),
        "atomicPromotion": migration.target_client_dir == project_client_dir,
        "targetDbPath": target_db_path.display().to_string(),
    });
    println!("{receipt}");

    cleanup_scenario_root(&root);
    assert_eq!(preserved_legacy_bytes, legacy_bytes);
    assert_eq!(preserved_legacy_search_bytes, legacy_search_bytes);
    assert_eq!(format_receipt["physicalFormat"], "turso-0.7-native");
    assert_eq!(format_receipt["tursoVersion"], "0.7");
    assert_eq!(
        search_projection_format_receipt["physicalFormat"],
        "turso-0.7-native"
    );
    assert_eq!(
        migration_receipt["families"]["cacheManifest"]["status"],
        "verified"
    );
    assert!(
        hit.is_some(),
        "Turso 0.7 cutover lost the replayed generation"
    );
}

#[test]
#[ignore = "explicit real-scale Turso 0.7 migration performance scenario"]
fn s6_real_scale_turso_0_7_migration_receipt() {
    let source_client_dir = std::env::var_os("ASP_TURSO_S6_SOURCE_CLIENT_DIR")
        .map(PathBuf::from)
        .expect("ASP_TURSO_S6_SOURCE_CLIENT_DIR must select a preserved legacy client dir");
    let max_wall_ms = scenario_u64("ASP_TURSO_S6_MAX_WALL_MS", 10_000);
    let root = scenario_root("real-scale-turso-0-7-migration");
    let target_client_dir = root.join("live/client");
    let started = Instant::now();
    let migration = ClientDbEngine::migrate_legacy_project_client_dir_to_turso_0_7(
        &source_client_dir,
        &target_client_dir,
    )
    .expect("migrate preserved real-scale client DB");
    let wall_ms = elapsed_us(started).saturating_add(999) / 1_000;
    let receipt = json!({
        "schemaId": "agent.semantic-protocols.project-turso-performance-receipt",
        "schemaVersion": "1",
        "scenario": "S6",
        "sourceClientDir": source_client_dir,
        "targetClientDir": target_client_dir,
        "wallMs": wall_ms,
        "maxWallMs": max_wall_ms,
        "cacheManifestRows": migration.replay_coverage.cache_manifest.target_record_count,
        "sourceIndexRows": migration.replay_coverage.source_index.target_record_count,
        "artifactEventRows": migration.replay_coverage.artifact_event.target_record_count,
        "retiredDerivedRows":
            migration.replay_coverage.retired_derived_projection.source_record_count,
    });
    println!("{receipt}");
    cleanup_scenario_root(&root);
    assert!(
        wall_ms <= max_wall_ms,
        "real-scale Turso 0.7 migration wall {wall_ms}ms exceeded {max_wall_ms}ms"
    );
}

#[test]
#[ignore = "explicit project-scoped Turso performance scenario"]
fn s3_single_writer_many_readers_receipt() {
    let reader_count = scenario_usize("ASP_TURSO_S3_READERS", 8);
    let reads_per_reader = scenario_usize("ASP_TURSO_S3_READS_PER_READER", 64);
    let writer_generation_count = scenario_usize("ASP_TURSO_S3_WRITES", 32);
    let max_p95_us = scenario_u64("ASP_TURSO_S3_MAX_READ_P95_US", 500_000);
    let root = scenario_root("single-writer-many-readers");
    let client_dir = Arc::new(project_client_dir(&root));
    let workspace = Arc::new(root.join("workspaces/workspace-a"));
    fs::create_dir_all(workspace.join("src")).expect("create concurrent workspace");
    ClientDbEngine::open_write_session_client_dir(client_dir.as_path())
        .expect("bootstrap concurrent project DB");

    let barrier = Arc::new(Barrier::new(reader_count + 1));
    let mut readers = Vec::with_capacity(reader_count);
    for _ in 0..reader_count {
        let barrier = Arc::clone(&barrier);
        let client_dir = Arc::clone(&client_dir);
        let workspace = Arc::clone(&workspace);
        readers.push(thread::spawn(move || {
            let read_session = ClientDbEngine::open_read_session_client_dir(client_dir.as_path())
                .expect("open concurrent read session")
                .expect("concurrent project DB exists");
            let mut samples_us = Vec::with_capacity(reads_per_reader);
            let mut error_count = 0u64;
            let mut lock_error_count = 0u64;
            barrier.wait();
            for read_index in 0..reads_per_reader {
                let generation_index = read_index % writer_generation_count;
                let request_fingerprint = format!("fnv64:concurrent-{generation_index}");
                let started = Instant::now();
                match lookup_generation(&read_session, &workspace, &request_fingerprint) {
                    Ok(_) => samples_us.push(elapsed_us(started)),
                    Err(error) => {
                        samples_us.push(elapsed_us(started));
                        error_count += 1;
                        let error = error.to_ascii_lowercase();
                        if error.contains("busy") || error.contains("locked") {
                            lock_error_count += 1;
                        }
                    }
                }
            }
            (samples_us, error_count, lock_error_count)
        }));
    }

    let scenario_started = Instant::now();
    barrier.wait();
    let mut write_session = ClientDbEngine::open_write_session_client_dir(client_dir.as_path())
        .expect("open concurrent write session");
    let mut writer_error_count = 0u64;
    let mut writer_lock_error_count = 0u64;
    let mut writer_error = None;
    for generation_index in 0..writer_generation_count {
        let generation_id = format!("concurrent-generation-{generation_index}");
        let label = format!("concurrent-{generation_index}");
        if let Err(error) = write_session.import_manifest(&scenario_manifest(
            client_dir.as_path(),
            workspace.as_path(),
            [(generation_id.as_str(), label.as_str())],
        )) {
            writer_error_count += 1;
            let normalized_error = error.to_ascii_lowercase();
            if normalized_error.contains("busy") || normalized_error.contains("locked") {
                writer_lock_error_count += 1;
            }
            writer_error = Some(error);
            break;
        }
    }

    let mut samples_us = Vec::with_capacity(reader_count * reads_per_reader);
    let mut error_count = 0u64;
    let mut lock_error_count = 0u64;
    for reader in readers {
        let (reader_samples, reader_errors, reader_lock_errors) =
            reader.join().expect("join concurrent reader");
        samples_us.extend(reader_samples);
        error_count += reader_errors;
        lock_error_count += reader_lock_errors;
    }
    let scenario_elapsed_us = elapsed_us(scenario_started);
    let metrics = latency_metrics(&samples_us);
    let read_count = reader_count * reads_per_reader;
    let operations_per_second = if scenario_elapsed_us == 0 {
        0
    } else {
        (((read_count + writer_generation_count) as u128 * 1_000_000)
            / u128::from(scenario_elapsed_us)) as u64
    };

    let final_fingerprint = format!(
        "fnv64:concurrent-{}",
        writer_generation_count.saturating_sub(1)
    );
    let final_read_session = ClientDbEngine::open_read_session_client_dir(client_dir.as_path())
        .expect("open final concurrent read session")
        .expect("concurrent project DB exists");
    let final_hit = lookup_generation(&final_read_session, workspace.as_path(), &final_fingerprint)
        .expect("lookup final concurrent generation");
    let receipt = json!({
        "schemaId": "agent.semantic-protocols.project-turso-performance-receipt",
        "schemaVersion": "1",
        "scenario": "S3",
        "readerCount": reader_count,
        "readsPerReader": reads_per_reader,
        "writerGenerationCount": writer_generation_count,
        "readCount": read_count,
        "p50Us": metrics.p50_us,
        "p95Us": metrics.p95_us,
        "p99Us": metrics.p99_us,
        "operationsPerSecond": operations_per_second,
        "errorCount": error_count,
        "lockErrorCount": lock_error_count,
        "writerErrorCount": writer_error_count,
        "writerLockErrorCount": writer_lock_error_count,
        "writerError": writer_error,
        "finalGenerationPresent": final_hit.is_some(),
        "maxReadP95Us": max_p95_us,
    });
    println!("{receipt}");

    cleanup_scenario_root(&root);
    assert_eq!(error_count, 0, "concurrent reads returned errors");
    assert_eq!(lock_error_count, 0, "concurrent reads returned lock errors");
    assert_eq!(writer_error_count, 0, "concurrent writer returned errors");
    assert_eq!(
        writer_lock_error_count, 0,
        "concurrent writer returned lock errors"
    );
    assert!(final_hit.is_some(), "final committed generation is missing");
    assert!(
        metrics.p95_us <= max_p95_us,
        "concurrent read p95 {}us exceeded configured gate {}us",
        metrics.p95_us,
        max_p95_us
    );
}

fn scenario_manifest<'a>(
    client_dir: &Path,
    project_root: &Path,
    generations: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> ClientCacheManifest {
    let generations = generations
        .into_iter()
        .enumerate()
        .map(|(index, (generation_id, label))| {
            json!({
                "generationId": generation_id,
                "languageId": LANGUAGE_ID,
                "providerId": PROVIDER_ID,
                "providerVersion": "0.1.0",
                "exportMethod": EXPORT_METHOD,
                "projectRoot": project_root.display().to_string(),
                "packageRoot": ".",
                "schemaIds": ["agent.semantic-protocols.semantic-search-packet"],
                "cacheStatus": "hit",
                "rawSourceStored": false,
                "requestFingerprint": format!("fnv64:{label}"),
                "fileHashes": [{
                    "path": "src/lib.rs",
                    "sha256": "1111111111111111111111111111111111111111111111111111111111111111",
                    "byteLen": 1,
                    "mtimeMs": index
                }],
                "artifactIds": [format!("search/{label}.json")]
            })
        })
        .collect::<Vec<_>>();
    serde_json::from_value(json!({
        "schemaId": "agent.semantic-protocols.client-cache-manifest",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.client",
        "protocolVersion": "1",
        "cacheRoot": client_dir.display().to_string(),
        "generations": generations,
    }))
    .expect("build scenario cache manifest")
}

fn lookup_generation(
    read_session: &agent_semantic_client_db::ClientDbEngineReadSession,
    project_root: &Path,
    request_fingerprint: &str,
) -> Result<Option<agent_semantic_client_db::ClientDbGenerationHit>, String> {
    read_session.lookup_generation_request(
        &LanguageId::from(LANGUAGE_ID),
        &ProviderId::from(PROVIDER_ID),
        project_root,
        &CacheExportMethod::from(EXPORT_METHOD),
        Some(request_fingerprint.to_owned()),
    )
}

fn scenario_root(label: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "asp-project-turso-scenario-{}-{label}-{suffix}",
        std::process::id()
    ))
}

fn project_client_dir(root: &Path) -> PathBuf {
    root.join("projects")
        .join("by-id")
        .join("project-performance")
        .join("live")
        .join("client")
}

fn cleanup_scenario_root(root: &Path) {
    if std::env::var_os("ASP_TURSO_KEEP_SCENARIO_ROOT").is_some() {
        return;
    }
    if root.exists() {
        fs::remove_dir_all(root).expect("remove scenario root");
    }
}

fn scenario_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

fn scenario_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

fn elapsed_us(started: Instant) -> u64 {
    started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LatencyMetrics {
    p50_us: u64,
    p95_us: u64,
    p99_us: u64,
}

fn latency_metrics(samples_us: &[u64]) -> LatencyMetrics {
    assert!(!samples_us.is_empty(), "latency sample set is empty");
    let mut sorted = samples_us.to_vec();
    sorted.sort_unstable();
    LatencyMetrics {
        p50_us: percentile(&sorted, 50),
        p95_us: percentile(&sorted, 95),
        p99_us: percentile(&sorted, 99),
    }
}

fn percentile(sorted_samples: &[u64], percentile: usize) -> u64 {
    let last_index = sorted_samples.len() - 1;
    let index = (last_index * percentile).div_ceil(100);
    sorted_samples[index]
}
