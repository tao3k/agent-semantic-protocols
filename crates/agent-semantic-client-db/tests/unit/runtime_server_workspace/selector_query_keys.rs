// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Exact-segment query-key publication tests.

use super::generation;
use super::owner;
use super::resident_pointer;
use agent_semantic_client_db::runtime_server_workspace::RuntimeServerWorkspaceRegistry;
use agent_semantic_client_db::runtime_server_workspace::WorkspaceExactProjectionDataPlaneClient;
use agent_semantic_client_db::runtime_server_workspace::WorkspaceSearchGenerationDataPlaneClient;
use tempfile::tempdir;

#[tokio::test(flavor = "multi_thread")]
async fn exact_projection_round_trip_preserves_parser_owned_selector_query_keys() {
    let temporary = tempdir().expect("temporary runtime root");
    let registry =
        RuntimeServerWorkspaceRegistry::new(temporary.path().to_path_buf()).expect("registry");
    let mut published_owner = owner(
        "src/lib.rs",
        "rust://src/lib.rs#item/function/compare",
        b"fn compare() {}",
    );
    published_owner.selectors[0].query_keys = vec!["compare".to_owned(), "function".to_owned()];
    registry
        .publish(
            "selector-query-key-round-trip",
            agent_semantic_client_db::runtime_server_workspace::WorkspaceRecoverySource::TursoGeneration,
            generation(
                "workspace-selector-query-key-round-trip",
                1,
                published_owner.clone(),
            ),
        )
        .await
        .expect("publish selector query-key generation");

    let pointer = resident_pointer(temporary.path(), "workspace-selector-query-key-round-trip");
    let client = WorkspaceExactProjectionDataPlaneClient::open(&pointer)
        .await
        .expect("open exact selector query-key index");
    let restored = client
        .owner_snapshot("src/lib.rs")
        .expect("read owner snapshot")
        .expect("owner exists");

    assert_eq!(restored, published_owner);
    let compare = client
        .owner_search_snapshot("src/lib.rs", &["compare".to_owned()], 100)
        .expect("read compare owner search")
        .expect("owner exists");
    assert_eq!(compare.candidate_count, 1);
    assert_eq!(compare.selectors.len(), 1);
    assert_eq!(
        compare.selectors[0].selector,
        "rust://src/lib.rs#item/function/compare"
    );

    let absent = client
        .owner_search_snapshot("src/lib.rs", &["absent".to_owned()], 100)
        .expect("read absent owner search")
        .expect("owner exists");
    assert_eq!(absent.candidate_count, 0);
    assert!(absent.selectors.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn exact_projection_round_trip_preserves_owner_syntax_diagnostic() {
    let temporary = tempdir().expect("temporary runtime root");
    let registry =
        RuntimeServerWorkspaceRegistry::new(temporary.path().to_path_buf()).expect("registry");
    let mut published_owner = owner(
        "src/unavailable.rs",
        "rust://src/unavailable.rs#item/function/placeholder",
        b"fn unavailable() {}",
    );
    published_owner.selectors.clear();
    published_owner.native_syntax_diagnostic =
        Some(agent_semantic_search::NativeSyntaxDiagnostic {
            owner_path: published_owner.owner_path.clone(),
            content_digest: published_owner.content_digest.clone(),
            reason_kind: "source-syntax-unavailable".to_owned(),
            message: "bounded parser diagnostic".to_owned(),
        });
    registry
        .publish(
            "owner-syntax-diagnostic-round-trip",
            agent_semantic_client_db::runtime_server_workspace::WorkspaceRecoverySource::TursoGeneration,
            generation(
                "workspace-owner-syntax-diagnostic-round-trip",
                1,
                published_owner.clone(),
            ),
        )
        .await
        .expect("publish diagnosed owner generation");

    let pointer = resident_pointer(
        temporary.path(),
        "workspace-owner-syntax-diagnostic-round-trip",
    );
    let client = WorkspaceExactProjectionDataPlaneClient::open(&pointer)
        .await
        .expect("open exact diagnosed owner index");
    let restored = client
        .owner_snapshot("src/unavailable.rs")
        .expect("read diagnosed owner snapshot")
        .expect("diagnosed owner exists");

    assert_eq!(restored, published_owner);
}

#[tokio::test(flavor = "multi_thread")]
async fn lexical_owner_without_parser_selectors_is_a_typed_native_syntax_diagnostic() {
    let temporary = tempdir().expect("temporary runtime root");
    let registry =
        RuntimeServerWorkspaceRegistry::new(temporary.path().to_path_buf()).expect("registry");
    let mut lexical_owner = owner(
        "build-support/asp-rust-project-harness-policy/src/member_policy.rs",
        "rust://build-support/asp-rust-project-harness-policy/src/member_policy.rs#item/module/member_policy",
        b"pub fn package_policy() {}",
    );
    lexical_owner.selectors.clear();
    registry
        .publish(
            "lexical-owner-without-parser-selectors",
            agent_semantic_client_db::runtime_server_workspace::WorkspaceRecoverySource::TursoGeneration,
            generation(
                "workspace-lexical-owner-without-parser-selectors",
                1,
                lexical_owner,
            ),
        )
        .await
        .expect("publish lexical-only owner generation");

    let pointer = resident_pointer(
        temporary.path(),
        "workspace-lexical-owner-without-parser-selectors",
    );
    let client = WorkspaceSearchGenerationDataPlaneClient::open(
        &pointer,
        &super::project_root("workspace-lexical-owner-without-parser-selectors"),
    )
    .await
    .expect("open lexical-only search generation");
    let owner_paths =
        vec!["build-support/asp-rust-project-harness-policy/src/member_policy.rs".to_owned()];

    assert!(
        client
            .parser_owned_callable_selector_pairs(&owner_paths)
            .expect("project optional parser-owned callable selectors")
            .is_empty(),
        "a lexical owner must not invent a callable selector"
    );
    let (projections, relations, diagnostics) = client
        .native_syntax_playbook_projection(&owner_paths)
        .expect("project an explicit empty native-syntax capability");
    assert!(projections.is_empty());
    assert!(relations.is_empty());
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].owner_path, owner_paths[0]);
    assert_eq!(diagnostics[0].reason_kind, "source-syntax-unavailable");
    assert!(!diagnostics[0].content_digest.is_empty());
    let native_syntax_stage = agent_semantic_search::build_native_syntax_stage_with_diagnostics(
        agent_semantic_search::SearchGenerationIdentity {
            project_id: "project-native-syntax-diagnostic".to_owned(),
            workspace_id: "workspace-lexical-owner-without-parser-selectors".to_owned(),
            source_root_digest: format!("blake3-256:{}", "1".repeat(64)),
            provider_digest: format!("blake3-256:{}", "2".repeat(64)),
            schema_digest: format!("blake3-256:{}", "3".repeat(64)),
            generation_candidate_digest: format!("blake3-256:{}", "4".repeat(64)),
        },
        projections,
        relations,
        diagnostics,
    )
    .expect("diagnosed native syntax remains a complete stage");
    assert_eq!(
        native_syntax_stage.stage,
        agent_semantic_search::SearchGenerationConstructionStage::NativeSyntax
    );
    assert!(native_syntax_stage.complete);
}
