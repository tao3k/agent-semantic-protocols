// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::{SourceIndexRecoveryExecution, recover_unchanged_generation};
use crate::server_source_index::generation_build::SourceIndexGenerationRefresh;
use agent_semantic_content_identity::{SourceSnapshotKind, WorkspaceSnapshot};

fn digest(value: &str) -> String {
    format!("blake3-256:{}", blake3::hash(value.as_bytes()).to_hex())
}

#[tokio::test]
async fn durable_recovery_requires_current_content_and_execution() {
    let temp = tempfile::tempdir().unwrap();
    let project_root = temp.path().join("project");
    std::fs::create_dir_all(&project_root).unwrap();
    let client_dir = temp.path().join("client");
    let workspace = "workspace-recovery-test";
    let source = b"// parser fixture with no declarations\n";
    let blobs = crate::ClientDbSourceIndexSourceBlobs::from_normalized([(
        crate::ClientDbSourceIndexPath::new("src/lib.rs"),
        source.to_vec(),
    )]);
    let snapshot = WorkspaceSnapshot::from_file_bytes(blobs.iter());
    let evidence = snapshot.evidence(SourceSnapshotKind::Filesystem, digest("provider-and-scope"));
    let import = crate::build_source_index_import(crate::ClientDbSourceIndexImportRequest {
        generation_id: crate::client_db_source_index_generation_id_for_snapshot(&evidence),
        project_root: project_root.clone(),
        schema_id: crate::server_source_index::config::SOURCE_INDEX_SCHEMA_ID.into(),
        schema_version: crate::server_source_index::config::SOURCE_INDEX_SCHEMA_VERSION.into(),
        selector_source: crate::server_source_index::config::SOURCE_INDEX_PROVIDER_ID.into(),
        file_hashes: vec![agent_semantic_client_core::ClientCacheFileHash {
            path: "src/lib.rs".into(),
            sha256: String::new(),
            byte_len: source.len() as u64,
            mtime_ms: 0,
        }],
        source_blobs: blobs.clone(),
        files: vec![crate::ClientDbSourceIndexImportFile {
            relative_path: "src/lib.rs".into(),
            language_id: "rust".into(),
            provider_id: "asp-rust".into(),
            text: String::from_utf8(source.to_vec()).unwrap(),
            selectors: Vec::new(),
            relations: Vec::new(),
        }],
    })
    .unwrap();
    let execution = SourceIndexRecoveryExecution {
        runtime_bundle_digest: digest("runtime"),
        schema_bundle_digest: digest("schema"),
        workspace_closure_digest: digest("closure"),
    };
    let mut materialization =
        crate::runtime_server_workspace::WorkspaceCanonicalMaterialization::from_source_index(
            workspace,
            &snapshot,
            &evidence,
            &import,
            &blobs,
            Vec::new(),
        )
        .unwrap();
    materialization.bind_runtime_provider_execution(
        agent_semantic_artifacts::runtime_provider_execution_binding::RuntimeProviderExecutionBinding::build(
            "repo-recovery".into(), workspace.into(), execution.runtime_bundle_digest.clone(),
            execution.schema_bundle_digest.clone(), execution.workspace_closure_digest.clone(),
            evidence.root_integrity_reference().unwrap(), materialization.import_digest.clone(),
        ).unwrap(),
    ).unwrap();
    let refresh = crate::ClientDbSourceIndexRefreshRequest {
        import,
        file_count: 1,
        source_snapshot: evidence.clone(),
    };
    let expected_import = serde_json::to_value(&refresh.import).unwrap();
    let fixture_dir = client_dir.clone();
    tokio::task::spawn_blocking(move || {
        crate::fixture::SourceIndexFixture::for_client_dir_with_workspace_identity(
            &fixture_dir,
            workspace,
        )
        .commit_source_index_generation_with_materialization(refresh, materialization)
        .unwrap();
    })
    .await
    .unwrap();

    let db_path = crate::ClientDbEngine::db_path_for_client_dir(&client_dir);
    let active = crate::active_turso_source_index_generation(
        &db_path,
        &project_root,
        &crate::server_source_index::config::SOURCE_INDEX_SCHEMA_ID.into(),
        &crate::server_source_index::config::SOURCE_INDEX_SCHEMA_VERSION.into(),
    )
    .await
    .unwrap()
    .unwrap();
    let reconstructed = crate::source_index::recovered_source_index_import(
        &active.snapshot,
        &project_root,
        crate::server_source_index::config::SOURCE_INDEX_SCHEMA_ID.into(),
        crate::server_source_index::config::SOURCE_INDEX_SCHEMA_VERSION.into(),
        blobs.clone(),
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(&reconstructed).unwrap(),
        expected_import
    );
    let candidate = crate::runtime_server_admission::WorkspaceGenerationCandidateIdentity::for_runtime_admission("repo-recovery", workspace, None).unwrap();
    let registry = agent_semantic_client_core::RuntimeProviderProjection {
        authority_ref: "fixture".into(),
        providers: Vec::new(),
    };
    let registry_evidence = registry.evidence(&project_root);
    let files = vec![crate::ClientDbSourceIndexScopeFile {
        path: project_root.join("src/lib.rs"),
        language_id: "rust".into(),
        provider_id: "asp-rust".into(),
        projection_coverage: crate::ClientDbSourceIndexProjectionCoverage::Complete,
        projection_diagnostic: None,
        selector_receipts: Vec::new(),
        relations: Vec::new(),
    }];
    let request = SourceIndexGenerationRefresh {
        recovery_execution: Some(&execution),
        changed_owner_paths: None,
        replacement_authority: None,
        index_root: &project_root,
        files: &files,
        project_resolutions: &[],
        candidate: &candidate,
        registry: &registry_evidence,
        provider_registry: &registry,
    };
    let recovered = recover_unchanged_generation(&db_path, &request, workspace, &evidence, &blobs)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(recovered.materialization().owners.len(), 1);
    assert_eq!(recovered.materialization().owners[0].bytes, source);
    for changed in [
        snapshot.evidence(
            SourceSnapshotKind::Filesystem,
            digest("changed-provider-or-scope"),
        ),
        WorkspaceSnapshot::from_file_bytes([("src/lib.rs", b"changed".as_slice())])
            .evidence(SourceSnapshotKind::Filesystem, digest("provider-and-scope")),
        WorkspaceSnapshot::from_file_bytes([
            ("src/added.rs", source.as_slice()),
            ("src/lib.rs", source.as_slice()),
        ])
        .evidence(SourceSnapshotKind::Filesystem, digest("provider-and-scope")),
    ] {
        assert!(
            recover_unchanged_generation(&db_path, &request, workspace, &changed, &blobs)
                .await
                .unwrap()
                .is_none()
        );
    }
    for axis in 0..3 {
        let mut changed_execution = execution.clone();
        match axis {
            0 => changed_execution.runtime_bundle_digest = digest("changed"),
            1 => changed_execution.schema_bundle_digest = digest("changed"),
            _ => changed_execution.workspace_closure_digest = digest("changed"),
        }
        let changed_request = SourceIndexGenerationRefresh {
            recovery_execution: Some(&changed_execution),
            ..request
        };
        assert!(
            recover_unchanged_generation(&db_path, &changed_request, workspace, &evidence, &blobs)
                .await
                .unwrap()
                .is_none()
        );
    }
    let corrupt_bytes = crate::ClientDbSourceIndexSourceBlobs::from_normalized([(
        crate::ClientDbSourceIndexPath::new("src/lib.rs"),
        b"corrupt bytes under unchanged evidence".to_vec(),
    )]);
    assert!(
        recover_unchanged_generation(&db_path, &request, workspace, &evidence, &corrupt_bytes)
            .await
            .is_err()
    );
}
