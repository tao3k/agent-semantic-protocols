use std::{
    ffi::{OsStr, OsString},
    fs,
    path::PathBuf,
};

use agent_semantic_client_core::state_core::{
    ASP_STATE_HOME_ENV, ResolvedState, SQLITE_V1_BACKEND, STATE_LAYOUT_VERSION, TURSO_BACKEND,
};
use agent_semantic_client_core::{CacheExportMethod, ClientCacheManifest, LanguageId, ProviderId};
#[cfg(feature = "turso-backend")]
use agent_semantic_client_core::{
    CacheGenerationId, ClientCacheFileHash, SemanticSchemaId, SemanticSchemaVersion,
};
#[cfg(feature = "turso-backend")]
use agent_semantic_client_db::{
    CLIENT_DB_SOURCE_INDEX_PROVIDER_ID, CLIENT_DB_SOURCE_INDEX_SCHEMA_ID,
    CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION, ClientDbSourceIndexImportFile,
    ClientDbSourceIndexImportRequest, ClientDbSourceIndexLookupState, ClientDbSourceIndexSource,
    ClientDbStructuralDependencyUsage, ClientDbStructuralIndexImport, ClientDbStructuralKind,
    ClientDbStructuralLocator, ClientDbStructuralName, ClientDbStructuralOwner,
    ClientDbStructuralPath, ClientDbStructuralQueryKey, ClientDbStructuralSource,
    ClientDbStructuralSymbol, build_source_index_import,
};
use agent_semantic_client_db::{ClientDbBackend, ClientDbEngine};
use serde_json::json;

