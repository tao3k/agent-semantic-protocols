
#[test]
fn db_engine_active_backend_contract_tracks_turso_default() {
    let project_root = temp_root("db-engine-active-project");
    let state_home = temp_root("db-engine-active-state-home");
    let state = ResolvedState::resolve_with_state_home(&project_root, &state_home)
        .expect("resolve state with explicit state home");
    let engine = ClientDbEngine::from_resolved_state(&state);
    let report = engine.inspect();

    assert_eq!(report.layout_version, STATE_LAYOUT_VERSION);
    assert_eq!(report.client_dir, state.paths.client_dir);
    assert_eq!(report.manifest_path, state.paths.client_manifest_json);
    assert_eq!(report.artifact_path, state.paths.artifacts_dir);
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
    assert_eq!(manifest["repoId"], state.repo.repo_id.as_str());
    assert_eq!(
        manifest["workspaceId"],
        state.workspace.workspace_id.as_str()
    );
    assert_eq!(manifest["scopeId"], state.scope_id.to_string());
    assert_eq!(manifest["dbFileName"], report.db_file_name);
    assert_eq!(manifest["schemaVersion"], report.schema_version);
    assert_eq!(manifest["durability"], report.durability);
    assert_eq!(manifest["features"]["asyncIo"], true);
    assert_eq!(manifest["features"]["concurrentWrites"], false);
    assert_eq!(manifest["features"]["fts"], true);
    assert_eq!(manifest["features"]["ftsIndexMethod"], true);
    assert_eq!(manifest["features"]["multiProcessWal"], true);
    assert_eq!(manifest["features"]["serializedWriterSlot"], true);
    assert_eq!(manifest["features"]["busyTimeoutMs"], 5000);
    assert_eq!(manifest["features"]["openLockRetryAttempts"], 80);
    assert_eq!(manifest["features"]["openLockRetryBaseMs"], 5);
    assert_eq!(manifest["features"]["openLockRetryMaxMs"], 200);
    assert_eq!(manifest["features"]["statementLockRetryAttempts"], 80);
    assert_eq!(manifest["features"]["operationLock"], true);
    assert_eq!(manifest["features"]["operationLockRetryAttempts"], 1000);
    assert_eq!(manifest["features"]["operationLockRetryMs"], 5);
    assert_eq!(manifest["features"]["mvcc"], false);
    assert_eq!(manifest["features"]["beginConcurrent"], false);
    assert_eq!(manifest["features"]["sync"], false);
    assert_eq!(manifest["dbPath"], report.db_path.to_str().unwrap());
    assert_eq!(
        manifest["artifactPath"],
        state.paths.artifacts_dir.to_str().unwrap()
    );
    assert_eq!(
        manifest["generationManifestPath"],
        state.paths.client_cache_manifest_path.to_str().unwrap()
    );
    assert!(manifest.get("sqliteControlDbPath").is_none());
    assert!(manifest.get("sqliteReport").is_none());
    assert!(
        !project_root
            .join(".cache")
            .join("agent-semantic-protocol")
            .exists()
    );

    assert_eq!(engine.backend(), ClientDbBackend::Turso);
    assert_eq!(report.backend, TURSO_BACKEND);
    assert_eq!(engine.backend().as_str(), TURSO_BACKEND);
    assert_eq!(report.db_file_name, "client.turso");
    assert_eq!(
        engine.db_path(),
        state.paths.client_dir.join("client.turso")
    );
    assert_eq!(report.db_path, state.paths.client_dir.join("client.turso"));
    assert!(
        !report.db_path.exists(),
        "DB Engine inspect must not create the active DB file"
    );
    assert_eq!(
        engine.inspect_backend().status,
        agent_semantic_client_db::ClientDbStatus::Missing
    );
    let expected_snapshot_root = "a".repeat(64);
    let expected_index_artifact_digest = "b".repeat(64);
    let source_index_lookup = ClientDbEngine::lookup_source_index_from_client_dir(
        agent_semantic_client_db::ClientDbSourceIndexClientDirLookupRequest {
            client_dir: &state.paths.client_dir,
            indexed_project_root: &project_root,
            language_id: None,
            query_keys: Vec::new(),
            limit: 8,
            expected_snapshot_root: &expected_snapshot_root,
            expected_index_artifact_digest: &expected_index_artifact_digest,
        },
    )
    .expect("lookup missing source-index control DB");
    assert_eq!(
        source_index_lookup.db_path,
        ClientDbEngine::turso_path_for_client_dir(&state.paths.client_dir)
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
                expected_snapshot_root: &expected_snapshot_root,
                expected_index_artifact_digest: &expected_index_artifact_digest,
            },
        )
        .expect("lookup missing source-index control DB from project root");
        assert_eq!(project_source_index_lookup, source_index_lookup);
    }
    let report_json = serde_json::to_value(&report).expect("serialize db engine report");
    assert_eq!(report_json["layoutVersion"], STATE_LAYOUT_VERSION);
    assert_eq!(report_json["repoId"], state.repo.repo_id.as_str());
    assert_eq!(
        report_json["workspaceId"],
        state.workspace.workspace_id.as_str()
    );
    assert_eq!(report_json["scopeId"], state.scope_id.to_string());
    assert_eq!(report_json["features"]["concurrentWrites"], false);
    assert_eq!(report_json["features"]["ftsIndexMethod"], true);
    assert_eq!(report_json["features"]["multiProcessWal"], true);
    assert_eq!(report_json["features"]["serializedWriterSlot"], true);
    assert_eq!(report_json["features"]["busyTimeoutMs"], 5000);
    assert_eq!(report_json["features"]["operationLock"], true);
    assert_eq!(report_json["features"]["mvcc"], false);
    assert_eq!(report_json["features"]["beginConcurrent"], false);
    assert!(report_json.get("controlReport").is_none());
    assert!(report_json.get("sqliteReport").is_none());
    let schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../../schemas/semantic-db-engine-report.v1.schema.json"
    ))
    .expect("parse db engine report schema");
    assert_eq!(
        schema["$id"],
        "https://agent-semantic-protocols.local/schemas/semantic-db-engine-report.v1.schema.json"
    );
    assert_eq!(schema["properties"]["backend"]["const"], TURSO_BACKEND);
    assert_eq!(schema["properties"]["dbFileName"]["const"], "client.turso");
    assert_eq!(
        schema["properties"]["durability"]["const"],
        "turso-local-file"
    );
    assert_eq!(
        schema["properties"]["features"]["$ref"],
        "#/$defs/tursoFeatures"
    );
    assert_eq!(
        schema["$defs"]["tursoFeatures"]["properties"]["concurrentWrites"]["const"],
        false
    );
    assert_eq!(
        schema["$defs"]["tursoFeatures"]["properties"]["multiProcessWal"]["const"],
        true
    );
    assert_eq!(
        schema["$defs"]["tursoFeatures"]["properties"]["busyTimeoutMs"]["const"],
        5000
    );
    assert_eq!(
        schema["$defs"]["tursoFeatures"]["properties"]["operationLock"]["const"],
        true
    );
    assert_eq!(
        schema["$defs"]["tursoFeatures"]["properties"]["mvcc"]["const"],
        false
    );
    assert_eq!(
        schema["$defs"]["tursoFeatures"]["properties"]["beginConcurrent"]["const"],
        false
    );
    assert!(schema["properties"].get("sqliteReport").is_none());
    assert!(schema["$defs"].get("activeEngineFeatures").is_none());
    assert!(schema["$defs"].get("engineFeatures").is_none());
    assert!(schema["$defs"].get("sqliteReport").is_none());
    assert!(schema["$defs"].get("sqliteRuntimePragmas").is_none());
    let manifest_schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../../schemas/semantic-db-engine-manifest.v1.schema.json"
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
    ] {
        assert!(
            manifest_required
                .iter()
                .any(|field| field.as_str() == Some(required_field)),
            "manifest schema must require {required_field}"
        );
    }
    assert_eq!(
        manifest_schema["properties"]["backend"]["const"],
        TURSO_BACKEND
    );
    assert_eq!(
        manifest_schema["properties"]["dbFileName"]["const"],
        "client.turso"
    );
    assert_eq!(
        manifest_schema["properties"]["durability"]["const"],
        "turso-local-file"
    );
    assert_eq!(
        manifest_schema["properties"]["features"]["$ref"],
        "#/$defs/tursoFeatures"
    );
    assert_eq!(
        manifest_schema["$defs"]["tursoFeatures"]["properties"]["concurrentWrites"]["const"],
        false
    );
    assert_eq!(
        manifest_schema["$defs"]["tursoFeatures"]["properties"]["multiProcessWal"]["const"],
        true
    );
    assert_eq!(
        manifest_schema["$defs"]["tursoFeatures"]["properties"]["busyTimeoutMs"]["const"],
        5000
    );
    assert_eq!(
        manifest_schema["$defs"]["tursoFeatures"]["properties"]["operationLock"]["const"],
        true
    );
    assert_eq!(
        manifest_schema["$defs"]["tursoFeatures"]["properties"]["mvcc"]["const"],
        false
    );
    assert_eq!(
        manifest_schema["$defs"]["tursoFeatures"]["properties"]["beginConcurrent"]["const"],
        false
    );
    for retired_field in ["sqliteControlDbPath", "sqliteReport"] {
        assert!(
            !manifest_required
                .iter()
                .any(|field| field.as_str() == Some(retired_field)),
            "manifest schema must not require retired {retired_field}"
        );
        assert!(
            manifest_schema["properties"].get(retired_field).is_none(),
            "manifest schema must not expose retired {retired_field}"
        );
    }
    assert!(
        manifest_schema["$defs"]
            .get("activeEngineFeatures")
            .is_none()
    );
    assert!(manifest_schema["$defs"].get("engineFeatures").is_none());
    assert!(manifest_schema["$defs"].get("sqliteReport").is_none());
    assert!(
        manifest_schema["$defs"]
            .get("sqliteRuntimePragmas")
            .is_none()
    );
    let route_receipt_schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../../schemas/semantic-db-route-receipt.v1.schema.json"
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
