use agent_semantic_client_core::{
    CacheGenerationId, ClientCacheFileHash, LanguageId, ProviderId, SemanticSchemaId,
    SemanticSchemaVersion,
};
use agent_semantic_client_db::{
    CLIENT_DB_SOURCE_INDEX_PROVIDER_ID, CLIENT_DB_SOURCE_INDEX_SCHEMA_ID,
    CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION, ClientDbSourceIndexImport, ClientDbSourceIndexOwner,
    ClientDbSourceIndexPath, ClientDbSourceIndexQueryKey, ClientDbSourceIndexRefreshRequest,
    ClientDbSourceIndexSelector, ClientDbSourceIndexSource, ClientDbSourceIndexSourceBlobs,
};

fn fixture_root() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "asp-turso-materialization-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock after Unix epoch")
            .as_nanos()
    ))
}

fn generation_fixture(
    project_root: &std::path::Path,
) -> (
    ClientDbSourceIndexRefreshRequest,
    ClientDbSourceIndexSourceBlobs,
) {
    let owner_path = "src/materialized.rs";
    let selector = "rust://src/materialized.rs#item/function/materialized";
    let source = b"pub fn materialized() {}\n";
    let import = ClientDbSourceIndexImport {
        relations: Vec::new(),
        generation_id: CacheGenerationId::from("materialized-generation"),
        project_root: project_root.to_path_buf(),
        schema_id: SemanticSchemaId::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_ID),
        schema_version: SemanticSchemaVersion::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION),
        file_hashes: vec![ClientCacheFileHash {
            path: owner_path.to_owned(),
            sha256: "a".repeat(64),
            byte_len: source.len() as u64,
            mtime_ms: 1,
        }],
        owners: vec![ClientDbSourceIndexOwner {
            owner_path: ClientDbSourceIndexPath::new(owner_path),
            language_id: Some(LanguageId::from("rust")),
            provider_id: Some(ProviderId::from("rs-harness")),
            source_kind: ClientDbSourceIndexSource::from(CLIENT_DB_SOURCE_INDEX_PROVIDER_ID),
            line_count: Some(1),
            query_keys: vec![ClientDbSourceIndexQueryKey::from("materialized")],
        }],
        selectors: vec![ClientDbSourceIndexSelector {
            owner_path: ClientDbSourceIndexPath::new(owner_path),
            provider_id: ProviderId::from("rs-harness"),
            selector_id: selector.into(),
            symbol: Some("materialized".into()),
            kind: Some("function".into()),
            source: ClientDbSourceIndexSource::from(CLIENT_DB_SOURCE_INDEX_PROVIDER_ID),
            query_keys: vec![ClientDbSourceIndexQueryKey::from("materialized")],
            derived_projections: vec![
                crate::projection_fixture::callable_skeleton_projection_fixture(
                    owner_path,
                    selector,
                    "materialized",
                ),
            ],
            projection_record: crate::projection_fixture::projection_record(
                crate::projection_fixture::ProjectionFixtureInput {
                    language_id: "rust",
                    provider_id: "rs-harness",
                    owner_path,
                    structural_selector: selector,
                    item_kind: "function",
                    item_name: "materialized",
                    source,
                    source_byte_start: 0,
                    source_byte_end: source.len() as u64,
                },
            ),
        }],
    };
    let source_blobs =
        crate::projection_fixture::source_blobs_fixture([(owner_path, source.as_slice())]);
    let source_snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes([(
        owner_path,
        blake3::hash(source).to_hex().to_string(),
    )])
    .evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        "materialization-fixture-provider",
    );
    (
        ClientDbSourceIndexRefreshRequest {
            import,
            file_count: 1,
            source_snapshot,
        },
        source_blobs,
    )
}

