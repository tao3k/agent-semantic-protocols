use std::time::{Duration, Instant};

use super::{ValidatedSortedRecordTable, encode_sorted_record_table};

#[test]
fn large_directory_table_parse_p99_is_sub_millisecond() {
    const RECORDS: usize = 131_072;
    const RUNS: usize = 1_000;
    let records = (0..RECORDS)
        .map(|index| {
            (
                format!("symbol-{index:08x}").into_bytes(),
                format!("owner-{index:08x}").into_bytes(),
            )
        })
        .collect();
    let encoded = encode_sorted_record_table(records).expect("encode large search directory");
    let mut samples = Vec::with_capacity(RUNS);

    for _ in 0..RUNS {
        let started = Instant::now();
        let table = ValidatedSortedRecordTable::parse(&encoded).expect("open search directory");
        assert_eq!(table.len(), RECORDS);
        samples.push(started.elapsed());
    }

    samples.sort_unstable();
    let p99_index = samples.len().saturating_mul(99).div_ceil(100) - 1;
    let p99 = samples[p99_index];
    eprintln!(
        "search-memory-table-parse records={RECORDS} runs={RUNS} p99Nanos={}",
        p99.as_nanos()
    );
    assert!(
        p99 < Duration::from_millis(1),
        "large search directory cold-open p99 must remain sub-millisecond: p99={p99:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn two_hundred_fifty_six_concurrent_memory_search_readers_are_lock_free() {
    use std::sync::Arc;

    let records = (0..4_096usize)
        .map(|index| {
            (
                format!("symbol-{index:08x}").into_bytes(),
                format!("owner-{index:08x}").into_bytes(),
            )
        })
        .collect();
    let encoded =
        Arc::new(encode_sorted_record_table(records).expect("encode concurrent search directory"));
    let barrier = Arc::new(tokio::sync::Barrier::new(257));
    let mut readers = tokio::task::JoinSet::new();

    for _ in 0..256 {
        let encoded = Arc::clone(&encoded);
        let barrier = Arc::clone(&barrier);
        readers.spawn(async move {
            barrier.wait().await;
            let table = ValidatedSortedRecordTable::parse(encoded.as_slice())
                .expect("open immutable table");
            assert_eq!(
                table
                    .get_checked(b"symbol-00000fff")
                    .expect("validated lookup")
                    .expect("registered key"),
                b"owner-00000fff"
            );
        });
    }

    barrier.wait().await;
    while let Some(result) = readers.join_next().await {
        result.expect("memory search reader task completes");
    }
}

#[test]
fn memory_search_cost_receipt_is_opentelemetry_serializable() {
    let observation = crate::runtime_server_opentelemetry::RuntimePerformanceObservation::new(
        "exact-query",
        "resident-exact-generation-open",
        50,
        1_000,
        "within-budget",
    )
    .with_memory_search_metrics(52_000_000, 512, 128, 256, 64);
    let wire = serde_json::to_value(observation).expect("serialize memory search metrics");

    assert_eq!(wire["memorySearchMappedBytes"], 52_000_000);
    assert_eq!(wire["memorySearchDirectoryBytesValidated"], 512);
    assert_eq!(wire["memorySearchKeyBytesTouched"], 128);
    assert_eq!(wire["memorySearchValueBytesTouched"], 256);
    assert_eq!(wire["memorySearchSourceBytesRead"], 64);
    assert_eq!(wire["memorySearchTursoOpens"], 0);
    assert_eq!(wire["memorySearchSocketConnects"], 0);
    assert_eq!(wire["memorySearchProviderSpawns"], 0);
}

#[tokio::test(flavor = "current_thread")]
async fn structurally_empty_generation_reenters_runtime_owned_admission() {
    let temporary = tempfile::tempdir().expect("temporary generation directory");
    let project_root = temporary.path().join("workspace");
    tokio::fs::create_dir_all(&project_root)
        .await
        .expect("create workspace root");
    let generation_directory = temporary.path().join("generation");
    tokio::fs::create_dir_all(&generation_directory)
        .await
        .expect("create generation directory");
    let workspace_snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_bytes(
        std::iter::empty::<(&str, &[u8])>(),
    );
    let projection_capability = crate::active_generation_projection_capability::ActiveGenerationProjectionCapabilityManifest::from_source_index(
        format!(
            "blake3-256:{}",
            blake3::hash(b"empty-generation-provider-catalog").to_hex()
        ),
        &[],
    )
    .expect("empty projection capability manifest");
    let source_snapshot = workspace_snapshot.evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        projection_capability.provider_catalog_digest.clone(),
    );
    let generation = crate::runtime_server_workspace::WorkspaceMemoryGeneration::try_from_build(
        crate::runtime_server_workspace::WorkspaceGenerationBuild {
            projection_capability,
            workspace_identity: "workspace-empty-search-generation".to_owned(),
            project_root: project_root.to_string_lossy().into_owned(),
            active_epoch: 1,
            workspace_snapshot,
            source_snapshot,
            module_graph_digest: format!(
                "blake3-256:{}",
                blake3::hash(b"empty-generation-module-graph").to_hex()
            ),
            project_resolutions: Vec::new(),
            owners: Vec::new(),
            relations: Vec::new(),
        },
    )
    .expect("empty workspace generation");
    let publisher =
        crate::runtime_server_workspace::WorkspaceGenerationPublisher::new(generation_directory)
            .await
            .expect("generation publisher");
    publisher
        .publish(std::sync::Arc::new(generation), false)
        .await
        .expect("publish empty generation");
    let client = crate::runtime_server_workspace::WorkspaceSearchGenerationDataPlaneClient::open(
        publisher.pointer_path(),
        &project_root,
    )
    .await
    .expect("open empty published generation");
    let lookup = client
        .read_source_index("anything", None, 8)
        .expect("read empty published generation");
    assert_eq!(
        lookup.state,
        crate::ClientDbSourceIndexLookupState::ColdRequired,
        "a structurally empty active generation must trigger Runtime-owned cold admission"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn resident_merkle_read_uses_the_published_generation_with_sub_millisecond_budget() {
    let temporary = tempfile::tempdir().expect("temporary generation directory");
    let project_root = temporary.path().join("workspace");
    tokio::fs::create_dir_all(&project_root)
        .await
        .expect("create workspace root");
    let generation_directory = temporary.path().join("generation");
    tokio::fs::create_dir_all(&generation_directory)
        .await
        .expect("create generation directory");

    let owner_bytes = b"pub fn cold_owner() {}\n".to_vec();
    let sibling_bytes = b"pub fn sibling_owner() {}\n".to_vec();
    let workspace_snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_bytes([
        ("src/lib.rs", owner_bytes.as_slice()),
        ("src/sibling.rs", sibling_bytes.as_slice()),
    ]);
    let projection_capability =
        crate::active_generation_projection_capability::test_projection_capability_manifest();
    let source_snapshot = workspace_snapshot.evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        projection_capability.provider_catalog_digest.clone(),
    );
    let generation = crate::runtime_server_workspace::WorkspaceMemoryGeneration::try_from_build(
        crate::runtime_server_workspace::WorkspaceGenerationBuild {
            projection_capability,
            workspace_identity: "workspace-cold-merkle-read".to_owned(),
            project_root: project_root.to_string_lossy().into_owned(),
            active_epoch: 1,
            workspace_snapshot,
            source_snapshot,
            module_graph_digest: format!(
                "blake3-256:{}",
                blake3::hash(b"cold-merkle-module-graph").to_hex()
            ),
            project_resolutions: Vec::new(),
            owners: vec![
                crate::runtime_server_workspace::WorkspaceOwnerSnapshot {
                    owner_path: "src/lib.rs".to_owned(),
                    content_digest: format!("blake3-256:{}", blake3::hash(&owner_bytes).to_hex()),
                    bytes: owner_bytes,
                    selectors: Vec::new(),
                },
                crate::runtime_server_workspace::WorkspaceOwnerSnapshot {
                    owner_path: "src/sibling.rs".to_owned(),
                    content_digest: format!("blake3-256:{}", blake3::hash(&sibling_bytes).to_hex()),
                    bytes: sibling_bytes,
                    selectors: Vec::new(),
                },
            ],
            relations: Vec::new(),
        },
    )
    .expect("workspace generation");
    let publisher =
        crate::runtime_server_workspace::WorkspaceGenerationPublisher::new(generation_directory)
            .await
            .expect("generation publisher");
    let snapshot = publisher
        .publish(std::sync::Arc::new(generation), false)
        .await
        .expect("publish generation before cold client starts");

    let client = crate::runtime_server_workspace::WorkspaceSearchGenerationDataPlaneClient::open(
        publisher.pointer_path(),
        &project_root,
    )
    .await
    .expect("resident client opens published search generation");
    let lookup = client
        .read_source_index("unmatched-query", None, 8)
        .expect("read structurally empty selector index");
    assert_eq!(
        lookup.state,
        crate::ClientDbSourceIndexLookupState::Miss,
        "a nonempty generation with no selector records remains valid for free-text lookup"
    );
    let started = tokio::time::Instant::now();
    let read = client
        .read_merkle_owner("src/lib.rs")
        .expect("resident client reads Merkle owner proof");
    let elapsed = started.elapsed();

    let crate::runtime_server_workspace::WorkspaceRuntimeMerkleOwnerRead::Owner {
        active_epoch,
        generation_digest,
        root_digest,
        owner_path,
        inclusion_proof,
        ..
    } = read
    else {
        panic!("published owner must be present");
    };
    assert_eq!(active_epoch, snapshot.active_epoch);
    assert_eq!(generation_digest, snapshot.generation_digest);
    assert_eq!(root_digest, snapshot.source_root_digest);
    assert_eq!(owner_path, "src/lib.rs");
    assert!(!inclusion_proof.is_empty());
    eprintln!(
        "search-published-generation-resident-merkle-read elapsedMicros={}",
        elapsed.as_micros()
    );
    assert!(
        elapsed < std::time::Duration::from_millis(1),
        "published-generation resident Merkle read must remain sub-millisecond: elapsed={elapsed:?}"
    );
}