#[test]
fn db_engine_active_backend_contract_tracks_default_turso_cutover() {
    let project_root = temp_root("db-engine-active-project");
    let state_home = temp_root("db-engine-active-state-home");
    let state = ResolvedState::resolve_with_state_home(&project_root, &state_home)
        .expect("resolve state with explicit state home");
    let engine = ClientDbEngine::from_resolved_state(&state);
    let report = engine.inspect();

    assert_eq!(report.future_backend, TURSO_BACKEND);
    assert_eq!(report.layout_version, STATE_LAYOUT_VERSION);
    assert_eq!(report.client_dir, state.paths.client_dir);
    assert_eq!(report.manifest_path, state.paths.client_manifest_json);
    assert_eq!(report.artifact_path, state.paths.artifacts_dir);
    assert_eq!(engine.future_backend(), TURSO_BACKEND);
    assert_eq!(engine.layout_version(), STATE_LAYOUT_VERSION);
    assert_eq!(engine.client_dir(), state.paths.client_dir.as_path());
    assert_eq!(
        engine.manifest_path(),
        state.paths.client_manifest_json.as_path()
    );
    assert_eq!(engine.artifact_path(), state.paths.artifacts_dir.as_path());
    assert_eq!(engine.repo_id(), state.repo.repo_id.as_str());
    assert_eq!(engine.workspace_id(), state.workspace.workspace_id.as_str());
    assert_eq!(engine.scope_id(), state.scope_id.to_string());
    engine
        .write_manifest()
        .expect("write active DB Engine manifest");
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(engine.manifest_path()).expect("read DB manifest"))
            .expect("parse DB manifest");
    assert_eq!(manifest["layoutVersion"], STATE_LAYOUT_VERSION);
    assert_eq!(manifest["backend"], report.backend);
    assert_eq!(manifest["futureBackend"], TURSO_BACKEND);
    assert_eq!(manifest["repoId"], state.repo.repo_id.as_str());
    assert_eq!(
        manifest["workspaceId"],
        state.workspace.workspace_id.as_str()
    );
    assert_eq!(manifest["scopeId"], state.scope_id.to_string());
    assert_eq!(manifest["dbFileName"], report.db_file_name);
    assert_eq!(manifest["schemaVersion"], report.schema_version);
    assert_eq!(manifest["durability"], report.durability);
    assert_eq!(manifest["dbPath"], report.db_path.to_str().unwrap());
    assert_eq!(
        manifest["artifactPath"],
        state.paths.artifacts_dir.to_str().unwrap()
    );
    assert_eq!(
        manifest["generationManifestPath"],
        state.paths.client_cache_manifest_path.to_str().unwrap()
    );
    assert_eq!(
        manifest["sqliteControlDbPath"],
        state.paths.client_db_path.to_str().unwrap()
    );
    assert!(
        !project_root
            .join(".cache")
            .join("agent-semantic-protocol")
            .exists()
    );

    if cfg!(feature = "turso-backend") {
        assert_eq!(engine.backend(), ClientDbBackend::Turso);
        assert_eq!(report.backend, TURSO_BACKEND);
        assert_eq!(engine.backend().as_str(), TURSO_BACKEND);
        assert_eq!(report.db_file_name, "client.turso");
        assert_eq!(
            engine.db_path(),
            state.paths.client_dir.join("client.turso")
        );
        assert_eq!(report.db_path, state.paths.client_dir.join("client.turso"));
        assert_eq!(report.future_backend_report.backend, TURSO_BACKEND);
        assert_eq!(report.future_backend_report.status, "missing");
        assert_eq!(report.future_backend_report.reason, None);
        assert_eq!(report.sqlite_report.db_path, state.paths.client_db_path);
        assert!(
            !state.paths.client_db_path.exists(),
            "Turso active descriptor must not create SQLite control fixture"
        );
    } else {
        assert_eq!(engine.backend(), ClientDbBackend::SqliteV1);
        assert_eq!(report.backend, SQLITE_V1_BACKEND);
        assert_eq!(engine.backend().as_str(), SQLITE_V1_BACKEND);
        assert_eq!(report.db_file_name, "client.sqlite3");
        assert_eq!(engine.db_path(), state.paths.client_db_path);
        assert_eq!(report.db_path, state.paths.client_db_path);
        assert_eq!(report.future_backend_report.backend, TURSO_BACKEND);
        assert_eq!(report.future_backend_report.status, "feature-disabled");
        assert_eq!(
            ClientDbEngine::turso_path_for_client_dir(&state.paths.client_dir),
            state.paths.client_dir.join("client.turso")
        );
    }
    assert!(
        !report.db_path.exists(),
        "DB Engine inspect must not create the active DB file"
    );
    assert_eq!(
        engine.inspect_backend().status,
        agent_semantic_client_db::ClientDbStatus::Missing
    );
    let source_index_lookup = ClientDbEngine::lookup_source_index_from_client_dir(
        agent_semantic_client_db::ClientDbSourceIndexClientDirLookupRequest {
            client_dir: &state.paths.client_dir,
            indexed_project_root: &project_root,
            language_id: None,
            query_keys: Vec::new(),
            limit: 8,
        },
    )
    .expect("lookup missing source-index control DB");
    assert_eq!(
        source_index_lookup.db_path,
        ClientDbEngine::sqlite_path_for_client_dir(&state.paths.client_dir)
    );
    assert_eq!(
        source_index_lookup.state,
        agent_semantic_client_db::ClientDbSourceIndexLookupState::MissingDb
    );
    {
        let _state_home_env = EnvVarGuard::set(ASP_STATE_HOME_ENV, &state_home);
        let project_source_index_lookup = ClientDbEngine::lookup_source_index_from_project(
            agent_semantic_client_db::ClientDbSourceIndexProjectLookupRequest {
                cache_project_root: &project_root,
                indexed_project_root: &project_root,
                language_id: None,
                query_keys: Vec::new(),
                limit: 8,
            },
        )
        .expect("lookup missing source-index control DB from project root");
        assert_eq!(project_source_index_lookup, source_index_lookup);
    }
    let report_json = serde_json::to_value(&report).expect("serialize db engine report");
    assert_eq!(report_json["futureBackend"], TURSO_BACKEND);
    assert_eq!(report_json["layoutVersion"], STATE_LAYOUT_VERSION);
    assert_eq!(report_json["repoId"], state.repo.repo_id.as_str());
    assert_eq!(
        report_json["workspaceId"],
        state.workspace.workspace_id.as_str()
    );
    assert_eq!(report_json["scopeId"], state.scope_id.to_string());
    assert_eq!(
        report_json["sqliteReport"]["status"],
        agent_semantic_client_db::ClientDbStatus::Missing.as_str()
    );
    let schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../schemas/semantic-db-engine-report.v1.schema.json"
    ))
    .expect("parse db engine report schema");
    assert_eq!(
        schema["$id"],
        "https://agent-semantic-protocols.local/schemas/semantic-db-engine-report.v1.schema.json"
    );
    assert!(
        schema["properties"]["backend"]["enum"]
            .as_array()
            .expect("backend enum")
            .iter()
            .any(|value| value == TURSO_BACKEND)
    );
    assert_eq!(
        schema["properties"]["futureBackend"]["const"],
        TURSO_BACKEND
    );
    let manifest_schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../schemas/semantic-db-engine-manifest.v1.schema.json"
    ))
    .expect("parse db engine manifest schema");
    assert_eq!(
        manifest_schema["$id"],
        "https://agent-semantic-protocols.local/schemas/semantic-db-engine-manifest.v1.schema.json"
    );
    let manifest_required = manifest_schema["required"]
        .as_array()
        .expect("manifest required fields");
    for required_field in [
        "backend",
        "dbPath",
        "generationManifestPath",
        "sqliteControlDbPath",
        "futureBackendReport",
        "sqliteReport",
    ] {
        assert!(
            manifest_required
                .iter()
                .any(|field| field.as_str() == Some(required_field)),
            "manifest schema must require {required_field}"
        );
    }
    assert!(
        manifest_schema["properties"]["backend"]["enum"]
            .as_array()
            .expect("manifest backend enum")
            .iter()
            .any(|value| value == TURSO_BACKEND)
    );
    assert_eq!(
        manifest_schema["properties"]["futureBackend"]["const"],
        TURSO_BACKEND
    );
    let route_receipt_schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../schemas/semantic-db-route-receipt.v1.schema.json"
    ))
    .expect("parse db route receipt schema");
    assert_eq!(
        route_receipt_schema["$id"],
        "https://agent-semantic-protocols.local/schemas/semantic-db-route-receipt.v1.schema.json"
    );
    assert!(
        route_receipt_schema["properties"]["routeSource"]["enum"]
            .as_array()
            .expect("route source enum")
            .iter()
            .any(|value| value == "overlay-fts")
    );

    let _ = std::fs::remove_dir_all(project_root);
    let _ = std::fs::remove_dir_all(state_home);
}

