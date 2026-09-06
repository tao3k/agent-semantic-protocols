use super::{
    AGENT_SESSION_REGISTRY_DB_NAME, AgentSessionRegisterRequest, AgentSessionRegistry, Arc,
    CLIENT_DB_SOURCE_INDEX_PROVIDER_ID, CLIENT_DB_SOURCE_INDEX_SCHEMA_ID,
    CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION, CacheGenerationId, ClientCacheFileHash,
    ClientDbSourceIndexImportAssemblyRequest, ClientDbSourceIndexImportFile,
    ClientDbSourceIndexImportRequest, ClientDbSourceIndexRefreshRequest,
    ClientDbSourceIndexScopeFile, ClientDbSourceIndexSource, Command, LanguageId, PathBuf,
    ProviderId, ResolvedState, SemanticSchemaId, SemanticSchemaVersion,
    build_fixture_source_index_import, client_db_source_index_file_count, env,
    source_index_relative_path, source_index_scope_dirs, temp_root,
};

#[tokio::test]
async fn agent_session_registry_project_open_requires_runtime_owner() {
    let root = temp_root("agent-session-registry-state-home");
    let state_home = root.join("state");
    let project_root = root.join("project");
    std::fs::create_dir_all(&project_root).expect("create project root");
    gix::discover(&project_root).expect("resolve the owner-backed project repository with Gix");

    let status = Command::new(env::current_exe().expect("locate current test binary"))
        .arg("--exact")
        .arg("db::session_and_source_index::agent_session_registry_project_open_without_runtime_owner_helper")
        .arg("--nocapture")
        .env("ASP_SESSION_STATE_HOME_CHILD", "1")
        .env("ASP_SESSION_STATE_HOME", &state_home)
        .env("ASP_SESSION_PROJECT_ROOT", &project_root)
        .env("ASP_STATE_HOME", &state_home)
        .status()
        .expect("run isolated ASP_STATE_HOME contract");
    assert!(status.success(), "isolated ASP_STATE_HOME contract failed");
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn agent_session_registry_project_open_without_runtime_owner_helper() {
    if env::var("ASP_SESSION_STATE_HOME_CHILD").ok().as_deref() != Some("1") {
        return;
    }
    let state_home =
        PathBuf::from(env::var_os("ASP_SESSION_STATE_HOME").expect("ASP_SESSION_STATE_HOME"));
    let project_root =
        PathBuf::from(env::var_os("ASP_SESSION_PROJECT_ROOT").expect("ASP_SESSION_PROJECT_ROOT"));
    let state =
        ResolvedState::resolve_with_state_home(&project_root, &state_home).expect("resolve state");
    state.ensure_minimal_layout().expect("ensure state layout");
    let state_root =
        AgentSessionRegistry::state_root_for_project(&project_root).expect("resolve project root");
    assert_eq!(state_root, state.state_home);
    let error = match AgentSessionRegistry::open_or_create_project(&project_root).await {
        Ok(_) => panic!("client project open without Runtime Server must fail closed"),
        Err(error) => error,
    };
    assert!(
        error.contains("requires Runtime Server typed IPC; direct-open is forbidden"),
        "unexpected project-open error: {error}"
    );
    assert!(
        !state_root.join(AGENT_SESSION_REGISTRY_DB_NAME).exists(),
        "client must not create the Runtime Server-owned registry DB"
    );
    assert!(
        !state
            .paths
            .project_dir
            .join(AGENT_SESSION_REGISTRY_DB_NAME)
            .exists(),
        "agent session registry must not create a project-id DB"
    );
    assert!(
        !state
            .paths
            .client_dir
            .join("agent")
            .join(AGENT_SESSION_REGISTRY_DB_NAME)
            .exists(),
        "agent session registry must not create a project-local DB"
    );
}

#[tokio::test]
async fn agent_session_register_rejects_same_child_rebind_from_stale_root_mapping() {
    let root = temp_root("agent-session-registry-move-child");
    let state_root = root.join("agent");
    let registry =
        AgentSessionRegistry::open_or_create_state_root(&state_root).expect("create registry");
    registry
        .register_session(AgentSessionRegisterRequest {
            project_id: "project-1".into(),
            root_session_id: "old-root".into(),
            session_id: "child-session".into(),
            parent_session_id: Some("old-root".into()),
            name: "asp-explore".into(),
            role: "asp-explore".into(),
            model_observation: None,
            message_target_id: None,
            status: "closed".into(),
            expires_at: None,
            metadata_json: "{}".into(),
            now: 1_800_000_000,
        })
        .await
        .expect("register stale mapping");

    let error = registry
        .register_session(AgentSessionRegisterRequest {
            project_id: "project-1".into(),
            root_session_id: "new-root".into(),
            session_id: "child-session".into(),
            parent_session_id: Some("new-root".into()),
            name: "asp-explore".into(),
            role: "asp-explore".into(),
            model_observation: None,
            message_target_id: None,
            status: "active".into(),
            expires_at: None,
            metadata_json: "{}".into(),
            now: 1_800_000_010,
        })
        .await
        .expect_err("one child ThreadId must not be rebound to a different root");

    assert!(
        error.contains("agent-session-child-identity-rebind-denied"),
        "unexpected child rebind terminal: {error}"
    );
    assert_eq!(
        registry
            .query_sessions(
                "project-1",
                Some(agent_semantic_client_db::AgentSessionRootSessionId::from(
                    "old-root",
                )),
                Some(agent_semantic_client_db::AgentSessionResidentName::from(
                    "asp-explore",
                )),
            )
            .await
            .expect("query old root")
            .len(),
        1
    );
    assert_eq!(
        registry
            .session_by_id("project-1", "child-session")
            .await
            .expect("lookup retained child")
            .expect("child exists")
            .root_session_id(),
        "old-root"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn agent_session_register_is_idempotent_and_cross_project_rebind_is_typed() {
    let root = temp_root("agent-session-registry-global-child-identity");
    let registry =
        AgentSessionRegistry::open_or_create_state_root(root.join("agent")).expect("registry");
    let request = |project_id: &str, now| AgentSessionRegisterRequest {
        project_id: project_id.into(),
        root_session_id: "root-session".into(),
        session_id: "child-session".into(),
        parent_session_id: Some("parent-thread".into()),
        name: "asp-explorer".into(),
        role: "asp_explorer".into(),
        model_observation: None,
        message_target_id: None,
        status: "active".into(),
        expires_at: None,
        metadata_json: "{}".into(),
        now,
    };

    let first = registry
        .register_session(request("project-a", 1_800_000_000))
        .await
        .expect("first registration");
    let replay = registry
        .register_session(request("project-a", 1_800_000_001))
        .await
        .expect("same child binding is idempotent");
    assert_eq!(first.session_id(), replay.session_id());
    assert_eq!(replay.physical_generation, 1);

    let error = registry
        .register_session(request("project-b", 1_800_000_002))
        .await
        .expect_err("one Codex ThreadId cannot bind to another project");
    assert!(
        error.contains("agent-session-child-identity-rebind-denied")
            && error.contains("storedProject=project-a")
            && error.contains("requestedProject=project-b"),
        "unexpected cross-project rebind terminal: {error}"
    );
    let _ = std::fs::remove_dir_all(root);
}

fn concurrent_session_request(
    writer_id: usize,
    root_session_id: String,
) -> AgentSessionRegisterRequest<'static> {
    AgentSessionRegisterRequest {
        project_id: "project-session-stress".into(),
        root_session_id: root_session_id.into(),
        session_id: format!("child-session-{writer_id}").into(),
        parent_session_id: Some("main-session".into()),
        name: "asp-explore".into(),
        role: "asp-explore".into(),
        model_observation: Some(agent_semantic_client_db::AgentSessionModelObservationRef {
            model: "gpt-test",
            source:
                agent_semantic_client_db::AgentSessionModelObservationSource::CodexSubagentStart,
            observed_at: 10,
            evidence_ref: Some("turn:test"),
        }),
        message_target_id: None,
        status: "active".into(),
        expires_at: None,
        metadata_json: "{\"route\":\"resident-owner-stress\"}".into(),
        now: 1_800_001_000 + writer_id as i64,
    }
}

#[tokio::test]
async fn agent_session_registry_one_owner_accepts_concurrent_session_routes() {
    let root = temp_root("agent-session-registry-process-stress");
    let state_root = root.join("agent");
    let writer_count = 6usize;
    let registry = Arc::new(
        AgentSessionRegistry::open_or_create_state_root(&state_root).expect("open registry"),
    );
    let mut tasks = Vec::new();
    for writer_id in 0..writer_count {
        let registry = Arc::clone(&registry);
        tasks.push(tokio::spawn(async move {
            registry
                .register_session(concurrent_session_request(
                    writer_id,
                    format!("root-session-{writer_id}"),
                ))
                .await
        }));
    }
    for task in tasks {
        task.await
            .expect("join resident owner session")
            .expect("register resident owner session");
    }
    let sessions = registry
        .query_sessions(
            "project-session-stress",
            None,
            Some(agent_semantic_client_db::AgentSessionResidentName::from(
                "asp-explore",
            )),
        )
        .await
        .expect("query process stress sessions");
    assert_eq!(sessions.len(), writer_count);
    for writer_id in 0..writer_count {
        assert!(
            sessions.iter().any(|session| session.session_id()
                == format!("child-session-{writer_id}")
                && session.root_session_id() == format!("root-session-{writer_id}")),
            "missing process writer {writer_id} session in {sessions:?}"
        );
    }

    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn agent_session_registry_retains_concurrent_children_on_one_shared_route() {
    let root = temp_root("agent-session-registry-process-shared-route-stress");
    let state_root = root.join("agent");
    let writer_count = 6usize;
    let registry = Arc::new(
        AgentSessionRegistry::open_or_create_state_root(&state_root).expect("open registry"),
    );
    let successful_writers = {
        let mut tasks = Vec::new();
        for writer_id in 0..writer_count {
            let registry = Arc::clone(&registry);
            tasks.push(tokio::spawn(async move {
                registry
                    .register_session(concurrent_session_request(
                        writer_id,
                        "root-session".to_owned(),
                    ))
                    .await
            }));
        }
        let mut successful_writers = 0usize;
        for task in tasks {
            successful_writers +=
                usize::from(task.await.expect("join shared-route session").is_ok());
        }
        successful_writers
    };
    assert_eq!(successful_writers, writer_count);

    let sessions = registry
        .query_sessions(
            "project-session-stress",
            Some(agent_semantic_client_db::AgentSessionRootSessionId::from(
                "root-session",
            )),
            Some(agent_semantic_client_db::AgentSessionResidentName::from(
                "asp-explore",
            )),
        )
        .await
        .expect("query shared route process stress session");
    assert_eq!(
        sessions.len(),
        writer_count,
        "shared route identity must not overwrite concrete child executions"
    );
    for writer_id in 0..writer_count {
        assert!(
            sessions
                .iter()
                .any(|session| session.session_id() == format!("child-session-{writer_id}")),
            "missing shared-route child {writer_id}: {sessions:?}"
        );
    }

    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn source_index_import_assembly_uses_turso_ready_contract_rows() {
    let root = temp_root("source-index-import");
    let src = root.join("src");
    std::fs::create_dir_all(&src).expect("create src dir");
    let lib = src.join("lib.rs");
    let source = b"pub fn turso_source_index_fixture() {}\n";
    std::fs::write(&lib, source).expect("write source");
    let selector = "rust://src/lib.rs#item/function/turso_source_index_fixture";
    let scope_file = ClientDbSourceIndexScopeFile {
        relations: Vec::new(),
        path: lib.clone(),
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("asp-rust"),
        projection_coverage:
            agent_semantic_client_db::ClientDbSourceIndexProjectionCoverage::Complete,
        projection_diagnostic: None,
        selector_receipts: vec![agent_semantic_client_db::ClientDbSourceIndexSelector {
            owner_path: "src/lib.rs".into(),
            provider_id: ProviderId::from("asp-rust"),
            selector_id: selector.into(),
            symbol: Some("turso_source_index_fixture".into()),
            kind: Some("function".into()),
            source: ClientDbSourceIndexSource::from(CLIENT_DB_SOURCE_INDEX_PROVIDER_ID),
            query_keys: vec!["turso_source_index_fixture".into()],
            derived_projections: Vec::new(),
            projection_record: crate::projection_fixture::projection_record(
                crate::projection_fixture::ProjectionFixtureInput {
                    language_id: "rust",
                    provider_id: "asp-rust",
                    owner_path: "src/lib.rs",
                    structural_selector: selector,
                    item_kind: "function",
                    item_name: "turso_source_index_fixture",
                    source,
                    source_byte_start: 0,
                    source_byte_end: source.len() as u64,
                },
            ),
        }],
    };

    let import = agent_semantic_client_db::assemble_source_index_import(
        ClientDbSourceIndexImportAssemblyRequest {
            generation_id: CacheGenerationId::from("source-index-generation"),
            project_root: root.clone(),
            schema_id: SemanticSchemaId::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_ID),
            schema_version: SemanticSchemaVersion::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION),
            selector_source: ClientDbSourceIndexSource::from(CLIENT_DB_SOURCE_INDEX_PROVIDER_ID),
            file_text_bytes_limit: 4096,
            registry_fingerprint: "registry:v1".to_string(),
            extra_scope_dirs: Vec::new(),
            files: vec![scope_file.clone()],
            source_blobs: agent_semantic_client_db::ClientDbSourceIndexSourceBlobs::from_normalized(
                [(
                    agent_semantic_client_db::ClientDbSourceIndexPath::new("src/lib.rs"),
                    b"pub fn turso_source_index_fixture() {}\n".to_vec(),
                )],
            ),
        },
    )
    .expect("assemble source-index import");

    assert_eq!(source_index_relative_path(&root, &lib), "src/lib.rs");
    assert_eq!(
        source_index_scope_dirs(&root, &[scope_file]),
        [".", "src"].into_iter().map(str::to_string).collect()
    );
    assert_eq!(client_db_source_index_file_count(usize::MAX), u32::MAX);
    assert_eq!(import.owners.len(), 1);
    assert_eq!(import.selectors.len(), 1);
    assert!(
        import
            .file_hashes
            .iter()
            .any(|hash| hash.path == "src/lib.rs")
    );
    assert!(
        import
            .owners
            .first()
            .expect("owner")
            .query_keys
            .iter()
            .any(|key| key.as_str() == "turso_source_index_fixture")
    );

    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn source_index_refresh_request_remains_db_engine_owned() {
    let root = temp_root("source-index-refresh");
    let import = build_fixture_source_index_import(ClientDbSourceIndexImportRequest {
        source_blobs: Default::default(),
        generation_id: CacheGenerationId::from("source-index-refresh-generation"),
        project_root: root.clone(),
        schema_id: SemanticSchemaId::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_ID),
        schema_version: SemanticSchemaVersion::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION),
        selector_source: ClientDbSourceIndexSource::from(CLIENT_DB_SOURCE_INDEX_PROVIDER_ID),
        file_hashes: vec![ClientCacheFileHash {
            path: "src/lib.rs".to_string(),
            sha256: "0123456789abcdef".repeat(4),
            byte_len: 37,
            mtime_ms: 42,
        }],
        files: vec![ClientDbSourceIndexImportFile {
            relations: Vec::new(),
            relative_path: "src/lib.rs".to_string(),
            language_id: LanguageId::from("rust"),
            provider_id: ProviderId::from("asp-rust"),
            text: "pub fn source_index_refresh_fixture() {}\n".to_string(),
            selectors: Vec::new(),
        }],
    })
    .expect("build source-index import");

    let request = ClientDbSourceIndexRefreshRequest {
        file_count: 1,
        import,
        source_snapshot: crate::snapshot_fixture::source_snapshot_evidence(),
    };
    assert_eq!(request.file_count, 1);
    assert_eq!(
        request.import.schema_id.as_str(),
        CLIENT_DB_SOURCE_INDEX_SCHEMA_ID
    );

    let _ = std::fs::remove_dir_all(root);
}
