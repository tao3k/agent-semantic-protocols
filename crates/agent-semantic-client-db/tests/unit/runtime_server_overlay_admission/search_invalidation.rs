// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Atomic invalidation tests for owner-derived lexical and graph search state.

use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use agent_semantic_client_db::runtime_server_workspace::RuntimeServerWorkspaceRegistry;
use agent_semantic_client_db::runtime_server_workspace::WORKSPACE_GENERATION_DELTA_SCHEMA_ID;
use agent_semantic_client_db::runtime_server_workspace::WorkspaceGenerationBuild;
use agent_semantic_client_db::runtime_server_workspace::WorkspaceGenerationDelta;
use agent_semantic_client_db::runtime_server_workspace::WorkspaceMemoryGeneration;
use agent_semantic_client_db::runtime_server_workspace::WorkspaceOwnerSnapshot;
use agent_semantic_client_db::runtime_server_workspace::WorkspaceRecoverySource;
use agent_semantic_client_db::runtime_server_workspace::WorkspaceSelectorSnapshot;

static NEXT_FIXTURE_ID: AtomicU64 = AtomicU64::new(1);

fn fixture_root() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "asp-runtime-search-invalidation-{}-{}",
        std::process::id(),
        NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed)
    ))
}

fn content_search_generation_receipt(
    workspace_identity: &str,
    source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
) -> agent_semantic_search::ContentSearchGenerationReceipt {
    use agent_semantic_search::ContentSearchGenerationReceipt;
    use agent_semantic_search::SearchGenerationConstructionStage;
    use agent_semantic_search::SearchGenerationIdentity;
    use agent_semantic_search::SearchGenerationStageReceipt;
    use agent_semantic_search::canonical_blake3_digest;
    let identity = SearchGenerationIdentity {
        project_id: "project-overlay-fixture".to_owned(),
        workspace_id: workspace_identity.to_owned(),
        source_root_digest: canonical_blake3_digest(&source_snapshot.root_digest).unwrap(),
        provider_digest: canonical_blake3_digest(&source_snapshot.provider_digest).unwrap(),
        schema_digest: format!("blake3-256:{}", "0".repeat(64)),
        generation_candidate_digest: format!("blake3-256:{}", "1".repeat(64)),
    };
    let stage = |kind, byte: char, worker: &str| SearchGenerationStageReceipt {
        stage: kind,
        identity: identity.clone(),
        artifact_digest: format!("blake3-256:{}", byte.to_string().repeat(64)),
        worker_id: worker.to_owned(),
        complete: true,
    };
    ContentSearchGenerationReceipt::new(stage(
        SearchGenerationConstructionStage::SourceByteAcquisition,
        '4',
        "fixture-source-byte-acquisition",
    ))
    .unwrap()
}

