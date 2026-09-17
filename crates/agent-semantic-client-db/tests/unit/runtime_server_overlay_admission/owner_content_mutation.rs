// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::{RuntimeServerWorkspaceRegistry, WorkspaceOwnerSnapshot, WorkspaceRecoverySource};
use agent_semantic_client_db::runtime_server_workspace::{
    WORKSPACE_OWNER_CONTENT_MUTATION_SCHEMA_ID, WORKSPACE_OWNER_TOPOLOGY_REBIND_SCHEMA_ID,
    WorkspaceOwnerContentMutationV1, WorkspaceOwnerContentUpsertV1, WorkspaceOwnerTopologyRebindV1,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn one_owner_content_mutation_is_atomic_and_does_not_republish_the_generation() {
    let root = super::fixture_root();
    let workspace_identity = "workspace-owner-content-mutation-v1";
    let registry =
        RuntimeServerWorkspaceRegistry::new(root.clone()).expect("create workspace registry");
    let previous = b"fn previous() {}\n";
    let authority = agent_semantic_search::ResidentSearchAuthority {
        language_id: agent_semantic_config::LanguageId::new("rust"),
        provider_id: agent_semantic_config::ProviderId::new("asp-rust"),
    };
    registry
        .publish(
            "owner-content-base",
            WorkspaceRecoverySource::TursoGeneration,
            super::overlay_fixture::generation_with_authority_and_selectors(
                workspace_identity,
                &root,
                1,
                previous,
                Some(authority.clone()),
                vec![
                    agent_semantic_client_db::runtime_server_workspace::WorkspaceSelectorSnapshot {
                        selector: "rust://src/lib.rs#item/function/previous_symbol".to_owned(),
                        byte_start: 0,
                        byte_end: previous.len(),
                        query_keys: vec!["previous_symbol".to_owned()],
                        derived_projections: Vec::new(),
                    },
                ],
            ),
        )
        .await
        .expect("publish canonical generation");
    let old_lease = registry
        .lease(workspace_identity, &root)
        .expect("lease old resident state");
    let old_resident =
        agent_semantic_client_db::runtime_resident_read::RuntimeResidentReadClient::from_resident_lease(
            old_lease.clone(),
        )
        .expect("open old resident read");
    assert_eq!(
        old_resident
            .read_topology_index("previous_symbol", 8)
            .expect("old topology index hit")
            .len(),
        1
    );
    assert_eq!(
        old_resident
            .smallest_enclosing_topology_anchor("src/lib.rs", 3, 11,)
            .expect("old content-bound anchor")
            .expect("old selector anchor")
            .structural_selector,
        "rust://src/lib.rs#item/function/previous_symbol"
    );
    let base_generation_digest = old_lease.generation().generation_digest.clone();
    let pointer_path =
        agent_semantic_client_db::runtime_server_workspace::workspace_generation_pointer_path(
            &root,
            workspace_identity,
            &root,
        )
        .expect("generation pointer path");
    let pointer_before = tokio::fs::read(&pointer_path)
        .await
        .expect("read canonical pointer");
    let next = b"fn next() { let body_only_token = 1; }\n";
    let receipt = registry
        .publish_owner_content_mutation(
            workspace_identity,
            &root,
            WorkspaceOwnerContentMutationV1 {
                schema_id: WORKSPACE_OWNER_CONTENT_MUTATION_SCHEMA_ID.to_owned(),
                schema_version: "1".to_owned(),
                mutation_id: "post-tool-edit-1".to_owned(),
                base_generation_digest: base_generation_digest.clone(),
                upserts: vec![WorkspaceOwnerContentUpsertV1 {
                    previous_content_digest: Some(format!(
                        "blake3-256:{}",
                        blake3::hash(previous).to_hex()
                    )),
                    owner: WorkspaceOwnerSnapshot {
                        authority: Some(authority.clone()),
                        owner_path: "src/lib.rs".to_owned(),
                        content_digest: format!("blake3-256:{}", blake3::hash(next).to_hex()),
                        bytes: next.to_vec(),
                        native_syntax_diagnostic: None,
                        selectors: Vec::new(),
                    },
                }],
                removals: Vec::new(),
            },
        )
        .await
        .expect("publish owner-local content transaction");

    receipt.validate().expect("valid V1 mutation receipt");
    assert_eq!(receipt.base_generation_digest, base_generation_digest);
    assert_eq!(receipt.upserted_owner_count, 1);
    assert_eq!(receipt.removed_owner_count, 0);
    assert_eq!(receipt.touched_leaf_count, 1);
    assert_eq!(receipt.full_merkle_rebuilds, 0);
    assert_eq!(receipt.written_node_count, "src/lib.rs".len() + 1);
    assert_eq!(
        old_lease.owner("src/lib.rs").as_deref(),
        Some(previous.as_slice())
    );

    let current = registry
        .lease(workspace_identity, &root)
        .expect("lease committed resident state");
    assert_eq!(
        current.owner("src/lib.rs").as_deref(),
        Some(next.as_slice())
    );
    assert_eq!(
        current.epoch(),
        1,
        "content delta does not republish the generation"
    );
    assert_ne!(current.runtime_generation_digest(), base_generation_digest);
    assert_eq!(
        tokio::fs::read(&pointer_path)
            .await
            .expect("read unchanged canonical pointer"),
        pointer_before
    );
    let content_generation_digest = current
        .generation()
        .content_search_generation
        .content_generation_digest
        .clone();
    let resident =
        agent_semantic_client_db::runtime_resident_read::RuntimeResidentReadClient::from_resident_lease(
            current,
        )
        .expect("open resident owner-overlay read");
    resident
        .build_lexical_attachment(
            &content_generation_digest,
            agent_semantic_search::ResidentIndexBuildResources::new(
                1,
                agent_semantic_search::ResidentIndexBuildResources::TANTIVY_MINIMUM_ARENA_BYTES_PER_THREAD,
                agent_semantic_search::ResidentIndexBuildStrategy::SingleSegmentBulk,
            )
            .expect("fixture Tantivy resources"),
        )
        .expect("build fixture base Tantivy attachment");
    let next_plan = agent_semantic_search::build_resident_grep_candidate_plan("next", false, true)
        .expect("next trigram plan");
    let previous_plan =
        agent_semantic_search::build_resident_grep_candidate_plan("previous", false, true)
            .expect("previous trigram plan");
    assert_eq!(
        resident
            .resident_grep_candidate_owner_paths(&next_plan, 8)
            .expect("new owner candidate")
            .0,
        ["src/lib.rs"]
    );
    assert!(
        resident
            .resident_grep_candidate_owner_paths(&previous_plan, 8)
            .expect("stale owner candidate shadowing")
            .0
            .is_empty()
    );
    assert!(
        resident
            .smallest_enclosing_topology_anchor("src/lib.rs", 3, 7,)
            .expect("content mutation shadows the previous anchor shard")
            .is_none()
    );
    assert!(
        resident
            .read_topology_index("previous_symbol", 8)
            .expect("stale topology shard is rejected after owner mutation")
            .is_empty()
    );
    assert_eq!(
        resident
            .resident_owner_bytes("src/lib.rs")
            .expect("resident owner bytes")
            .as_deref(),
        Some(next.as_slice())
    );
    assert_eq!(
        resident
            .resident_tantivy_owner_paths(
                "title:lib",
                &agent_semantic_client_core::LanguageId::from("rust"),
                None,
                8,
            )
            .expect("owner-path Tantivy delta hit"),
        ["src/lib.rs"]
    );
    assert!(
        resident
            .resident_tantivy_owner_paths(
                "body:body_only_token",
                &agent_semantic_client_core::LanguageId::from("rust"),
                None,
                8,
            )
            .expect("function body tokens stay outside Topology Index")
            .is_empty()
    );
    assert!(
        resident
            .resident_tantivy_owner_paths(
                "body:previous",
                &agent_semantic_client_core::LanguageId::from("rust"),
                None,
                8,
            )
            .expect("stale Tantivy base shadowing")
            .is_empty()
    );

    let rebind_receipt = registry
        .publish_resident_owner_topology_rebind(
            workspace_identity,
            &root,
            WorkspaceOwnerTopologyRebindV1 {
                schema_id: WORKSPACE_OWNER_TOPOLOGY_REBIND_SCHEMA_ID.to_owned(),
                schema_version: "1".to_owned(),
                rebind_id: "parser-rebind-1".to_owned(),
                base_generation_digest: base_generation_digest.clone(),
                owners: vec![WorkspaceOwnerSnapshot {
                    authority: Some(authority),
                    owner_path: "src/lib.rs".to_owned(),
                    content_digest: format!("blake3-256:{}", blake3::hash(next).to_hex()),
                    bytes: next.to_vec(),
                    native_syntax_diagnostic: None,
                    selectors: vec![
                        agent_semantic_client_db::runtime_server_workspace::WorkspaceSelectorSnapshot {
                            selector: "rust://src/lib.rs#item/function/next_symbol".to_owned(),
                            byte_start: 0,
                            byte_end: next.len(),
                            query_keys: vec!["next_symbol".to_owned()],
                            derived_projections: Vec::new(),
                        },
                    ],
                }],
                relations: Vec::new(),
            },
        )
        .await
        .expect("publish V1 owner topology rebind");
    rebind_receipt.validate().expect("valid V1 rebind receipt");
    assert_eq!(rebind_receipt.rebound_owner_count, 1);
    assert_eq!(
        rebind_receipt.topology_node_count, 3,
        "src directory, owner, and parser-native function are peer topology nodes"
    );
    let rebound = registry
        .resident_read_client(workspace_identity, &root)
        .expect("read rebound resident symbols");
    assert_eq!(
        rebound
            .read_topology_index("next_symbol", 8)
            .expect("new parser symbol is resident")
            .len(),
        1
    );
    assert_eq!(
        rebound
            .smallest_enclosing_topology_anchor("src/lib.rs", 3, 7,)
            .expect("rebound content-bound anchor")
            .expect("rebound selector anchor")
            .structural_selector,
        "rust://src/lib.rs#item/function/next_symbol"
    );
    assert!(
        rebound
            .read_topology_index("previous_symbol", 8)
            .expect("old symbol remains shadowed")
            .is_empty()
    );
    assert_eq!(
        rebound
            .resident_tantivy_owner_paths(
                "body:next_symbol",
                &agent_semantic_client_core::LanguageId::from("rust"),
                None,
                8,
            )
            .expect("V1 Scheme Tantivy axis observes the rebound topology index"),
        ["src/lib.rs"]
    );
    assert!(
        rebound
            .resident_tantivy_owner_paths(
                "body:body_only_token",
                &agent_semantic_client_core::LanguageId::from("rust"),
                None,
                8,
            )
            .expect("function body remains outside the rebound topology index")
            .is_empty()
    );
    assert_eq!(
        rebound
            .resident_owner_bytes("src/lib.rs")
            .expect("rebound owner bytes")
            .as_deref(),
        Some(next.as_slice())
    );

    registry.shutdown().await.expect("drain writer lane");
    let _ = tokio::fs::remove_dir_all(root).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn owner_content_mutation_rejects_a_stale_previous_digest_without_partial_visibility() {
    let root = super::fixture_root();
    let workspace_identity = "workspace-owner-content-stale-cas";
    let registry =
        RuntimeServerWorkspaceRegistry::new(root.clone()).expect("create workspace registry");
    let previous = b"fn previous() {}\n";
    registry
        .publish(
            "owner-content-stale-base",
            WorkspaceRecoverySource::TursoGeneration,
            super::generation(workspace_identity, &root, 1, previous),
        )
        .await
        .expect("publish canonical generation");
    let base = registry
        .lease(workspace_identity, &root)
        .expect("lease canonical generation");
    let error = registry
        .publish_owner_content_mutation(
            workspace_identity,
            &root,
            WorkspaceOwnerContentMutationV1 {
                schema_id: WORKSPACE_OWNER_CONTENT_MUTATION_SCHEMA_ID.to_owned(),
                schema_version: "1".to_owned(),
                mutation_id: "post-tool-stale-1".to_owned(),
                base_generation_digest: base.generation().generation_digest.clone(),
                upserts: vec![WorkspaceOwnerContentUpsertV1 {
                    previous_content_digest: Some(format!("blake3-256:{}", "0".repeat(64))),
                    owner: WorkspaceOwnerSnapshot {
                        authority: None,
                        owner_path: "src/lib.rs".to_owned(),
                        content_digest: format!(
                            "blake3-256:{}",
                            blake3::hash(b"fn invalid() {}\n").to_hex()
                        ),
                        bytes: b"fn invalid() {}\n".to_vec(),
                        native_syntax_diagnostic: None,
                        selectors: Vec::new(),
                    },
                }],
                removals: Vec::new(),
            },
        )
        .await
        .expect_err("stale owner CAS must fail closed");
    assert!(error.contains("previous digest mismatch"));
    let current = registry
        .lease(workspace_identity, &root)
        .expect("lease unchanged resident state");
    assert_eq!(
        current.owner("src/lib.rs").as_deref(),
        Some(previous.as_slice())
    );
    assert_eq!(
        current.runtime_generation_digest(),
        base.runtime_generation_digest()
    );

    registry.shutdown().await.expect("drain writer lane");
    let _ = tokio::fs::remove_dir_all(root).await;
}
