
#[tokio::test(flavor = "current_thread")]
async fn turso_backend_bootstrap_smoke_creates_local_file() {
    let project_root = temp_root("turso-bootstrap-project");
    let state_home = temp_root("turso-bootstrap-state-home");
    let state = ResolvedState::resolve_with_state_home(&project_root, &state_home)
        .expect("resolve state with explicit state home");
    let engine = ClientDbEngine::from_resolved_state(&state);
    let turso_path = engine.db_path().to_path_buf();
    let source_snapshot = crate::snapshot_fixture::source_snapshot_evidence();

    let report = engine
        .bootstrap_active_turso()
        .await
        .expect("bootstrap Turso client DB");

    assert_eq!(engine.backend(), ClientDbBackend::Turso);
    assert_eq!(report.backend, TURSO_BACKEND);
    assert_eq!(report.status, "bootstrap-smoke");
    assert_eq!(report.db_file_name, "client.turso");
    assert_eq!(report.schema_version, 1);
    assert_eq!(report.schema_bootstrap, "ready");
    assert_eq!(report.reason, None);
    assert_eq!(report.db_path, state.paths.client_dir.join("client.turso"));
    assert_eq!(report.db_path, turso_path);
    assert!(report.db_path.exists());
    let graph = agent_semantic_client_db::ClientDbEvidenceGraph {
        schema_id: agent_semantic_client_db::CLIENT_DB_EVIDENCE_GRAPH_SCHEMA_ID,
        schema_version: agent_semantic_client_db::CLIENT_DB_EVIDENCE_GRAPH_SCHEMA_VERSION,
        generation_id: "generation-fixture".to_string(),
        project_root: project_root.clone(),
        nodes: vec![
            agent_semantic_client_db::ClientDbEvidenceGraphNode {
                id: "source-owner:generation-fixture:src/lib.rs".to_string(),
                kind: "source-owner",
                semantic_kind: None,
                label: "src/lib.rs".to_string(),
                path: Some("src/lib.rs".to_string()),
                selector: None,
                query_keys: vec!["src/lib.rs".to_string()],
                language_id: Some("rust".to_string()),
                provider_id: Some("rs-harness".to_string()),
            },
            agent_semantic_client_db::ClientDbEvidenceGraphNode {
                id: "selector:rust://src/lib.rs#item/struct/TursoFixture".to_string(),
                kind: "selector",
                semantic_kind: Some("struct".to_string()),
                label: "TursoFixture".to_string(),
                path: Some("src/lib.rs".to_string()),
                selector: Some("rust://src/lib.rs#item/struct/TursoFixture".to_string()),
                query_keys: vec!["TursoFixture".to_string()],
                language_id: Some("rust".to_string()),
                provider_id: Some("rs-harness".to_string()),
            },
        ],
        edges: vec![agent_semantic_client_db::ClientDbEvidenceGraphEdge {
            from: "source-owner:generation-fixture:src/lib.rs".to_string(),
            to: "selector:rust://src/lib.rs#item/struct/TursoFixture".to_string(),
            kind: "contains-selector",
        }],
    };
    let graph_report = agent_semantic_client_db::persist_turso_evidence_graph(
        &turso_path,
        &graph,
        &source_snapshot,
    )
    .await
    .expect("persist EvidenceGraph projection into Turso");
    assert_eq!(graph_report.entity_count, 2);
    assert_eq!(graph_report.edge_count, 1);
    let graph_edges =
        agent_semantic_client_db::list_turso_graph_edges(
            &turso_path,
            &source_snapshot,
            Some("contains-selector"),
            8,
        )
        .await
        .expect("list Turso graph edges");
    assert_eq!(graph_edges.len(), 1);
    assert_eq!(
        graph_edges[0].from,
        "source-owner:generation-fixture:src/lib.rs"
    );
    assert_eq!(
        graph_edges[0].to,
        "selector:rust://src/lib.rs#item/struct/TursoFixture"
    );
    assert_eq!(graph_edges[0].kind, "contains-selector");
    let source_index_import = build_source_index_import(ClientDbSourceIndexImportRequest {
        generation_id: CacheGenerationId::from("source-index-turso-fixture"),
        project_root: project_root.clone(),
        schema_id: SemanticSchemaId::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_ID),
        schema_version: SemanticSchemaVersion::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION),
        selector_source: ClientDbSourceIndexSource::from(CLIENT_DB_SOURCE_INDEX_PROVIDER_ID),
        file_hashes: vec![ClientCacheFileHash {
            path: "src/source_index_fixture.rs".to_string(),
            sha256: "abcdef0123456789".repeat(4),
            byte_len: 43,
            mtime_ms: 7,
        }],
        files: vec![ClientDbSourceIndexImportFile {
            relative_path: "src/source_index_fixture.rs".to_string(),
            language_id: LanguageId::from("rust"),
            provider_id: ProviderId::from("rs-harness"),
            text: "pub fn source_index_turso_fixture() {}\n".to_string(),
            selectors: Vec::new(),
        }],
    })
    .expect("build source-index Turso read-model import");
    let source_index_report = engine
        .persist_source_index_read_model(&source_index_import, &source_snapshot)
        .await
        .expect("persist source-index read-model through DB Engine facade");
    assert_eq!(source_index_report.graph_entity_count, 2);
    assert_eq!(source_index_report.graph_edge_count, 1);
    assert_eq!(source_index_report.search_document_count, 1);
    let source_index_hits = engine
        .search_source_index_documents("source_index_turso_fixture", 8)
        .await
        .expect("search Turso source-index documents through DB Engine facade");
    assert!(
        source_index_hits.is_empty(),
        "source-index imports use the canonical snapshot read model, not stable FTS documents: {source_index_hits:?}"
    );
    let rust_language_id = LanguageId::from("rust");
    let python_language_id = LanguageId::from("python");
    let source_index_lookup = engine
        .lookup_source_index_read_model(
            &source_snapshot,
            "source_index_turso_fixture",
            Some(&rust_language_id),
            8,
        )
        .await
        .expect("lookup Turso source-index read-model through DB Engine facade");
    assert_eq!(
        source_index_lookup.state,
        ClientDbSourceIndexLookupState::Hit
    );
    assert_eq!(source_index_lookup.db_path, turso_path);
    assert!(
        source_index_lookup
            .candidates
            .iter()
            .any(|candidate| candidate.path == "src/source_index_fixture.rs"
                && candidate.language_id.as_ref().map(|id| id.as_str()) == Some("rust")
                && candidate.provider_id.as_ref().map(|id| id.as_str()) == Some("rs-harness")
                && candidate.source_kind.as_str() == "turso-source-index"),
        "source_index_lookup={source_index_lookup:?}"
    );
    let source_index_client_dir_lookup =
        ClientDbEngine::lookup_source_index_read_model_from_client_dir(
            &state.paths.client_dir,
            &source_snapshot,
            "source_index_turso_fixture",
            Some(&rust_language_id),
            8,
        )
        .await
        .expect("lookup Turso source-index read-model from resolved client dir");
    assert_eq!(
        source_index_client_dir_lookup.state,
        ClientDbSourceIndexLookupState::Hit
    );
    assert_eq!(source_index_client_dir_lookup.db_path, turso_path);
    assert!(
        source_index_client_dir_lookup
            .candidates
            .iter()
            .any(|candidate| candidate.path == "src/source_index_fixture.rs"
                && candidate.language_id.as_ref().map(|id| id.as_str()) == Some("rust")
                && candidate.provider_id.as_ref().map(|id| id.as_str()) == Some("rs-harness")
                && candidate.source_kind.as_str() == "turso-source-index"),
        "source_index_client_dir_lookup={source_index_client_dir_lookup:?}"
    );
    let source_index_language_miss = engine
        .lookup_source_index_read_model(
            &source_snapshot,
            "source_index_turso_fixture",
            Some(&python_language_id),
            8,
        )
        .await
        .expect("lookup Turso source-index read-model with non-matching language");
    assert_eq!(
        source_index_language_miss.state,
        ClientDbSourceIndexLookupState::Miss
    );
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
    let structural_entities =
        agent_semantic_client_db::list_turso_graph_entities(
            &turso_path,
            &source_snapshot,
            None,
            32,
        )
            .await
            .expect("list Turso graph entities after structural projection");
    assert!(
        structural_entities.iter().any(|entity| {
            entity.kind == "symbol"
                && entity.label == "parse_config"
                && entity.path.as_deref() == Some("src/structural_fixture.rs")
                && entity.language_id.as_deref() == Some("rust")
                && entity.provider_id.as_deref() == Some("rs-harness")
                && entity
                    .query_keys
                    .iter()
                    .any(|query_key| query_key == "parse_config")
        }),
        "structural_entities={structural_entities:?}"
    );
    assert!(
        structural_entities.iter().any(|entity| {
            entity.kind == "dependency-usage"
                && entity.label == "serde_json::from_str"
                && entity.path.as_deref() == Some("src/structural_fixture.rs")
                && entity
                    .query_keys
                    .iter()
                    .any(|query_key| query_key == "serde_json::from_str")
        }),
        "structural_entities={structural_entities:?}"
    );
    let structural_edges = agent_semantic_client_db::list_turso_graph_edges(
        &turso_path,
        &source_snapshot,
        None,
        32,
    )
    .await
    .expect("list Turso graph edges after structural projection");
    assert!(
        structural_edges
            .iter()
            .any(|edge| edge.kind == "defines-symbol"),
        "structural_edges={structural_edges:?}"
    );
    assert!(
        structural_edges
            .iter()
            .any(|edge| edge.kind == "uses-dependency"),
        "structural_edges={structural_edges:?}"
    );
    let structural_symbol_hits = engine
        .search_structural_index_documents("parse_config", 8)
        .await
        .expect("search Turso structural-index symbol documents through DB Engine facade");
    assert!(
        structural_symbol_hits.iter().any(|hit| {
            hit.source == "stable"
                && hit.document_id.contains("structural-index-turso-fixture")
                && hit.selector.as_deref()
                    == Some("rust://src/structural_fixture.rs#item/fn/parse_config")
        }),
        "structural_symbol_hits={structural_symbol_hits:?}"
    );
    let structural_dependency_hits = engine
        .search_structural_index_documents("serde_json from_str", 8)
        .await
        .expect("search Turso structural-index dependency documents through DB Engine facade");
    assert!(
        structural_dependency_hits.iter().any(|hit| {
            hit.source == "stable"
                && hit.document_id.contains("structural-index-turso-fixture")
                && hit.selector.as_deref()
                    == Some("rust://src/structural_fixture.rs#dep/serde_json/from_str")
        }),
        "structural_dependency_hits={structural_dependency_hits:?}"
    );
    engine
        .upsert_search_document(&agent_semantic_client_db::TursoClientDbSearchDocument {
            namespace: "stable".to_string(),
            document_id: "doc:fixture".to_string(),
            entity_id: "selector:rust://src/lib.rs#item/struct/TursoFixture".to_string(),
            selector: Some("rust://src/lib.rs#item/struct/TursoFixture".to_string()),
            document: "TursoFixture implements a searchable DB engine smoke document".to_string(),
        })
        .await
        .expect("upsert Turso search document row through DB Engine facade");
    engine
        .upsert_overlay_document(&agent_semantic_client_db::TursoClientDbOverlayDocument {
            repo_id: "repo-fixture".to_string(),
            workspace_id: "workspace-fixture".to_string(),
            session_id: "session-fixture".to_string(),
            base_generation: "generation-fixture".to_string(),
            document_id: "overlay:fixture".to_string(),
            selector: Some("rust://src/lib.rs#item/struct/TursoFixture".to_string()),
            document: "dirty overlay document for dynamic search smoke".to_string(),
        })
        .await
        .expect("upsert Turso overlay document row through DB Engine facade");
    let hits = engine
        .search_documents("TursoFixture", 8)
        .await
        .expect("search Turso documents through DB Engine facade");
    assert!(
        hits.iter()
            .any(|hit| hit.source == "stable" && hit.document_id == "doc:fixture"),
        "hits={hits:?}"
    );
    assert!(
        hits.iter()
            .any(|hit| hit.source == "overlay" && hit.document_id == "overlay:fixture"),
        "hits={hits:?}"
    );
    engine
        .upsert_route_receipt(&agent_semantic_client_db::TursoClientDbRouteReceipt {
            receipt_id: "route-receipt:fixture".to_string(),
            repo_id: state.repo.repo_id.to_string(),
            workspace_id: state.workspace.workspace_id.to_string(),
            scope_id: state.scope_id.to_string(),
            session_id: Some("session-fixture".to_string()),
            query: "TursoFixture".to_string(),
            route_source: "overlay-fts".to_string(),
            selected_selector: Some("rust://src/lib.rs#item/struct/TursoFixture".to_string()),
            next_command: Some(
                "asp rust query --selector rust://src/lib.rs#item/struct/TursoFixture --workspace . --code"
                    .to_string(),
            ),
            hit_count: hits.len() as u32,
            evidence_ids: hits
                .iter()
                .map(|hit| format!("{}:{}", hit.source, hit.document_id))
                .collect(),
            created_at_ms: 1_782_856_000_000,
        })
        .await
        .expect("upsert route receipt through DB Engine facade");
    let route_receipts = engine
        .list_route_receipts(Some("session-fixture"), 8)
        .await
        .expect("list route receipts through DB Engine facade");
    assert_eq!(route_receipts.len(), 1);
    assert_eq!(route_receipts[0].receipt_id, "route-receipt:fixture");
    assert_eq!(route_receipts[0].query, "TursoFixture");
    assert_eq!(route_receipts[0].route_source, "overlay-fts");
    assert_eq!(
        route_receipts[0].selected_selector.as_deref(),
        Some("rust://src/lib.rs#item/struct/TursoFixture")
    );
    assert_eq!(
        route_receipts[0].next_command.as_deref(),
        Some(
            "asp rust query --selector rust://src/lib.rs#item/struct/TursoFixture --workspace . --code"
        )
    );
    assert_eq!(route_receipts[0].hit_count, hits.len() as u32);
    assert!(
        route_receipts[0]
            .evidence_ids
            .iter()
            .any(|evidence_id| evidence_id == "overlay:overlay:fixture"),
        "route_receipts={route_receipts:?}"
    );
    let route_receipt_json =
        serde_json::to_value(&route_receipts[0]).expect("serialize Turso route receipt");
    assert_eq!(route_receipt_json["receiptId"], "route-receipt:fixture");
    assert_eq!(route_receipt_json["routeSource"], "overlay-fts");
    assert_eq!(
        route_receipt_json["nextCommand"],
        "asp rust query --selector rust://src/lib.rs#item/struct/TursoFixture --workspace . --code"
    );
    let _ = std::fs::remove_dir_all(project_root);
    let _ = std::fs::remove_dir_all(state_home);
}