fn admitted_project_resolution(
    dependency_name: &str,
) -> agent_semantic_runtime::AdmittedProjectResolution {
    let scope = serde_json::from_value(serde_json::json!({
        "schemaId": "agent.semantic-protocols.project-resolution",
        "schemaVersion": "1",
        "state": "resolved",
        "completeness": "exact",
        "languageId": "rust",
        "providerId": "rs-harness",
        "parserId": "rust.cargo-toml",
        "candidateGenerationDigest": "blake3-256:candidates",
        "projectEntry": "Cargo.toml",
        "packageGraph": {
            "schemaId": "agent.semantic-protocols.language-package-graph",
            "schemaVersion": "1",
            "languageId": "rust",
            "providerId": "rs-harness",
            "projectEntry": "Cargo.toml",
            "parserId": "rust.cargo-toml",
            "manifests": [],
            "lockfiles": [],
            "packages": [],
            "internalDependencyEdges": [],
            "externalDependencies": [{
                "dependencyId": dependency_name,
                "name": dependency_name,
                "kind": "normal"
            }],
            "unresolved": []
        },
        "sourceScopes": [],
        "conflicts": [],
        "metrics": {
            "parsedManifestCount": 1,
            "parsedLockfileCount": 0,
            "affectedPackageCount": 0,
            "fullWorkspaceReads": 0,
            "fullManifestReparses": 0,
            "dbOpens": 0,
            "elapsedMicros": 1
        }
    }))
    .expect("typed ProjectResolution fixture");
    agent_semantic_runtime::AdmittedProjectResolution::new(".", scope)
        .expect("admit ProjectResolution fixture")
}

