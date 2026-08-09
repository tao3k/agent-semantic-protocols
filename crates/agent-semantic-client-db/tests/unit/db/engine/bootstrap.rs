#[tokio::test(flavor = "current_thread")]
async fn turso_0_7_bootstrap_rejects_existing_db_without_format_receipt() {
    let project_root = temp_root("turso-legacy-sqlite-project");
    let state_home = temp_root("turso-legacy-sqlite-state-home");
    init_git_repository(&project_root);
    let state = ResolvedState::resolve_with_state_home(&project_root, &state_home)
        .expect("resolve state with explicit state home");
    state
        .ensure_minimal_layout()
        .expect("commit State Core identity before DB bootstrap");
    let engine = ClientDbEngine::from_resolved_state(&state);
    let turso_path = engine.db_path().to_path_buf();
    std::fs::create_dir_all(turso_path.parent().expect("Turso parent"))
        .expect("create Turso parent");
    std::fs::write(&turso_path, b"SQLite format 3\0legacy-client-db")
        .expect("write legacy SQLite header");

    let error = engine
        .bootstrap_active_turso()
        .await
        .expect_err("unreceipted DB must require full Turso 0.7 staging migration");
    assert!(error.contains("has no Turso 0.7 format receipt"));
    assert!(error.contains("full staging migration is required"));
    assert!(error.contains("in-place compatibility bootstrap is forbidden"));
    assert_eq!(
        std::fs::read(&turso_path).expect("legacy file remains rollback input"),
        b"SQLite format 3\0legacy-client-db"
    );

    let _ = std::fs::remove_dir_all(project_root);
    let _ = std::fs::remove_dir_all(state_home);
}

