// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_client_core::CacheGenerationId;
use agent_semantic_client_core::ClientCacheFileHash;
use agent_semantic_client_core::LanguageId;
use agent_semantic_client_core::ProviderId;
use agent_semantic_client_core::SemanticSchemaId;
use agent_semantic_client_core::SemanticSchemaVersion;
use agent_semantic_client_db::CLIENT_DB_SOURCE_INDEX_PROVIDER_ID;
use agent_semantic_client_db::CLIENT_DB_SOURCE_INDEX_SCHEMA_ID;
use agent_semantic_client_db::CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION;
use agent_semantic_client_db::ClientDbSourceIndexImport;
use agent_semantic_client_db::ClientDbSourceIndexImportFile;
use agent_semantic_client_db::ClientDbSourceIndexImportRequest;
use agent_semantic_client_db::ClientDbSourceIndexOwner;
use agent_semantic_client_db::ClientDbSourceIndexPath;
use agent_semantic_client_db::ClientDbSourceIndexQueryKey;
use agent_semantic_client_db::ClientDbSourceIndexRefreshRequest;
use agent_semantic_client_db::ClientDbSourceIndexSelector;
use agent_semantic_client_db::ClientDbSourceIndexSource;
use agent_semantic_client_db::ClientDbSourceIndexSourceBlobs;