#[test]
fn project_resolution_graph_is_part_of_canonical_and_mmap_generation_identity() {
    let root = fixture_root();
    let project_root = root.join("project-resolution-generation");
    std::fs::create_dir_all(&project_root).expect("create ProjectResolution fixture root");
    let (request, source_blobs) = generation_fixture(&project_root);
    let source_snapshot = request.source_snapshot.clone();
    let first = agent_semantic_client_db::runtime_server_workspace::WorkspaceCanonicalMaterialization::from_source_index(
        "workspace-project-resolution-generation",
        &source_snapshot,
        &request.import,
        &source_blobs,
        vec![admitted_project_resolution("serde")],
    )
    .expect("first ProjectResolution materialization")
    .into_generation(0)
    .expect("first resident generation");
    let second = agent_semantic_client_db::runtime_server_workspace::WorkspaceCanonicalMaterialization::from_source_index(
        "workspace-project-resolution-generation",
        &source_snapshot,
        &request.import,
        &source_blobs,
        vec![admitted_project_resolution("tokio")],
    )
    .expect("second ProjectResolution materialization")
    .into_generation(0)
    .expect("second resident generation");

    assert_ne!(
        first.workspace_source_scope_generation,
        second.workspace_source_scope_generation
    );
    assert_ne!(first.memory_backend_digest, second.memory_backend_digest);
    assert_ne!(first.generation_digest, second.generation_digest);
    let restored: agent_semantic_client_db::runtime_server_workspace::WorkspaceMemoryGeneration =
        serde_json::from_slice(&serde_json::to_vec(&first).expect("encode mmap generation"))
            .expect("restore mmap generation");
    restored.validate().expect("validate restored generation");
    assert_eq!(restored.project_resolutions, first.project_resolutions);

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn turso_generation_materialization_is_reusable_and_drift_is_fail_closed() {
    let root = fixture_root();
    let client_dir = root.join("client");
    let project_root = root.join("project");
    std::fs::create_dir_all(&project_root).expect("create canonical project root");
    let fixture =
        agent_semantic_client_db::fixture::SourceIndexFixture::for_client_dir(&client_dir);
    let (request, source_blobs) = generation_fixture(&project_root.join("."));

    let first = fixture
        .commit_source_index_generation(request.clone(), &source_blobs)
        .expect("commit canonical generation and materialization");
    assert!(!first.reused_generation);

    let active = fixture
        .load_active_workspace_generation_materialization(&project_root)
        .expect("load active materialization")
        .expect("active materialization must exist");
    assert_eq!(
        active.source_snapshot.root_digest,
        first.source_snapshot.root_digest
    );
    assert_eq!(
        active.workspace_snapshot.root_digest(),
        active.source_snapshot.root_digest
    );
    assert_eq!(
        active.workspace_generation.root_digest,
        active.source_snapshot.root_digest
    );
    assert_eq!(active.workspace_generation.leaf_count, 1);
    assert_eq!(active.workspace_generation.owner_count, 1);
    assert_eq!(active.root_depth, [1, 0]);
    assert_eq!(active.owners.len(), 1);
    assert_eq!(
        active.owners[0].selectors[0].derived_projections[0].projection_kind,
        "callable-skeleton"
    );
    assert_eq!(
        active.project_root,
        std::fs::canonicalize(&project_root)
            .expect("canonicalize materialization project root")
            .display()
            .to_string()
    );

    let reused = fixture
        .commit_source_index_generation(request.clone(), &source_blobs)
        .expect("reuse identical canonical materialization");
    assert!(reused.reused_generation);
    assert_eq!(reused.generation_id, first.generation_id);

    let mut drifted = active.clone();
    drifted.owners[0].bytes = b"pub fn forged_value() {}\n".to_vec();
    drifted.owners[0].content_digest = format!(
        "blake3-256:{}",
        blake3::hash(&drifted.owners[0].bytes).to_hex()
    );
    let error = fixture
        .commit_source_index_generation_with_materialization(request, drifted)
        .expect_err("forged exact owner bytes must fail closed");
    assert!(
        error.contains("proof source drift") || error.contains("proof projection drift"),
        "unexpected materialization drift error: {error}"
    );

    let after_failure = fixture
        .load_active_workspace_generation_materialization(&project_root)
        .expect("reload active materialization")
        .expect("active materialization must remain visible");
    assert_eq!(after_failure, active);
    let counters = fixture.counters();
    assert_eq!(counters.database_open_count, 1);
    assert_eq!(counters.schema_bootstrap_count, 1);
    assert_eq!(counters.workspace_lock_retry_count, 0);
    assert_eq!(counters.max_active_writer_count, 1);

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn one_workspace_persists_identical_generation_ids_per_project_resolution() {
    let root = fixture_root();
    let client_dir = root.join("client");
    let first_root = root.join("packages/first");
    let second_root = root.join("packages/second");
    std::fs::create_dir_all(&first_root).expect("create first project scope");
    std::fs::create_dir_all(&second_root).expect("create second project scope");
    let workspace_identity = "workspace-monorepo";
    let fixture =
        agent_semantic_client_db::fixture::SourceIndexFixture::for_client_dir_with_workspace_identity(
            &client_dir,
            workspace_identity,
        );

    for project_root in [&first_root, &second_root] {
        let (mut request, source_blobs) = generation_fixture(project_root);
        let source_snapshot = request.source_snapshot.clone();
        request.source_snapshot = source_snapshot.clone();
        let materialization =
            agent_semantic_client_db::runtime_server_workspace::WorkspaceCanonicalMaterialization::from_source_index(
                workspace_identity,
                &source_snapshot,
                &request.import,
                &source_blobs,
                Vec::new(),
            )
            .expect("materialize scoped generation");
        fixture
            .commit_source_index_generation_with_materialization(request, materialization)
            .expect("commit scoped generation");
    }

    let first = fixture
        .load_active_workspace_generation_materialization(&first_root)
        .expect("load first scope")
        .expect("first scope materialization");
    let second = fixture
        .load_active_workspace_generation_materialization(&second_root)
        .expect("load second scope")
        .expect("second scope materialization");

    assert_eq!(
        first.project_root,
        std::fs::canonicalize(&first_root)
            .expect("canonicalize first project scope")
            .display()
            .to_string()
    );
    assert_eq!(
        second.project_root,
        std::fs::canonicalize(&second_root)
            .expect("canonicalize second project scope")
            .display()
            .to_string()
    );
    assert_ne!(first.import_digest, second.import_digest);
    assert_eq!(first.workspace_identity, second.workspace_identity);
    let counters = fixture.counters();
    assert_eq!(counters.database_open_count, 1);
    assert_eq!(counters.schema_bootstrap_count, 1);
    assert_eq!(counters.workspace_lock_retry_count, 0);
    assert_eq!(counters.max_active_writer_count, 1);

    let _ = std::fs::remove_dir_all(root);
}
