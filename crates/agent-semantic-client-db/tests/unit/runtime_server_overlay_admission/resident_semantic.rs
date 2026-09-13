// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::{
    RuntimeServerWorkspaceRegistry, WORKSPACE_GENERATION_DELTA_SCHEMA_ID, WorkspaceGenerationDelta,
    WorkspaceOwnerSnapshot, WorkspaceRecoverySource, fixture_root, generation,
    generation_with_selectors,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn resident_delta_keeps_canonical_semantic_owners_as_a_read_through_base() {
    let root = fixture_root();
    let workspace_identity = "workspace-resident-overlay-base-read-through";
    let base_bytes = b"fn canonical() {}\n";
    let base_selector = "rust://src/lib.rs#item/function/canonical";
    let registry =
        RuntimeServerWorkspaceRegistry::new(root.clone()).expect("create workspace registry");
    registry
        .publish(
            "resident-overlay-base-read-through",
            WorkspaceRecoverySource::TursoGeneration,
            generation_with_selectors(
                workspace_identity,
                &root,
                1,
                base_bytes,
                vec![
                    agent_semantic_client_db::runtime_server_workspace::WorkspaceSelectorSnapshot {
                        selector: base_selector.to_owned(),
                        byte_start: 0,
                        byte_end: base_bytes.len(),
                        query_keys: vec!["canonical".to_owned()],
                        derived_projections: Vec::new(),
                    },
                ],
            ),
        )
        .await
        .expect("publish canonical semantic generation");
    let base_generation_digest = registry
        .lease(workspace_identity, &root)
        .expect("lease canonical semantic generation")
        .generation()
        .generation_digest
        .clone();
    let delta_bytes = b"fn delta() {}\n";

    registry
        .publish_resident_owner_delta(
            workspace_identity,
            &root,
            WorkspaceGenerationDelta {
                schema_id: WORKSPACE_GENERATION_DELTA_SCHEMA_ID.to_owned(),
                schema_version: "2".to_owned(),
                base_generation_digest,
                owners: vec![WorkspaceOwnerSnapshot {
                    authority: None,
                    owner_path: "src/delta.rs".to_owned(),
                    content_digest: format!("blake3-256:{}", blake3::hash(delta_bytes).to_hex()),
                    bytes: delta_bytes.to_vec(),
                    native_syntax_diagnostic: None,
                    selectors: Vec::new(),
                }],
                tombstones: Vec::new(),
                relations: Vec::new(),
            },
        )
        .await
        .expect("publish resident semantic delta");

    let resident = registry
        .resident_read_client(workspace_identity, &root)
        .expect("open resident semantic read-through");
    assert!(
        resident
            .semantic_owner_materialized("src/lib.rs")
            .expect("read canonical semantic owner state")
    );
    assert!(
        resident
            .semantic_owner_materialized("src/delta.rs")
            .expect("read delta semantic owner state")
    );
    let (projections, _, diagnostics) = resident
        .native_syntax_playbook_projection(&["src/lib.rs".to_owned()])
        .expect("project canonical semantic owner through overlay");
    assert!(diagnostics.is_empty());
    assert_eq!(projections[0].selectors[0].selector, base_selector);

    registry.shutdown().await.expect("drain writer lane");
    let _ = tokio::fs::remove_dir_all(root).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn resident_parser_delta_is_visible_without_rewriting_the_canonical_generation() {
    let root = fixture_root();
    let workspace_identity = "workspace-resident-parser-delta";
    let registry =
        RuntimeServerWorkspaceRegistry::new(root.clone()).expect("create workspace registry");
    let bytes = b"fn resident() {}\n";
    registry
        .publish(
            "resident-parser-base",
            WorkspaceRecoverySource::TursoGeneration,
            generation(workspace_identity, &root, 1, bytes),
        )
        .await
        .expect("publish canonical base generation");
    let base = registry
        .lease(workspace_identity, &root)
        .expect("lease canonical base generation");
    let base_generation_digest = base.generation().generation_digest.clone();
    let pointer_path =
        agent_semantic_client_db::runtime_server_workspace::workspace_generation_pointer_path(
            &root,
            workspace_identity,
            &root,
        )
        .expect("resolve canonical pointer");
    let pointer_before = tokio::fs::read(&pointer_path)
        .await
        .expect("read canonical pointer before resident delta");
    let counters_before = registry.data_plane_counters();
    let selector = "rust://src/lib.rs#item/function/resident";

    let resident_digest = registry
        .publish_resident_owner_delta(
            workspace_identity,
            &root,
            WorkspaceGenerationDelta {
                schema_id: WORKSPACE_GENERATION_DELTA_SCHEMA_ID.to_owned(),
                schema_version: "2".to_owned(),
                base_generation_digest: base_generation_digest.clone(),
                owners: vec![WorkspaceOwnerSnapshot {
                    authority: None,
                    owner_path: "src/lib.rs".to_owned(),
                    content_digest: format!("blake3-256:{}", blake3::hash(bytes).to_hex()),
                    bytes: bytes.to_vec(),
                    native_syntax_diagnostic: None,
                    selectors: vec![
                        agent_semantic_client_db::runtime_server_workspace::WorkspaceSelectorSnapshot {
                            selector: selector.to_owned(),
                            byte_start: 0,
                            byte_end: bytes.len(),
                            query_keys: vec!["resident".to_owned()],
                            derived_projections: Vec::new(),
                        },
                    ],
                }],
                tombstones: Vec::new(),
                relations: Vec::new(),
            },
        )
        .await
        .expect("publish resident parser delta");

    let current = registry
        .lease(workspace_identity, &root)
        .expect("lease resident parser overlay");
    let (observed_resident_digest, owner) = current
        .runtime_owner_snapshot("src/lib.rs")
        .expect("read parser-owned resident owner");
    assert_eq!(observed_resident_digest, resident_digest);
    assert_ne!(resident_digest, base_generation_digest);
    let (scope_root, scope_generation_digest) = registry
        .unique_resident_scope(workspace_identity)
        .expect("resolve Query execution scope after resident overlay publication");
    assert_eq!(scope_root, root);
    assert_eq!(
        scope_generation_digest, base_generation_digest,
        "request-serving overlay identity must not replace the immutable base generation binding"
    );
    assert_eq!(owner.selectors[0].selector, selector);
    assert_eq!(current.epoch(), 1);
    assert_eq!(
        current.generation().generation_digest,
        base_generation_digest
    );
    assert_eq!(registry.data_plane_counters(), counters_before);
    assert_eq!(
        tokio::fs::read(&pointer_path)
            .await
            .expect("read canonical pointer after resident delta"),
        pointer_before
    );

    registry.shutdown().await.expect("drain writer lane");
    let _ = tokio::fs::remove_dir_all(root).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn resident_parser_delta_preserves_a_valid_empty_projection() {
    let root = fixture_root();
    let workspace_identity = "workspace-resident-empty-parser-delta";
    let registry =
        RuntimeServerWorkspaceRegistry::new(root.clone()).expect("create workspace registry");
    let bytes = b"// parser accepts an owner with no declarations\n";
    registry
        .publish(
            "resident-empty-parser-base",
            WorkspaceRecoverySource::TursoGeneration,
            generation(workspace_identity, &root, 1, bytes),
        )
        .await
        .expect("publish canonical base generation");
    let base_generation_digest = registry
        .lease(workspace_identity, &root)
        .expect("lease canonical base generation")
        .generation()
        .generation_digest
        .clone();

    registry
        .publish_resident_owner_delta(
            workspace_identity,
            &root,
            WorkspaceGenerationDelta {
                schema_id: WORKSPACE_GENERATION_DELTA_SCHEMA_ID.to_owned(),
                schema_version: "2".to_owned(),
                base_generation_digest,
                owners: vec![WorkspaceOwnerSnapshot {
                    authority: None,
                    owner_path: "src/lib.rs".to_owned(),
                    content_digest: format!("blake3-256:{}", blake3::hash(bytes).to_hex()),
                    bytes: bytes.to_vec(),
                    native_syntax_diagnostic: None,
                    selectors: Vec::new(),
                }],
                tombstones: Vec::new(),
                relations: Vec::new(),
            },
        )
        .await
        .expect("publish valid empty parser projection");

    let resident = registry
        .resident_read_client(workspace_identity, &root)
        .expect("open resident parser projection");
    let (projections, relations, diagnostics) = resident
        .native_syntax_playbook_projection(&["src/lib.rs".to_owned()])
        .expect("project valid empty native syntax");
    assert_eq!(projections.len(), 1);
    assert!(projections[0].selectors.is_empty());
    assert!(relations.is_empty());
    assert!(diagnostics.is_empty());

    registry.shutdown().await.expect("drain writer lane");
    let _ = tokio::fs::remove_dir_all(root).await;
}
