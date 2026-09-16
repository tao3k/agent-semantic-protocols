// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use agent_semantic_client_db::WorkspaceDbRegistry;
use agent_semantic_client_db::runtime_server::RuntimeServer;
use agent_semantic_client_db::runtime_server::RuntimeServerExit;
use agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmission;
use agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationCandidateBuild;
use agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationCandidateBuilder;
use agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationCandidateIdentity;
use agent_semantic_client_db::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalog;
use agent_semantic_client_db::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalogEntry;
use agent_semantic_client_db::runtime_server_control::RuntimeServerState;
use agent_semantic_client_db::runtime_server_control::prepare_runtime_server_endpoint;

async fn fixture_endpoint(
    runtime_dir: &tempfile::TempDir,
    epoch: u64,
) -> (
    agent_semantic_client_db::RuntimeServerEndpoint,
    Arc<agent_semantic_artifacts::runtime_artifact_catalog::RuntimeArtifactCatalog>,
) {
    let catalog = agent_semantic_artifacts::runtime_artifact_catalog::RuntimeArtifactCatalog::new(
        agent_semantic_config::runtime_dev::RuntimeArtifactMode::Release,
    );
    let endpoint = prepare_runtime_server_endpoint(
        runtime_dir.path(),
        std::path::Path::new("/runtime/asp"),
        &agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(
            b"runtime-digest",
        ),
        catalog.mode_label(),
        &catalog.digest(),
        epoch,
        &format!("binding-{epoch}"),
    )
    .await
    .expect("prepare isolated runtime server endpoint");
    (endpoint, Arc::new(catalog))
}

fn run_git(root: &std::path::Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .expect("run git fixture command");
    assert!(
        output.status.success(),
        "git fixture command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn candidate_build(
    workspace_identity: &str,
    project_root: &std::path::Path,
    candidate: WorkspaceGenerationCandidateIdentity,
) -> WorkspaceGenerationCandidateBuild {
    let bytes = b"pub fn process_cold_restore() -> u8 { 1 }\n";
    let source_snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_bytes([(
        "src/lib.rs",
        bytes.as_slice(),
    )])
    .evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        format!(
            "blake3-256:{}",
            blake3::hash(b"process-cold-restore-provider").to_hex()
        ),
    );
    let import = agent_semantic_client_db::ClientDbSourceIndexImport {
        source_blobs: Default::default(),
        relations: Vec::new(),
        generation_id: agent_semantic_client_core::CacheGenerationId::from(
            "process-cold-restore-fixture",
        ),
        project_root: project_root.to_path_buf(),
        schema_id: agent_semantic_client_core::SemanticSchemaId::from(
            agent_semantic_client_db::CLIENT_DB_SOURCE_INDEX_SCHEMA_ID,
        ),
        schema_version: agent_semantic_client_core::SemanticSchemaVersion::from(
            agent_semantic_client_db::CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION,
        ),
        file_hashes: vec![agent_semantic_client_core::ClientCacheFileHash {
            path: "src/lib.rs".to_owned(),
            sha256: blake3::hash(bytes).to_hex().to_string(),
            byte_len: bytes.len() as u64,
            mtime_ms: 1,
        }],
        owners: vec![agent_semantic_client_db::ClientDbSourceIndexOwner {
            owner_path: "src/lib.rs".into(),
            language_id: None,
            provider_id: None,
            source_kind: "file".into(),
            line_count: Some(1),
            query_keys: Vec::new(),
        }],
        selectors: Vec::new(),
    };
    let mut materialization =
        agent_semantic_client_db::runtime_server_workspace::WorkspaceCanonicalMaterialization::new(
            workspace_identity,
            source_snapshot.clone(),
            &import,
            [1, 0],
            vec![
                agent_semantic_client_db::runtime_server_workspace::WorkspaceOwnerSnapshot {
                    authority: None,
                    owner_path: "src/lib.rs".to_owned(),
                    content_digest: format!("blake3-256:{}", blake3::hash(bytes).to_hex()),
                    bytes: bytes.to_vec(),
                    native_syntax_diagnostic: None,
                    selectors: Vec::new(),
                },
            ],
            Vec::new(),
        )
        .expect("construct process-cold canonical materialization");
    materialization
        .attach_content_search_generation(crate::fixture::content_search_generation_receipt(
            workspace_identity,
            &source_snapshot,
        ))
        .expect("attach process-cold content search generation");
    let source_index_digest = materialization.import_digest.clone();
    materialization
        .bind_runtime_provider_execution(crate::fixture::runtime_provider_execution_binding(
            workspace_identity,
            &source_snapshot,
            &source_index_digest,
        ))
        .expect("bind process-cold Runtime provider execution");
    WorkspaceGenerationCandidateBuild::new(
        candidate,
        agent_semantic_client_db::ClientDbSourceIndexRefreshRequest {
            import,
            file_count: 1,
            source_snapshot,
        },
        materialization,
    )
}