fn generation(
    workspace_identity: &str,
    project_root: &std::path::Path,
    bytes: &[u8],
    selector: WorkspaceSelectorSnapshot,
    relation: agent_semantic_client_db::ClientDbSourceIndexOwnedRelation,
) -> WorkspaceMemoryGeneration {
    let content_digest = format!("blake3-256:{}", blake3::hash(bytes).to_hex());
    let workspace_snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes(
        [("src/lib.rs", content_digest.clone())],
    );
    let source_snapshot = workspace_snapshot.evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        format!(
            "blake3-256:{}",
            blake3::hash(b"runtime-search-invalidation-provider").to_hex()
        ),
    );
    WorkspaceMemoryGeneration::try_from_build(WorkspaceGenerationBuild {
        projection_capability:
            agent_semantic_client_db::fixture::projection_capability_manifest_fixture(),
        relations: vec![relation],
        workspace_identity: workspace_identity.to_owned(),
        project_root: project_root.display().to_string(),
        active_epoch: 1,
        workspace_snapshot,
        content_search_generation: content_search_generation_receipt(
            workspace_identity,
            &source_snapshot,
        ),
        source_snapshot,
        module_graph_digest: format!(
            "blake3-256:{}",
            blake3::hash(b"runtime-search-invalidation-module-graph").to_hex()
        ),
        runtime_provider_execution_binding: None,
        project_resolutions: Vec::new(),
        auxiliary_owners: Vec::new(),
        owners: vec![WorkspaceOwnerSnapshot {
            authority: None,
            owner_path: "src/lib.rs".to_owned(),
            content_digest,
            bytes: bytes.to_vec(),
            native_syntax_diagnostic: None,
            selectors: vec![selector],
        }],
    })
    .expect("typed runtime search generation")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn owner_delta_invalidates_lexical_postings_and_graph_edges_in_one_epoch() {
    let root = fixture_root();
    let workspace_identity = "workspace-atomic-search-invalidation";
    let registry =
        RuntimeServerWorkspaceRegistry::new(root.clone()).expect("create workspace registry");
    let stale_selector = WorkspaceSelectorSnapshot {
        selector: "rust://src/lib.rs#item/function/stale".to_owned(),
        byte_start: 0,
        byte_end: b"fn stale() {}".len(),
        query_keys: vec!["stale".to_owned()],
        derived_projections: Vec::new(),
    };
    let stale_relation = agent_semantic_client_db::ClientDbSourceIndexOwnedRelation {
        owner_path: agent_semantic_client_db::ClientDbSourceIndexPath::new("src/lib.rs"),
        relation:
            agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelation {
                from: agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelationEndpoint {
                    kind: agent_semantic_content_identity::ProviderRelationEndpointKindV1::Item,
                    id: stale_selector.selector.clone(),
                },
                kind: "calls".into(),
                to: agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelationEndpoint {
                    kind: agent_semantic_content_identity::ProviderRelationEndpointKindV1::Item,
                    id: "rust://src/lib.rs#item/function/dependency".into(),
                },
            },
    };
    registry
        .publish(
            "publish-stale-search-generation",
            WorkspaceRecoverySource::TursoGeneration,
            generation(
                workspace_identity,
                &root,
                b"fn stale() {}",
                stale_selector.clone(),
                stale_relation,
            ),
        )
        .await
        .expect("publish stale search generation");
    let old_lease = registry
        .lease(workspace_identity, &root)
        .expect("lease stale search generation");
    assert_eq!(
        old_lease
            .read_source_index("stale", None, 8)
            .expect("query stale lexical posting")
            .candidates
            .len(),
        1
    );
    assert_eq!(
        old_lease
            .relations_from("item", &stale_selector.selector)
            .len(),
        1
    );

    let fresh_bytes = b"fn fresh() {}";
    let fresh_owner = WorkspaceOwnerSnapshot {
        authority: None,
        owner_path: "src/lib.rs".to_owned(),
        content_digest: format!("blake3-256:{}", blake3::hash(fresh_bytes).to_hex()),
        bytes: fresh_bytes.to_vec(),
        native_syntax_diagnostic: None,
        selectors: vec![WorkspaceSelectorSnapshot {
            selector: "rust://src/lib.rs#item/function/fresh".to_owned(),
            byte_start: 0,
            byte_end: fresh_bytes.len(),
            query_keys: vec!["fresh".to_owned()],
            derived_projections: Vec::new(),
        }],
    };
    registry
        .publish_owner_delta(
            "replace-owner-and-invalidate-derived-search-state",
            workspace_identity,
            &root,
            WorkspaceGenerationDelta {
                schema_id: WORKSPACE_GENERATION_DELTA_SCHEMA_ID.to_owned(),
                schema_version: "2".to_owned(),
                base_generation_digest: old_lease.generation().generation_digest.clone(),
                owners: vec![fresh_owner],
                tombstones: Vec::new(),
                relations: Vec::new(),
            },
        )
        .await
        .expect("publish fresh owner generation");

    let current = registry
        .lease(workspace_identity, &root)
        .expect("lease fresh search generation");
    assert!(
        current
            .read_source_index("stale", None, 8)
            .expect("stale lexical lookup after owner replacement")
            .candidates
            .is_empty()
    );
    assert_eq!(
        current
            .read_source_index("fresh", None, 8)
            .expect("fresh lexical lookup after owner replacement")
            .candidates
            .len(),
        1
    );
    assert!(
        current
            .relations_from("item", &stale_selector.selector)
            .is_empty()
    );
    assert_eq!(old_lease.epoch(), 1, "old lease remains immutable");
    assert_eq!(current.epoch(), 2, "one owner delta publishes one epoch");

    registry.shutdown().await.expect("drain writer lane");
    let _ = tokio::fs::remove_dir_all(root).await;
}