#[test]
fn db_engine_write_session_imports_manifest_without_exposing_sqlite_handle() {
    let project_root = temp_root("db-engine-write-session-project");
    let state_home = temp_root("db-engine-write-session-state-home");
    let state = ResolvedState::resolve_with_state_home(&project_root, &state_home)
        .expect("resolve state with explicit state home");
    fs::create_dir_all(project_root.join("src")).expect("create src dir");
    fs::write(
        project_root.join("src/lib.rs"),
        "pub fn cached_fixture() {}\n",
    )
    .expect("write source fixture");
    let manifest: ClientCacheManifest = serde_json::from_value(json!({
        "schemaId": "agent.semantic-protocols.client-cache-manifest",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.client",
        "protocolVersion": "1",
        "cacheRoot": state.paths.client_dir.display().to_string(),
        "generations": [
            {
                "generationId": "rust-main-1",
                "languageId": "rust",
                "providerId": "rs-harness",
                "providerVersion": "0.1.0",
                "exportMethod": "search/prime",
                "projectRoot": project_root.display().to_string(),
                "packageRoot": ".",
                "schemaIds": ["agent.semantic-protocols.semantic-search-packet"],
                "cacheStatus": "hit",
                "rawSourceStored": false,
                "requestFingerprint": "fnv64:write-session",
                "fileHashes": [{
                    "path": "src/lib.rs",
                    "sha256": "0000000000000000000000000000000000000000000000000000000000000000",
                    "byteLen": 1,
                    "mtimeMs": 1
                }],
                "artifactIds": ["search/rust-main-1.json"]
            }
        ]
    }))
    .expect("manifest fixture");

    let mut write_session = ClientDbEngine::open_write_session_client_dir(&state.paths.client_dir)
        .expect("open DB Engine write session");
    write_session
        .import_manifest(&manifest)
        .expect("import manifest through DB Engine write session");
    let write_report = write_session.inspect().expect("inspect write session");
    assert_eq!(
        write_report.status,
        agent_semantic_client_db::ClientDbStatus::Present
    );
    assert_eq!(write_report.generation_count, 1);
    assert!(state.paths.client_db_path.exists());

    let read_session = ClientDbEngine::open_read_session_client_dir(&state.paths.client_dir)
        .expect("open DB Engine read session")
        .expect("read session exists");
    let hit = read_session
        .lookup_generation_request(
            &LanguageId::from("rust"),
            &ProviderId::from("rs-harness"),
            &project_root,
            &CacheExportMethod::from("search/prime"),
            Some("fnv64:write-session".to_string()),
        )
        .expect("lookup generation through DB Engine read session")
        .expect("generation hit");
    assert_eq!(hit.artifact_ids.len(), 1);
    assert_eq!(hit.artifact_ids[0].as_str(), "search/rust-main-1.json");

    let engine = ClientDbEngine::from_resolved_state(&state);
    if engine.backend() == ClientDbBackend::Turso {
        assert!(
            !engine.db_path().exists(),
            "write-session control import must not create active Turso DB path"
        );
    }
}