fn runtime_bundle_probe() -> Arc<dyn Fn() -> Result<Option<String>, String> + Send + Sync + 'static>
{
    Arc::new(|| Ok(Some(format!("blake3-256:{}", "1".repeat(64)))))
}

fn counting_builder(build_count: Arc<AtomicUsize>) -> WorkspaceGenerationCandidateBuilder {
    Arc::new(
        move |workspace_identity,
              project_root,
              candidate,
              _changed_paths,
              provider_target,
              _cancellation| {
            let build_count = Arc::clone(&build_count);
            Box::pin(async move {
                assert!(provider_target.is_none());
                build_count.fetch_add(1, Ordering::SeqCst);
                Ok(candidate_build(
                    &workspace_identity,
                    &project_root,
                    candidate,
                ))
            })
        },
    )
}

async fn stop_unserved_server(server: &RuntimeServer) {
    if let Some(admission) = server.workspace_generation_admission() {
        admission.shutdown().await.expect("stop admission worker");
    }
    server
        .workspace_registry()
        .shutdown()
        .await
        .expect("stop workspace writer lane");
}

#[tokio::test(flavor = "multi_thread")]
async fn daemon_startup_does_not_eagerly_restore_registered_workspaces() {
    let runtime_dir = tempfile::tempdir().expect("create isolated runtime server directory");
    let project_root = runtime_dir.path().join("stale-workspace");
    tokio::fs::create_dir_all(&project_root)
        .await
        .expect("create catalog project root");
    let catalog = RuntimeWorkspaceAdmissionCatalog::load(
        runtime_dir.path().join("workspace-admissions.v1.json"),
    )
    .await
    .expect("load admission catalog");
    catalog
        .record(RuntimeWorkspaceAdmissionCatalogEntry {
            project_id: "repo-stale".to_owned(),
            workspace_identity: "workspace-stale".to_owned(),
            project_root,
        })
        .await
        .expect("record stale scope");
    let (endpoint, artifact_catalog) = fixture_endpoint(&runtime_dir, 31).await;
    let build_count = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let builder_count = Arc::clone(&build_count);
    let admission = WorkspaceGenerationAdmission::new(Arc::new(
        move |_, _, _, _, _changed_paths, _provider_target, _cancellation| {
            builder_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Box::pin(async move {
                Err(agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationBuildFailure::new(
                    agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationFailureStage::DurableRestore,
                    "fixture canonical generation missing",
                ))
            })
        },
    ))
    .with_catalog(catalog);
    let server = RuntimeServer::bind_with_catalog(
        endpoint.clone(),
        Arc::new(WorkspaceDbRegistry::with_state_home(
            runtime_dir.path().join("state"),
        )),
        artifact_catalog,
    )
    .await
    .expect("bind Runtime Server")
    .with_workspace_generation_admission(Arc::new(admission));
    let shutdown = server.shutdown_handle();
    let mut readiness = server.readiness_subscribe();
    let server = tokio::spawn(server.serve());

    if *readiness.borrow() != RuntimeServerState::Healthy {
        readiness
            .changed()
            .await
            .expect("observe Runtime Server Healthy state");
    }
    assert_eq!(*readiness.borrow(), RuntimeServerState::Healthy);
    assert_eq!(
        build_count.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "daemon startup must not restore or build catalog workspaces eagerly"
    );

    shutdown.shutdown();
    assert_eq!(
        server.await.expect("join Runtime Server").expect("serve"),
        RuntimeServerExit::ShutdownRequested
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn unchanged_process_cold_generation_restores_without_invoking_source_builder() {
    let runtime_dir = tempfile::tempdir().expect("process-cold runtime directory");
    let fixture_parent = std::env::current_dir()
        .expect("current repository root")
        .join("target/runtime-server-generation-restore-fixtures");
    std::fs::create_dir_all(&fixture_parent).expect("create process-cold fixture parent");
    let project = tempfile::Builder::new()
        .prefix("asp-process-cold-")
        .tempdir_in(fixture_parent)
        .expect("process-cold Git workspace");
    let project_root = project
        .path()
        .canonicalize()
        .expect("canonical project root");
    run_git(&project_root, &["init", "--quiet"]);
    std::fs::create_dir_all(project_root.join("src")).expect("create source directory");
    std::fs::write(
        project_root.join("src/lib.rs"),
        "pub fn process_cold_restore() -> u8 { 1 }\n",
    )
    .expect("write source owner");
    run_git(&project_root, &["add", "src/lib.rs"]);

    let workspace_identity =
        agent_semantic_client_db::AgentSessionRegistry::workspace_id(&project_root)
            .expect("derive workspace identity");
    let catalog = RuntimeWorkspaceAdmissionCatalog::load(
        runtime_dir.path().join("workspace-admissions.v1.json"),
    )
    .await
    .expect("load workspace admission catalog");
    catalog
        .record(
            RuntimeWorkspaceAdmissionCatalogEntry::resolve(
                workspace_identity.clone(),
                project_root.clone(),
            )
            .expect("resolve project/workspace binding"),
        )
        .await
        .expect("record project/workspace binding");
    let state_home = runtime_dir.path().join("state");

    let first_builds = Arc::new(AtomicUsize::new(0));
    let (first_endpoint, first_artifacts) = fixture_endpoint(&runtime_dir, 41).await;
    let first = RuntimeServer::bind_with_catalog(
        first_endpoint,
        Arc::new(WorkspaceDbRegistry::with_state_home(&state_home)),
        first_artifacts,
    )
    .await
    .expect("bind first Runtime Server")
    .with_workspace_generation_builder_catalog_and_runtime_bundle_probe(
        counting_builder(Arc::clone(&first_builds)),
        catalog.clone(),
        runtime_bundle_probe(),
    );
    first
        .workspace_generation_admission()
        .expect("first generation admission")
        .ensure_runtime_generation_ready(workspace_identity.clone(), project_root.clone())
        .await
        .expect("publish first generation");
    assert_eq!(first_builds.load(Ordering::SeqCst), 1);
    stop_unserved_server(&first).await;
    drop(first);

    let restored_builds = Arc::new(AtomicUsize::new(0));
    let (restored_endpoint, restored_artifacts) = fixture_endpoint(&runtime_dir, 42).await;
    let restored = RuntimeServer::bind_with_catalog(
        restored_endpoint,
        Arc::new(WorkspaceDbRegistry::with_state_home(&state_home)),
        restored_artifacts,
    )
    .await
    .expect("bind restored Runtime Server")
    .with_workspace_generation_builder_catalog_and_runtime_bundle_probe(
        counting_builder(Arc::clone(&restored_builds)),
        catalog.clone(),
        runtime_bundle_probe(),
    );
    let restore_started = std::time::Instant::now();
    let restored_receipt = restored
        .workspace_generation_admission()
        .expect("restored generation admission")
        .ensure_runtime_generation_ready(workspace_identity.clone(), project_root.clone())
        .await
        .expect("restore unchanged generation");
    let restore_elapsed = restore_started.elapsed();
    assert!(restored_receipt.commit.is_some());
    assert_eq!(
        restored_builds.load(Ordering::SeqCst),
        0,
        "an exact process-cold binding must restore mmap state without parser/source rebuild"
    );
    eprintln!(
        "process-cold-generation-restore elapsedMicros={} sourceBuilderCount=0 parserInvocationCount=0 providerProcessCount=0",
        restore_elapsed.as_micros()
    );

    let generation_directory =
        agent_semantic_client_db::runtime_server_workspace::workspace_generation_directory(
            restored.workspace_registry().root(),
            &workspace_identity,
            &project_root,
        )
        .expect("resolve generation directory");
    stop_unserved_server(&restored).await;
    drop(restored);
    tokio::fs::write(
        generation_directory.join("generation-admission-binding.v1.json"),
        b"{truncated",
    )
    .await
    .expect("corrupt admission binding");

    let rebuilt_count = Arc::new(AtomicUsize::new(0));
    let (rebuilt_endpoint, rebuilt_artifacts) = fixture_endpoint(&runtime_dir, 43).await;
    let rebuilt = RuntimeServer::bind_with_catalog(
        rebuilt_endpoint,
        Arc::new(WorkspaceDbRegistry::with_state_home(&state_home)),
        rebuilt_artifacts,
    )
    .await
    .expect("bind rebuilding Runtime Server")
    .with_workspace_generation_builder_catalog_and_runtime_bundle_probe(
        counting_builder(Arc::clone(&rebuilt_count)),
        catalog,
        runtime_bundle_probe(),
    );
    rebuilt
        .workspace_generation_admission()
        .expect("rebuilding generation admission")
        .ensure_runtime_generation_ready(workspace_identity, project_root)
        .await
        .expect("rebuild after corrupt binding");
    assert_eq!(
        rebuilt_count.load(Ordering::SeqCst),
        1,
        "corrupt admission evidence must fail closed to a complete rebuild"
    );
    stop_unserved_server(&rebuilt).await;
}
