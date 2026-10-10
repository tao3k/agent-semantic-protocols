// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Resident source-index lookup and latency gates for one workspace generation lease.

use std::time::Instant;

use agent_semantic_client_db::runtime_server_workspace::RuntimeServerWorkspaceRegistry;
use agent_semantic_client_db::runtime_server_workspace::WorkspaceGenerationBuild;
use agent_semantic_client_db::runtime_server_workspace::WorkspaceMemoryGeneration;
use agent_semantic_client_db::runtime_server_workspace::WorkspaceOwnerSnapshot;
use agent_semantic_client_db::runtime_server_workspace::WorkspaceRecoverySource;
use agent_semantic_client_db::runtime_server_workspace::WorkspaceSelectorSnapshot;

const SAMPLE_COUNT: usize = 2_048;
const P95_BUDGET_NS: u128 = 1_000_000;
const P99_HARD_BUDGET_NS: u128 = 10_000_000;

fn project_root() -> std::path::PathBuf {
    std::path::PathBuf::from("/runtime-server-resident-source-index/workspace-a")
}

fn generation() -> WorkspaceMemoryGeneration {
    let bytes = b"fn run_provider_projection_batch() {}";
    let owner_path = "src/projection.rs";
    let workspace_snapshot =
        agent_semantic_content_identity::WorkspaceSnapshot::from_file_bytes([(
            owner_path,
            bytes.as_slice(),
        )]);
    let source_snapshot = workspace_snapshot.evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        blake3::hash(b"resident-source-index-provider")
            .to_hex()
            .to_string(),
    );
    let module_graph_digest = format!(
        "blake3-256:{}",
        blake3::hash(b"resident-source-index-module-graph").to_hex()
    );
    let runtime_provider_execution_binding = agent_semantic_artifacts::runtime_provider_execution_binding::RuntimeProviderExecutionBinding::build(
        crate::fixture::FIXTURE_PROJECT_ID.to_owned(),
        "workspace-a".to_owned(),
        format!("blake3-256:{}", "1".repeat(64)),
        format!("blake3-256:{}", "2".repeat(64)),
        format!("blake3-256:{}", "3".repeat(64)),
        source_snapshot
            .root_integrity_reference()
            .expect("source snapshot integrity reference"),
        module_graph_digest.clone(),
    )
    .expect("Runtime provider execution binding");
    WorkspaceMemoryGeneration::try_from_build(WorkspaceGenerationBuild {
    projection_capability: agent_semantic_client_db::active_generation_projection_capability::ActiveGenerationProjectionCapabilityManifest::single_selector("blake3-256:0000000000000000000000000000000000000000000000000000000000000000".to_owned(), "rust://fixture/src/lib.rs#item/function/fixture".to_owned(), "src/lib.rs".to_owned(), std::collections::BTreeSet::from([agent_semantic_client_db::active_generation_projection_capability::ActiveGenerationProjectionMode::Source])).expect("test projection capability manifest"),
        relations: Vec::new(),
        workspace_identity: "workspace-a".to_owned(),
        project_root: project_root().display().to_string(),
        active_epoch: 1,
        workspace_snapshot,
        content_search_generation: crate::fixture::content_search_generation_receipt(
            "workspace-a",
            &source_snapshot,
        ),
        source_snapshot,
        module_graph_digest,
        runtime_provider_execution_binding: Some(runtime_provider_execution_binding),
        project_resolutions: Vec::new(),
        auxiliary_owners: Vec::new(),
        owners: vec![WorkspaceOwnerSnapshot {
                        authority: None,
            owner_path: owner_path.to_owned(),
            content_digest: format!("blake3-256:{}", blake3::hash(bytes).to_hex()),
            bytes: bytes.to_vec(),
            native_syntax_diagnostic: None,
            selectors: vec![WorkspaceSelectorSnapshot {
                selector: "rust://src/projection.rs#item/function/run_provider_projection_batch"
                    .to_owned(),
                byte_start: 0,
                byte_end: bytes.len(),
                query_keys: Vec::new(),
                derived_projections: Vec::new(),
            }],
        }],
    })
    .expect("resident source-index generation")
}

#[tokio::test(flavor = "multi_thread")]
async fn resident_source_index_read_is_zero_io_with_bounded_tail_latency() {
    let _performance = crate::test_support::performance_lock();
    let temporary = tempfile::tempdir().expect("temporary runtime root");
    let registry =
        RuntimeServerWorkspaceRegistry::new(temporary.path().to_path_buf()).expect("registry");
    registry
        .publish(
            "resident-source-index",
            WorkspaceRecoverySource::TursoGeneration,
            generation(),
        )
        .await
        .expect("publish resident generation");
    let lease = registry
        .lease("workspace-a", &project_root())
        .expect("generation lease");
    let language_id = agent_semantic_client_core::LanguageId::from("rust");
    let mut samples = Vec::with_capacity(SAMPLE_COUNT);

    for _ in 0..SAMPLE_COUNT {
        let started_at = Instant::now();
        let lookup = lease
            .read_source_index("provider projection batch", Some(&language_id), 16)
            .expect("resident source-index lookup");
        samples.push(started_at.elapsed().as_nanos());
        assert_eq!(
            lookup.state,
            agent_semantic_client_db::ClientDbSourceIndexLookupState::Hit
        );
        assert_eq!(lookup.candidates.len(), 1);
        assert_eq!(lookup.candidates[0].path.as_str(), "src/projection.rs");
    }

    samples.sort_unstable();
    let p95 = samples[(samples.len() * 95 / 100).min(samples.len() - 1)];
    let p99 = samples[(samples.len() * 99 / 100).min(samples.len() - 1)];
    assert!(
        p95 < P95_BUDGET_NS,
        "resident source-index p95 exceeded 1ms: p95Ns={p95}"
    );
    assert!(
        p99 < P99_HARD_BUDGET_NS,
        "resident source-index p99 exceeded 10ms: p99Ns={p99}"
    );
    let counters = registry.data_plane_counters();
    assert_eq!(counters.database_opens, 0);
    assert_eq!(counters.provider_spawns, 0);
    assert_eq!(counters.control_socket_roundtrips, 0);
    println!(
        "residentSourceIndex samples={SAMPLE_COUNT} p95Ns={p95} p99Ns={p99} databaseOpens=0 providerSpawns=0 controlSocketRoundtrips=0"
    );
}