#[tokio::test(flavor = "current_thread")]
async fn turso_backend_bootstrap_smoke_creates_local_file() {
    let project_root = temp_root("turso-bootstrap-project");
    let state_home = temp_root("turso-bootstrap-state-home");
    init_git_repository(&project_root);
    let state = ResolvedState::resolve_with_state_home(&project_root, &state_home)
        .expect("resolve state with explicit state home");
    state
        .ensure_minimal_layout()
        .expect("commit State Core identity before DB bootstrap");
    let engine = ClientDbEngine::from_resolved_state(&state);
    let turso_path = engine.db_path().to_path_buf();
    let source_snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes([(
        "src/source_index_fixture.rs",
        "abcdef0123456789".repeat(4),
    )])
    .evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        "rs-harness",
    );

    let report = engine
        .bootstrap_active_turso()
        .await
        .expect("bootstrap Turso client DB");

    assert_eq!(engine.backend(), ClientDbBackend::Turso);
    assert_eq!(report.backend, TURSO_BACKEND);
    assert_eq!(report.status, "bootstrap-smoke");
    assert_eq!(report.db_file_name, "facts.turso");
    assert_eq!(report.schema_version, 1);
    assert_eq!(report.schema_bootstrap, "ready");
    assert_eq!(report.reason, None);
    assert_eq!(report.db_path, state.paths.client_dir.join("facts.turso"));
    assert_eq!(report.db_path, turso_path);
    assert!(report.db_path.exists());
    let structural_index_import = ClientDbStructuralIndexImport {
        generation_id: CacheGenerationId::from("structural-index-turso-fixture"),
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("rs-harness"),
        provider_version: None,
        export_method: None,
        project_root: project_root.clone(),
        package_root: None,
        schema_id: SemanticSchemaId::from("agent.semantic-protocols.semantic-structural-index"),
        schema_version: SemanticSchemaVersion::from("1"),
        source_artifact_id: None,
        file_hashes: Vec::new(),
        owners: vec![ClientDbStructuralOwner {
            owner_path: ClientDbStructuralPath::from("src/structural_fixture.rs"),
            owner_kind: ClientDbStructuralKind::from("source"),
            source_authority: ClientDbStructuralSource::from("native-parser"),
            start_line: None,
            end_line: None,
            query_keys: vec![ClientDbStructuralQueryKey::from("parse_config")],
        }],
        symbols: vec![ClientDbStructuralSymbol {
            owner_path: ClientDbStructuralPath::from("src/structural_fixture.rs"),
            name: ClientDbStructuralName::from("parse_config"),
            kind: ClientDbStructuralKind::from("function"),
            visibility: Some(ClientDbStructuralKind::from("public")),
            source_locator: Some(ClientDbStructuralLocator::from(
                "rust://src/structural_fixture.rs#item/fn/parse_config",
            )),
            query_keys: vec![ClientDbStructuralQueryKey::from("parse_config")],
        }],
        dependency_usages: vec![ClientDbStructuralDependencyUsage {
            owner_path: ClientDbStructuralPath::from("src/structural_fixture.rs"),
            package_name: ClientDbStructuralName::from("serde_json"),
            package_version: None,
            api_name: Some(ClientDbStructuralName::from("from_str")),
            import_path: Some(ClientDbStructuralPath::from("serde_json::from_str")),
            manifest_path: None,
            lockfile_hash: None,
            source: ClientDbStructuralSource::from("native-parser"),
            source_locator: Some(ClientDbStructuralLocator::from(
                "rust://src/structural_fixture.rs#dep/serde_json/from_str",
            )),
            query_keys: vec![ClientDbStructuralQueryKey::from("serde_json::from_str")],
        }],
    };
    let structural_report = engine
        .persist_structural_index_read_model(&structural_index_import, &source_snapshot)
        .await
        .expect("persist structural-index read-model through DB Engine facade");
    assert_eq!(structural_report.search_document_count, 2);
    let structural_symbol_hits = engine
        .search_structural_index_documents(&source_snapshot, "parse_config", 8)
        .await
        .expect("search Turso structural-index symbol documents through DB Engine facade");
    assert_eq!(
        structural_symbol_hits.state,
        agent_semantic_client_db::TursoClientDbSearchState::Hit
    );
    assert!(
        structural_symbol_hits.hits.iter().any(|hit| {
            hit.source == "projection"
                && hit.document_id.starts_with("structural-index:symbol:")
                && hit
                    .document_id
                    .contains(source_snapshot.root_digest.as_str())
                && hit.selector.as_deref()
                    == Some("rust://src/structural_fixture.rs#item/fn/parse_config")
        }),
        "structural_symbol_hits={structural_symbol_hits:?}"
    );
    let structural_dependency_hits = engine
        .search_structural_index_documents(&source_snapshot, "serde_json from_str", 8)
        .await
        .expect("search Turso structural-index dependency documents through DB Engine facade");
    assert_eq!(
        structural_dependency_hits.state,
        agent_semantic_client_db::TursoClientDbSearchState::Hit
    );
    assert!(
        structural_dependency_hits.hits.iter().any(|hit| {
            hit.source == "projection"
                && hit.document_id.starts_with("structural-index:dependency:")
                && hit
                    .document_id
                    .contains(source_snapshot.root_digest.as_str())
                && hit.selector.as_deref()
                    == Some("rust://src/structural_fixture.rs#dep/serde_json/from_str")
        }),
        "structural_dependency_hits={structural_dependency_hits:?}"
    );

    let mut mismatched_snapshot = source_snapshot.clone();
    mismatched_snapshot.root_digest = "0".repeat(64);
    let mismatched_search = engine
        .search_structural_index_documents(&mismatched_snapshot, "parse_config", 8)
        .await
        .expect("reject a structural projection from another Merkle root");
    assert_eq!(
        mismatched_search.state,
        agent_semantic_client_db::TursoClientDbSearchState::ColdRequired
    );

    let replacement_documents = [
        agent_semantic_client_db::TursoClientDbSearchDocument {
            document_id: "doc:first".to_string(),
            entity_id: "selector:first".to_string(),
            selector: Some("rust://src/lib.rs#item/struct/FirstFixture".to_string()),
            document: "alphazulu9831 replacement fixture".to_string(),
        },
        agent_semantic_client_db::TursoClientDbSearchDocument {
            document_id: "doc:second".to_string(),
            entity_id: "selector:second".to_string(),
            selector: Some("rust://src/lib.rs#item/struct/SecondFixture".to_string()),
            document: "betayankee7042 replacement fixture".to_string(),
        },
    ];
    engine
        .replace_search_document_generation(
            "replacement-fixture",
            &source_snapshot,
            &replacement_documents,
        )
        .await
        .expect("publish root-bound replacement generation");
    engine
        .replace_search_document_generation(
            "replacement-fixture",
            &source_snapshot,
            &replacement_documents[1..],
        )
        .await
        .expect("replace the same root with a smaller complete generation");
    let removed_document = engine
        .search_documents("replacement-fixture", &source_snapshot, "alphazulu9831", 8)
        .await
        .expect("search the replaced Turso projection generation");
    assert_eq!(
        removed_document.state,
        agent_semantic_client_db::TursoClientDbSearchState::Miss
    );
    assert!(removed_document.hits.is_empty());
    let retained_document = engine
        .search_documents("replacement-fixture", &source_snapshot, "betayankee7042", 8)
        .await
        .expect("search the retained replacement document");
    assert_eq!(
        retained_document.state,
        agent_semantic_client_db::TursoClientDbSearchState::Hit
    );
    assert_eq!(retained_document.hits.len(), 1);
    assert_eq!(retained_document.hits[0].document_id, "doc:second");
    let _ = std::fs::remove_dir_all(project_root);
    let _ = std::fs::remove_dir_all(state_home);
}