#[test]
fn same_pass_auxiliary_blobs_receive_hashes_without_becoming_searchable_owners() {
    let source = b"pub fn indexed() {}\n";
    let auxiliary = b"package: test\n";
    let import =
        agent_semantic_client_db::build_source_index_import(ClientDbSourceIndexImportRequest {
            generation_id: CacheGenerationId::from("same-pass-auxiliary"),
            project_root: std::path::PathBuf::from("."),
            schema_id: SemanticSchemaId::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_ID),
            schema_version: SemanticSchemaVersion::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION),
            selector_source: ClientDbSourceIndexSource::from(CLIENT_DB_SOURCE_INDEX_PROVIDER_ID),
            file_hashes: vec![ClientCacheFileHash {
                path: "src/indexed.rs".to_owned(),
                sha256: "stale".to_owned(),
                byte_len: 0,
                mtime_ms: 7,
            }],
            source_blobs: ClientDbSourceIndexSourceBlobs::from_normalized([
                (
                    ClientDbSourceIndexPath::new("src/indexed.rs"),
                    source.to_vec(),
                ),
                (
                    ClientDbSourceIndexPath::new("gerbil.pkg"),
                    auxiliary.to_vec(),
                ),
            ]),
            files: vec![ClientDbSourceIndexImportFile {
                relative_path: "src/indexed.rs".to_owned(),
                language_id: LanguageId::from("rust"),
                provider_id: ProviderId::from("asp-rust"),
                text: String::from_utf8(source.to_vec()).expect("UTF-8 fixture"),
                selectors: Vec::new(),
                relations: Vec::new(),
            }],
        })
        .expect("assemble a complete same-pass import");

    assert_eq!(import.owners.len(), 1);
    assert_eq!(import.owners[0].owner_path.as_str(), "src/indexed.rs");
    let auxiliary_hash = import
        .file_hashes
        .iter()
        .find(|file_hash| file_hash.path == "gerbil.pkg")
        .expect("auxiliary blob hash");
    assert_eq!(auxiliary_hash.byte_len, auxiliary.len() as u64);
    assert_eq!(
        auxiliary_hash.sha256,
        format!("{:x}", <sha2::Sha256 as sha2::Digest>::digest(auxiliary))
    );
}

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
        source_blobs: Default::default(),
        relations: Vec::new(),
        generation_id: CacheGenerationId::from("materialized-generation"),
        project_root: project_root.to_path_buf(),
        schema_id: SemanticSchemaId::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_ID),
        schema_version: SemanticSchemaVersion::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION),
        file_hashes: vec![ClientCacheFileHash {
            path: owner_path.to_owned(),
            sha256: format!("{:x}", <sha2::Sha256 as sha2::Digest>::digest(source)),
            byte_len: source.len() as u64,
            mtime_ms: 1,
        }],
        owners: vec![ClientDbSourceIndexOwner {
            owner_path: ClientDbSourceIndexPath::new(owner_path),
            language_id: Some(LanguageId::from("rust")),
            provider_id: Some(ProviderId::from("asp-rust")),
            source_kind: ClientDbSourceIndexSource::from(CLIENT_DB_SOURCE_INDEX_PROVIDER_ID),
            line_count: Some(1),
            query_keys: vec![ClientDbSourceIndexQueryKey::from("materialized")],
        }],
        selectors: vec![ClientDbSourceIndexSelector {
            owner_path: ClientDbSourceIndexPath::new(owner_path),
            provider_id: ProviderId::from("asp-rust"),
            selector_id: selector.into(),
            symbol: Some("materialized".into()),
            kind: Some("function".into()),
            source: ClientDbSourceIndexSource::from(CLIENT_DB_SOURCE_INDEX_PROVIDER_ID),
            query_keys: vec![ClientDbSourceIndexQueryKey::from("materialized")],
            derived_projections: vec![
                crate::projection_fixture::callable_skeleton_projection_fixture(
                    selector,
                    "materialized",
                ),
            ],
            projection_record: crate::projection_fixture::projection_record(
                crate::projection_fixture::ProjectionFixtureInput {
                    language_id: "rust",
                    provider_id: "asp-rust",
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
    let mut import = import;
    import.source_blobs = source_blobs.clone();
    let source_snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes([(
        owner_path,
        blake3::hash(source).to_hex().to_string(),
    )])
    .evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        blake3::hash(b"materialization-provider")
            .to_hex()
            .to_string(),
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
) -> agent_semantic_content_identity::AdmittedProjectResolution {
    let scope = serde_json::from_value(serde_json::json!({
        "schemaId": "agent.semantic-protocols.project-resolution",
        "schemaVersion": "1",
        "state": "resolved",
        "completeness": "exact",
        "languageId": "rust",
        "providerId": "asp-rust",
        "parserId": "rust.cargo-toml",
        "candidateGenerationDigest": "blake3-256:candidates",
        "projectEntry": "Cargo.toml",
        "packageGraph": {
            "schemaId": "agent.semantic-protocols.language-package-graph",
            "schemaVersion": "1",
            "languageId": "rust",
            "providerId": "asp-rust",
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
    agent_semantic_content_identity::AdmittedProjectResolution::new(".", scope)
        .expect("admit ProjectResolution fixture")
}

#[test]
fn canonical_assembly_rejects_source_drift_with_reused_owner_digest() {
    let project_root = fixture_root();
    std::fs::create_dir_all(&project_root).expect("create source drift fixture");
    let (request, _) = generation_fixture(&project_root);
    assert!(!request.import.selectors.is_empty());
    let source_blobs = ClientDbSourceIndexSourceBlobs::from_normalized([(
        ClientDbSourceIndexPath::new("src/materialized.rs"),
        b"pub fn materialized() { }\n".to_vec(),
    )]);
    let workspace_snapshot =
        agent_semantic_content_identity::WorkspaceSnapshot::from_file_bytes(source_blobs.iter());
    let source_snapshot = workspace_snapshot.evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        blake3::hash(b"source-drift-provider").to_hex().to_string(),
    );
    let result = agent_semantic_client_db::runtime_server_workspace::WorkspaceCanonicalMaterialization::from_source_index(
        "workspace-source-drift",
        &workspace_snapshot,
        &source_snapshot,
        &request.import,
        &source_blobs,
        Vec::new(),
    );
    assert!(matches!(result, Err(error) if error.contains("source blob digest drift")));
}

#[test]
fn auxiliary_snapshot_leaves_are_committed_without_becoming_searchable_owners() {
    let root = fixture_root();
    let project_root = root.join("auxiliary-snapshot-generation");
    std::fs::create_dir_all(&project_root).expect("create auxiliary snapshot fixture root");
    let (mut request, _) = generation_fixture(&project_root);
    let source_blobs = ClientDbSourceIndexSourceBlobs::from_normalized([
        (
            ClientDbSourceIndexPath::new("src/materialized.rs"),
            b"pub fn materialized() {}\n".to_vec(),
        ),
        (
            ClientDbSourceIndexPath::new("gerbil.pkg"),
            b"package: test\n".to_vec(),
        ),
    ]);
    request.import.source_blobs = source_blobs.clone();
    let workspace_snapshot =
        agent_semantic_content_identity::WorkspaceSnapshot::from_file_bytes(source_blobs.iter());
    let source_snapshot = workspace_snapshot.evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        blake3::hash(b"auxiliary-snapshot-provider")
            .to_hex()
            .to_string(),
    );

    let materialization = agent_semantic_client_db::runtime_server_workspace::WorkspaceCanonicalMaterialization::from_source_index(
        "workspace-auxiliary-snapshot",
        &workspace_snapshot,
        &source_snapshot,
        &request.import,
        &source_blobs,
        Vec::new(),
    )
    .expect("materialize generation with an auxiliary snapshot leaf");

    assert_eq!(materialization.owners.len(), 1);
    assert_eq!(materialization.owners[0].owner_path, "src/materialized.rs");
    assert_eq!(materialization.auxiliary_owners.len(), 1);
    assert_eq!(materialization.auxiliary_owners[0].owner_path, "gerbil.pkg");
    assert_eq!(
        materialization.auxiliary_owners[0].bytes,
        b"package: test\n"
    );
    assert!(
        materialization
            .workspace_snapshot
            .file_digest("gerbil.pkg")
            .is_some()
    );
    assert_eq!(
        materialization.workspace_snapshot.root_digest(),
        materialization.source_snapshot.root_digest
    );
    let encoded = serde_json::to_vec(&materialization).expect("encode canonical materialization");
    let mut restored = serde_json::from_slice::<
        agent_semantic_client_db::runtime_server_workspace::WorkspaceCanonicalMaterialization,
    >(&encoded)
    .expect("restore canonical materialization");
    assert_eq!(restored.auxiliary_owners, materialization.auxiliary_owners);
    restored.auxiliary_owners[0].bytes.push(b'!');
    assert!(
        restored
            .validate_persisted("workspace-auxiliary-snapshot")
            .expect_err("corrupt persisted auxiliary bytes")
            .contains("auxiliary owner content digest drift")
    );
}

#[test]
fn project_resolution_graph_is_part_of_canonical_and_mmap_generation_identity() {
    let root = fixture_root();
    let project_root = root.join("project-resolution-generation");
    std::fs::create_dir_all(&project_root).expect("create ProjectResolution fixture root");
    let (request, source_blobs) = generation_fixture(&project_root);
    let source_snapshot = request.source_snapshot.clone();
    let workspace_snapshot =
        agent_semantic_content_identity::WorkspaceSnapshot::from_file_bytes(source_blobs.iter());
    let mut first = agent_semantic_client_db::runtime_server_workspace::WorkspaceCanonicalMaterialization::from_source_index(
        "workspace-project-resolution-generation",
        &workspace_snapshot,
        &source_snapshot,
        &request.import,
        &source_blobs,
        vec![admitted_project_resolution("serde")],
    )
    .expect("first ProjectResolution materialization");
    first
        .attach_content_search_generation(crate::fixture::content_search_generation_receipt(
            "workspace-project-resolution-generation",
            &source_snapshot,
        ))
        .expect("bind first content-search construction receipt");
    let first = first.into_generation(0).expect("first resident generation");
    let mut second = agent_semantic_client_db::runtime_server_workspace::WorkspaceCanonicalMaterialization::from_source_index(
        "workspace-project-resolution-generation",
        &workspace_snapshot,
        &source_snapshot,
        &request.import,
        &source_blobs,
        vec![admitted_project_resolution("tokio")],
    )
    .expect("second ProjectResolution materialization");
    second
        .attach_content_search_generation(crate::fixture::content_search_generation_receipt(
            "workspace-project-resolution-generation",
            &source_snapshot,
        ))
        .expect("bind second content-search construction receipt");
    let second = second
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
        agent_semantic_client_db::runtime_server_workspace::ExactProjectionKind::CallableSkeleton
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
        error.contains("incremental owner digest drift")
            || error.contains("proof source drift")
            || error.contains("proof projection drift"),
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
fn relation_admission_failure_does_not_publish_a_partial_generation() {
    let root = fixture_root();
    let client_dir = root.join("client");
    let project_root = root.join("project");
    std::fs::create_dir_all(&project_root).expect("create relation transaction project root");
    let fixture =
        agent_semantic_client_db::fixture::SourceIndexFixture::for_client_dir(&client_dir);
    let (request, source_blobs) = generation_fixture(&project_root);

    fixture
        .commit_source_index_generation(request.clone(), &source_blobs)
        .expect("commit relation transaction base generation");
    let before = fixture
        .load_active_workspace_generation_materialization(&project_root)
        .expect("load relation transaction base materialization")
        .expect("base materialization must exist");

    let mut invalid = request;
    invalid
        .import
        .relations
        .push(agent_semantic_client_db::ClientDbSourceIndexOwnedRelation {
            owner_path: agent_semantic_client_db::ClientDbSourceIndexPath::new(
                "src/missing.rs",
            ),
            relation:
                agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelation {
            from: agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelationEndpoint {
                kind: agent_semantic_content_identity::provider_projection_relation::PROVIDER_RELATION_ITEM_ENDPOINT_KIND.to_owned(),
                id: "rust://src/missing.rs#item/function/missing".into(),
            },
            kind: "calls".into(),
            to: agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelationEndpoint {
                kind: agent_semantic_content_identity::provider_projection_relation::PROVIDER_RELATION_ITEM_ENDPOINT_KIND.to_owned(),
                id: "rust://src/materialized.rs#item/function/materialized".into(),
            },
                },
        });
    let error = fixture
        .commit_source_index_generation(invalid, &source_blobs)
        .expect_err("unattributed relation must fail before generation publication");
    assert!(
        error.contains("outside changed owner membership")
            || error.contains("not present in canonical workspace generation"),
        "unexpected relation admission error: {error}"
    );

    let after = fixture
        .load_active_workspace_generation_materialization(&project_root)
        .expect("reload relation transaction materialization")
        .expect("base materialization must remain visible");
    assert_eq!(after, before);

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn relation_generation_transaction_preserves_replaces_and_deletes() {
    let root = fixture_root();
    let client_dir = root.join("client");
    let project_root = root.join("project");
    std::fs::create_dir_all(&project_root).expect("create relation generation project");
    let fixture =
        agent_semantic_client_db::fixture::SourceIndexFixture::for_client_dir(&client_dir);
    let (mut request, source_blobs) = generation_fixture(&project_root);
    let relation = |kind: &str| {
        agent_semantic_client_db::ClientDbSourceIndexOwnedRelation {
        owner_path: agent_semantic_client_db::ClientDbSourceIndexPath::new("src/materialized.rs"),
        relation:
            agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelation {
                from: agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelationEndpoint {
                    kind: agent_semantic_content_identity::provider_projection_relation::PROVIDER_RELATION_ITEM_ENDPOINT_KIND.to_owned(),
                    id: "rust://src/materialized.rs#item/function/materialized".into(),
                },
                kind: kind.into(),
                to: agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelationEndpoint {
                    kind: agent_semantic_content_identity::provider_projection_relation::PROVIDER_RELATION_ITEM_ENDPOINT_KIND.to_owned(),
                    id: "rust://src/materialized.rs#item/function/materialized".into(),
                },
            },
    }
    };

    let calls = relation("calls");
    request.import.generation_id = "relation-generation-base".to_owned().into();
    request.import.relations = vec![calls.clone()];
    fixture
        .commit_source_index_generation(request.clone(), &source_blobs)
        .expect("commit base relation generation");
    let base = fixture
        .load_active_workspace_generation_materialization(&project_root)
        .expect("load base relation generation")
        .expect("base relation generation must be visible");
    assert_eq!(base.relations, vec![calls.clone()]);

    request.import.generation_id = "relation-generation-unchanged".to_owned().into();
    fixture
        .commit_source_index_generation(request.clone(), &source_blobs)
        .expect("commit unchanged relation generation");
    let unchanged = fixture
        .load_active_workspace_generation_materialization(&project_root)
        .expect("load unchanged relation generation")
        .expect("unchanged relation generation must be visible");
    assert_eq!(unchanged.relations, vec![calls]);

    let uses = relation("uses");
    request.import.generation_id = "relation-generation-replaced".to_owned().into();
    request.import.relations = vec![uses.clone()];
    fixture
        .commit_source_index_generation(request.clone(), &source_blobs)
        .expect("commit replaced relation generation");
    let replaced = fixture
        .load_active_workspace_generation_materialization(&project_root)
        .expect("load replaced relation generation")
        .expect("replaced relation generation must be visible");
    assert_eq!(replaced.relations, vec![uses]);

    request.import.generation_id = "relation-generation-deleted".to_owned().into();
    request.import.relations.clear();
    fixture
        .commit_source_index_generation(request, &source_blobs)
        .expect("commit relation deletion generation");
    let deleted = fixture
        .load_active_workspace_generation_materialization(&project_root)
        .expect("load relation deletion generation")
        .expect("relation deletion generation must be visible");
    assert!(deleted.relations.is_empty());
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
        let workspace_snapshot =
            agent_semantic_content_identity::WorkspaceSnapshot::from_file_bytes(
                source_blobs.iter(),
            );
        request.source_snapshot = source_snapshot.clone();
        let materialization =
            agent_semantic_client_db::runtime_server_workspace::WorkspaceCanonicalMaterialization::from_source_index(
                workspace_identity,
                &workspace_snapshot,
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