#[tokio::test(flavor = "current_thread")]
async fn turso_backend_bootstrap_accepts_preexisting_empty_operation_lock_file() {
    let project_root = temp_root("turso-bootstrap-preexisting-lock-project");
    let state_home = temp_root("turso-bootstrap-preexisting-lock-state-home");
    let state = ResolvedState::resolve_with_state_home(&project_root, &state_home)
        .expect("resolve state with explicit state home");
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
    let state = ResolvedState::resolve_with_state_home(&project_root, &state_home)
        .expect("resolve state with explicit state home");
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

#[test]
fn turso_backend_bootstrap_process_helper() {
    if env::var("ASP_TURSO_BOOTSTRAP_PROCESS_CHILD")
        .ok()
        .as_deref()
        != Some("1")
    {
        return;
    }
    let project_root = PathBuf::from(
        env::var("ASP_TURSO_BOOTSTRAP_PROJECT_ROOT")
            .expect("ASP_TURSO_BOOTSTRAP_PROJECT_ROOT"),
    );
    let state_home = PathBuf::from(
        env::var("ASP_TURSO_BOOTSTRAP_STATE_HOME").expect("ASP_TURSO_BOOTSTRAP_STATE_HOME"),
    );
    let writer_id: usize = env::var("ASP_TURSO_BOOTSTRAP_WRITER_ID")
        .expect("ASP_TURSO_BOOTSTRAP_WRITER_ID")
        .parse()
        .expect("parse ASP_TURSO_BOOTSTRAP_WRITER_ID");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build Turso bootstrap process runtime");
    runtime.block_on(async {
        let state = ResolvedState::resolve_with_state_home(&project_root, &state_home)
            .expect("resolve shared process-pressure state");
        let engine = ClientDbEngine::from_resolved_state(&state);
        let report = engine
            .bootstrap_active_turso()
            .await
            .expect("process bootstrap Turso client DB");
        assert_eq!(report.backend, TURSO_BACKEND);
        assert_eq!(report.schema_bootstrap, "ready");
        let event = ClientDbArtifactEvent {
            artifact_path: format!("process-bootstrap/agent-{writer_id}.command.json"),
            event_ordinal: 0,
            timestamp_ms: 30_000 + writer_id as i64,
            kind: "state/bootstrap".to_string(),
            language: "rust".to_string(),
            method: "bootstrap".to_string(),
            target: "client-db".to_string(),
            query: format!("bootstrap-process-{writer_id}"),
            project_root: project_root.display().to_string(),
            project_root_arg: ".".to_string(),
            bytes: 512 + writer_id as u64,
        };
        ClientDbEngine::upsert_artifact_events_from_client_dir(&state.paths.client_dir, &[event])
            .expect("process bootstrap writer should write artifact event");
    });
}

#[test]
fn turso_backend_bootstrap_survives_concurrent_process_open_pressure() {
    let project_root = temp_root("turso-bootstrap-process-project");
    let state_home = temp_root("turso-bootstrap-process-state-home");
    let state = ResolvedState::resolve_with_state_home(&project_root, &state_home)
        .expect("resolve shared process-pressure state");
    let process_count = 8usize;
    let current_exe = env::current_exe().expect("locate current test binary");
    let mut children = Vec::new();

    for writer_id in 0..process_count {
        children.push((
            writer_id,
            Command::new(&current_exe)
                .arg("--exact")
                .arg("db_engine::turso_backend_bootstrap_process_helper")
                .arg("--nocapture")
                .env("ASP_TURSO_BOOTSTRAP_PROCESS_CHILD", "1")
                .env("ASP_TURSO_BOOTSTRAP_PROJECT_ROOT", &project_root)
                .env("ASP_TURSO_BOOTSTRAP_STATE_HOME", &state_home)
                .env("ASP_TURSO_BOOTSTRAP_WRITER_ID", writer_id.to_string())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("spawn process bootstrap writer"),
        ));
    }

    for (writer_id, child) in children {
        let output = child
            .wait_with_output()
            .expect("wait for process bootstrap writer");
        assert!(
            output.status.success(),
            "process bootstrap writer {writer_id} failed: status={} stdout={} stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let all = ClientDbEngine::lookup_artifact_events_from_client_dir(
        &state.paths.client_dir,
        None,
        process_count as u32,
    )
    .expect("read process bootstrap Turso artifact events");
    assert_eq!(all.len(), process_count);
    for writer_id in 0..process_count {
        assert!(
            all.iter().any(|event| event.artifact_path
                == format!("process-bootstrap/agent-{writer_id}.command.json")),
            "missing bootstrap process writer {writer_id} event in {all:?}"
        );
    }
    assert!(state.paths.client_dir.join("client.turso").exists());
    let _ = std::fs::remove_dir_all(project_root);
    let _ = std::fs::remove_dir_all(state_home);
}