#[tokio::test(flavor = "current_thread")]
async fn turso_backend_bootstrap_accepts_preexisting_empty_operation_lock_file() {
    let project_root = temp_root("turso-bootstrap-preexisting-lock-project");
    let state_home = temp_root("turso-bootstrap-preexisting-lock-state-home");
    init_git_repository(&project_root);
    let state = ResolvedState::resolve_with_state_home(&project_root, &state_home)
        .expect("resolve state with explicit state home");
    state
        .ensure_minimal_layout()
        .expect("commit State Core identity before DB bootstrap");
    let engine = ClientDbEngine::from_resolved_state(&state);
    let turso_path = engine.db_path().to_path_buf();
    std::fs::create_dir_all(
        turso_path
            .parent()
            .expect("active Turso path has parent directory"),
    )
    .expect("create Turso client dir");
    std::fs::write(turso_path.with_file_name("client.turso.operation.lock"), "")
        .expect("write preexisting empty operation lock");

    let report = engine
        .bootstrap_active_turso()
        .await
        .expect("bootstrap Turso client DB with preexisting lock file");

    assert_eq!(report.backend, TURSO_BACKEND);
    assert_eq!(report.schema_bootstrap, "ready");
    assert_eq!(report.db_path, turso_path);
    assert!(report.db_path.exists());
    let _ = std::fs::remove_dir_all(project_root);
    let _ = std::fs::remove_dir_all(state_home);
}

#[tokio::test(flavor = "current_thread")]
async fn turso_backend_bootstrap_is_idempotent_across_two_sequential_calls() {
    let project_root = temp_root("turso-bootstrap-idempotent-project");
    let state_home = temp_root("turso-bootstrap-idempotent-state-home");
    init_git_repository(&project_root);
    let state = ResolvedState::resolve_with_state_home(&project_root, &state_home)
        .expect("resolve state with explicit state home");
    state
        .ensure_minimal_layout()
        .expect("commit State Core identity before DB bootstrap");
    let engine = ClientDbEngine::from_resolved_state(&state);
    let turso_path = engine.db_path().to_path_buf();

    let first = engine
        .bootstrap_active_turso()
        .await
        .expect("first bootstrap Turso client DB");
    let second = engine
        .bootstrap_active_turso()
        .await
        .expect("second bootstrap Turso client DB");

    assert_eq!(first.backend, TURSO_BACKEND);
    assert_eq!(second.backend, TURSO_BACKEND);
    assert_eq!(first.schema_bootstrap, "ready");
    assert_eq!(second.schema_bootstrap, "ready");
    assert_eq!(first.db_path, turso_path);
    assert_eq!(second.db_path, turso_path);
    let _ = std::fs::remove_dir_all(project_root);
    let _ = std::fs::remove_dir_all(state_home);
}