#[cfg(feature = "turso-backend")]
#[tokio::test(flavor = "current_thread")]
async fn turso_backend_bootstrap_smoke_creates_local_file_without_sqlite_control_fixture() {
    let project_root = temp_root("turso-bootstrap-project");
    let state_home = temp_root("turso-bootstrap-state-home");
    let state = ResolvedState::resolve_with_state_home(&project_root, &state_home)
        .expect("resolve state with explicit state home");
    let engine = ClientDbEngine::from_resolved_state(&state);
    let sqlite_path = ClientDbEngine::sqlite_path_for_client_dir(&state.paths.client_dir);
    let turso_path = engine.db_path().to_path_buf();

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
    assert!(
        !sqlite_path.exists(),
        "Turso smoke must not create SQLite file"
    );
    agent_semantic_client_db::upsert_turso_graph_entity(
        &turso_path,
        &agent_semantic_client_db::TursoClientDbGraphEntity {
            id: "selector:rust://src/lib.rs#item/struct/TursoFixture".to_string(),
            kind: "selector".to_string(),
            label: "TursoFixture".to_string(),
            selector: Some("rust://src/lib.rs#item/struct/TursoFixture".to_string()),
            path: Some("src/lib.rs".to_string()),
            language_id: Some("rust".to_string()),
            provider_id: Some("rs-harness".to_string()),
            query_keys: vec!["TursoFixture".to_string(), "db-engine".to_string()],
        },
    )
    .await
    .expect("upsert Turso graph entity row");
    let graph_entities =
        agent_semantic_client_db::list_turso_graph_entities(&turso_path, Some("selector"), 8)
            .await
            .expect("list Turso graph entities");
    assert_eq!(graph_entities.len(), 1);
    assert_eq!(graph_entities[0].kind, "selector");
    assert_eq!(graph_entities[0].label, "TursoFixture");
    assert_eq!(graph_entities[0].language_id.as_deref(), Some("rust"));
    assert_eq!(graph_entities[0].provider_id.as_deref(), Some("rs-harness"));
    assert_eq!(
        graph_entities[0].query_keys,
        vec!["TursoFixture".to_string(), "db-engine".to_string()]
    );
    let graph = agent_semantic_client_db::ClientDbEvidenceGraph {
        schema_id: agent_semantic_client_db::CLIENT_DB_EVIDENCE_GRAPH_SCHEMA_ID,
        schema_version: agent_semantic_client_db::CLIENT_DB_EVIDENCE_GRAPH_SCHEMA_VERSION,
        generation_id: "generation-fixture".to_string(),
        project_root: project_root.clone(),
        nodes: vec![
            agent_semantic_client_db::ClientDbEvidenceGraphNode {
                id: "source-owner:generation-fixture:src/lib.rs".to_string(),
                kind: "source-owner",
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
    let graph_report = agent_semantic_client_db::persist_turso_evidence_graph(&turso_path, &graph)
        .await
        .expect("persist EvidenceGraph projection into Turso");
    assert_eq!(graph_report.entity_count, 2);
    assert_eq!(graph_report.edge_count, 1);
    let graph_edges =
        agent_semantic_client_db::list_turso_graph_edges(&turso_path, Some("contains-selector"), 8)
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
        }],
    })
    .expect("build source-index Turso read-model import");
    let source_index_report = engine
        .persist_source_index_read_model(&source_index_import)
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
        source_index_hits.iter().any(|hit| {
            hit.source == "stable"
                && hit.document_id.contains("source-index-turso-fixture")
                && hit.selector.as_deref() == Some("rust://src/source_index_fixture.rs#file")
        }),
        "source_index_hits={source_index_hits:?}"
    );
    let rust_language_id = LanguageId::from("rust");
    let python_language_id = LanguageId::from("python");
    let source_index_lookup = engine
        .lookup_source_index_read_model("source_index_turso_fixture", Some(&rust_language_id), 8)
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
        .lookup_source_index_read_model("source_index_turso_fixture", Some(&python_language_id), 8)
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
        .persist_structural_index_read_model(&structural_index_import)
        .await
        .expect("persist structural-index read-model through DB Engine facade");
    assert_eq!(structural_report.graph_entity_count, 3);
    assert_eq!(structural_report.graph_edge_count, 2);
    assert_eq!(structural_report.search_document_count, 2);
    let structural_entities =
        agent_semantic_client_db::list_turso_graph_entities(&turso_path, None, 32)
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
    let structural_edges = agent_semantic_client_db::list_turso_graph_edges(&turso_path, None, 32)
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
    assert!(
        !sqlite_path.exists(),
        "Turso graph/search/receipt writes must stay on the active Turso path"
    );
    let _ = std::fs::remove_dir_all(project_root);
    let _ = std::fs::remove_dir_all(state_home);
}

fn temp_root(label: &str) -> PathBuf {
    let mut root = std::env::temp_dir();
    let unique = format!(
        "asp-client-db-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos()
    );
    root.push(unique);
    std::fs::create_dir_all(&root).expect("create temp root");
    root
}

struct EnvVarGuard {
    key: &'static str,
    previous: Option<OsString>,
}

impl EnvVarGuard {
    fn set(key: &'static str, value: impl AsRef<OsStr>) -> Self {
        let previous = std::env::var_os(key);
        unsafe {
            std::env::set_var(key, value);
        }
        Self { key, previous }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(value) => unsafe {
                std::env::set_var(self.key, value);
            },
            None => unsafe {
                std::env::remove_var(self.key);
            },
        }
    }
}
