use super::support;

fn publish_owner_items_generation(
    root: &std::path::Path,
    owner_path: &str,
    owner_bytes: &[u8],
    structural_selector: &str,
) {
    use agent_semantic_client_db::runtime_server_admission_catalog::{
        RuntimeWorkspaceAdmissionCatalog, RuntimeWorkspaceAdmissionCatalogEntry,
    };
    use agent_semantic_client_db::runtime_server_workspace::{
        RuntimeServerWorkspaceRegistry, WorkspaceGenerationBuild, WorkspaceMemoryGeneration,
        WorkspaceOwnerSnapshot, WorkspaceRecoverySource, WorkspaceSelectorSnapshot,
    };

    let canonical_root = root.canonicalize().expect("canonical fixture root");
    let workspace_identity = "workspace-owner-items-fixture";
    let state_home = support::state_home(root);
    let server_root = state_home.join("runtime").join("server");
    let catalog_path = server_root.join("workspace-admissions.v1.json");
    let workspace_store_root = server_root.join("workspaces");
    let content_digest = format!("blake3-256:{}", blake3::hash(owner_bytes).to_hex());
    let workspace_snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes(
        [(owner_path, content_digest.clone())],
    );
    let source_snapshot = workspace_snapshot.evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        format!(
            "blake3-256:{}",
            blake3::hash(b"owner-items-resident-fixture-provider").to_hex()
        ),
    );
    let generation = WorkspaceMemoryGeneration::try_from_build(WorkspaceGenerationBuild {
        relations: Vec::new(),
        workspace_identity: workspace_identity.to_owned(),
        project_root: canonical_root.display().to_string(),
        active_epoch: 1,
        workspace_snapshot,
        source_snapshot,
        module_graph_digest: format!(
            "blake3-256:{}",
            blake3::hash(b"owner-items-resident-fixture-module-graph").to_hex()
        ),
        project_resolutions: Vec::new(),
        owners: vec![WorkspaceOwnerSnapshot {
            owner_path: owner_path.to_owned(),
            content_digest,
            bytes: owner_bytes.to_vec(),
            selectors: vec![WorkspaceSelectorSnapshot {
                selector: structural_selector.to_owned(),
                byte_start: 0,
                byte_end: owner_bytes.len(),
                derived_projections: Vec::new(),
            }],
        }],
    })
    .expect("typed owner-items resident generation");

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("owner-items fixture runtime")
        .block_on(async move {
            let catalog = RuntimeWorkspaceAdmissionCatalog::load(catalog_path)
                .await
                .expect("workspace admission catalog");
            catalog
                .record(RuntimeWorkspaceAdmissionCatalogEntry {
                    workspace_identity: workspace_identity.to_owned(),
                    project_root: canonical_root,
                })
                .await
                .expect("publish workspace admission");
            let registry = RuntimeServerWorkspaceRegistry::new(workspace_store_root)
                .expect("workspace registry");
            registry
                .publish(
                    "owner-items-fixture",
                    WorkspaceRecoverySource::TursoGeneration,
                    generation,
                )
                .await
                .expect("publish owner-items generation");
            registry.shutdown().await.expect("drain fixture writer");
        });
}

#[test]
fn search_owner_items_phrase_hit_attributes_to_parser_item() {
    let root = support::temp_project_root("search-owner-items-phrase-attribution");
    std::fs::create_dir_all(root.join("src")).expect("create src dir");
    let source = b"pub fn agent_session_artifact_activity() {\n    let heartbeat = true;\n}\n";
    std::fs::write(root.join("src/lib.rs"), source).expect("write source");
    support::write_activation(&root, &[support::provider("rust", Vec::new())]);
    publish_owner_items_generation(
        &root,
        "src/lib.rs",
        source,
        "rust://src/lib.rs#item/function/agent_session_artifact_activity",
    );

    let output = support::asp_command(&root)
        .args([
            "rust",
            "search",
            "owner",
            "src/lib.rs",
            "items",
            "--query",
            "heartbeat",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp");

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(
            "structuralSelector=rust://src/lib.rs#item/function/agent_session_artifact_activity"
        ),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("reason=owner-local-source-attribution"),
        "stdout={stdout}"
    );
    assert!(!stdout.contains("item=0"), "stdout={stdout}");

    std::fs::remove_dir_all(root).expect("remove temp root");
}

#[test]
fn query_owner_phrase_hit_attributes_to_parser_item() {
    let root = support::temp_project_root("query-owner-phrase-attribution");
    std::fs::create_dir_all(root.join("src")).expect("create src dir");
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn agent_session_artifact_activity() {\n    let heartbeat = true;\n}\n",
    )
    .expect("write source");
    support::write_activation(&root, &[support::provider("rust", Vec::new())]);

    let output = support::asp_command(&root)
        .args([
            "rust",
            "query",
            "--selector",
            "src/lib.rs",
            "--query",
            "heartbeat",
            "--workspace",
            ".",
            "--names-only",
        ])
        .output()
        .expect("run asp");

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(
            "structuralSelector=rust://src/lib.rs#item/function/agent_session_artifact_activity"
        ),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("status=hit") && stdout.contains("match=exact"),
        "stdout={stdout}"
    );
    assert!(!stdout.contains("item=0"), "stdout={stdout}");

    std::fs::remove_dir_all(root).expect("remove temp root");
}
