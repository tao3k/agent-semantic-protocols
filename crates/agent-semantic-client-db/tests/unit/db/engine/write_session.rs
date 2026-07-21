#[test]
fn db_engine_write_session_imports_manifest_without_exposing_retired_db_handle() {
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
    assert!(ClientDbEngine::turso_path_for_client_dir(&state.paths.client_dir).exists());

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
    assert!(
        engine.db_path().exists(),
        "write-session import must materialize active Turso cache-generation read model"
    );
    let mut write_session = ClientDbEngine::open_write_session_client_dir(&state.paths.client_dir)
        .expect("open DB Engine write session for Turso invalidate");
    let invalidated = write_session
        .invalidate_generations_for_project(&project_root)
        .expect("invalidate DB Engine cache-generation rows");
    assert_eq!(invalidated, 1);
    let read_session = ClientDbEngine::open_read_session_client_dir(&state.paths.client_dir)
        .expect("open DB Engine read session after Turso invalidation")
        .expect("read session exists after Turso invalidation");
    let miss = read_session
        .lookup_generation_request(
            &LanguageId::from("rust"),
            &ProviderId::from("rs-harness"),
            &project_root,
            &CacheExportMethod::from("search/prime"),
            Some("fnv64:write-session".to_string()),
        )
        .expect("lookup generation after DB Engine invalidation");
    assert_eq!(miss, None);
}

#[test]
fn db_engine_cache_status_survives_concurrent_read_write_smoke() {
    let project_root = temp_root("db-engine-cache-status-concurrent-project");
    let state_home = temp_root("db-engine-cache-status-concurrent-state-home");
    let state = ResolvedState::resolve_with_state_home(&project_root, &state_home)
        .expect("resolve state with explicit state home");
    fs::create_dir_all(project_root.join("src")).expect("create src dir");
    fs::write(
        project_root.join("src/lib.rs"),
        "pub fn concurrent_cache_status_fixture() {}\n",
    )
    .expect("write source fixture");

    let client_dir = Arc::new(state.paths.client_dir.clone());
    let project_root = Arc::new(project_root);
    let reader_count = 6usize;
    let barrier = Arc::new(Barrier::new(reader_count + 1));
    let mut readers = Vec::new();

    for reader_id in 0..reader_count {
        let barrier = Arc::clone(&barrier);
        let client_dir = Arc::clone(&client_dir);
        let project_root = Arc::clone(&project_root);
        readers.push(thread::spawn(move || {
            barrier.wait();
            for _ in 0..12 {
                let report = ClientDbEngine::inspect_client_dir(client_dir.as_path());
                assert_eq!(
                    report.db_path.file_name().and_then(|name| name.to_str()),
                    Some("facts.turso")
                );
                let read_session =
                    ClientDbEngine::open_read_session_client_dir(client_dir.as_path())
                        .expect("open read session during concurrent cache status smoke");
                if let Some(read_session) = read_session {
                    let _ = read_session
                        .lookup_generation_request(
                            &LanguageId::from("rust"),
                            &ProviderId::from("rs-harness"),
                            project_root.as_path(),
                            &CacheExportMethod::from("search/prime"),
                            None,
                        )
                        .expect("lookup generation during concurrent cache status smoke");
                }
            }
            reader_id
        }));
    }

    barrier.wait();
    for generation_index in 0..8 {
        let manifest = concurrent_cache_status_manifest(
            client_dir.as_ref(),
            project_root.as_ref(),
            generation_index,
        );
        let mut write_session = ClientDbEngine::open_write_session_client_dir(client_dir.as_path())
            .expect("open write session during concurrent cache status smoke");
        write_session
            .import_manifest(&manifest)
            .expect("import manifest during concurrent cache status smoke");
        let write_report = write_session
            .inspect()
            .expect("inspect write session during concurrent cache status smoke");
        assert_eq!(
            write_report.status,
            agent_semantic_client_db::ClientDbStatus::Present
        );
    }

    for reader in readers {
        reader.join().expect("join cache status reader");
    }

    let read_session = ClientDbEngine::open_read_session_client_dir(client_dir.as_path())
        .expect("open final read session")
        .expect("final read session exists");
    let hit = read_session
        .lookup_generation_request(
            &LanguageId::from("rust"),
            &ProviderId::from("rs-harness"),
            project_root.as_path(),
            &CacheExportMethod::from("search/prime"),
            Some("fnv64:cache-status-7".to_string()),
        )
        .expect("lookup final concurrent cache status generation")
        .expect("final concurrent cache status generation exists");
    assert_eq!(hit.artifact_ids.len(), 1);
    assert_eq!(hit.artifact_ids[0].as_str(), "search/cache-status-7.json");
}

fn concurrent_cache_status_manifest(
    client_dir: &Path,
    project_root: &Path,
    generation_index: usize,
) -> ClientCacheManifest {
    serde_json::from_value(json!({
        "schemaId": "agent.semantic-protocols.client-cache-manifest",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.client",
        "protocolVersion": "1",
        "cacheRoot": client_dir.display().to_string(),
        "generations": [
            {
                "generationId": format!("rust-cache-status-{generation_index}"),
                "languageId": "rust",
                "providerId": "rs-harness",
                "providerVersion": "0.1.0",
                "exportMethod": "search/prime",
                "projectRoot": project_root.display().to_string(),
                "packageRoot": ".",
                "schemaIds": ["agent.semantic-protocols.semantic-search-packet"],
                "cacheStatus": "hit",
                "rawSourceStored": false,
                "requestFingerprint": format!("fnv64:cache-status-{generation_index}"),
                "fileHashes": [{
                    "path": "src/lib.rs",
                    "sha256": "1111111111111111111111111111111111111111111111111111111111111111",
                    "byteLen": 1,
                    "mtimeMs": generation_index
                }],
                "artifactIds": [format!("search/cache-status-{generation_index}.json")]
            }
        ]
    }))
    .expect("concurrent cache status manifest fixture")
}

#[test]
fn agent_session_claim_keeps_first_resident_child_for_root_and_name() {
    let state = std::env::temp_dir().join(format!(
        "asp-agent-session-claim-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos(),
    ));
    std::fs::create_dir_all(&state).expect("temporary state root");
    let registry =
        agent_semantic_client_db::AgentSessionRegistry::open_or_create_state_root(&state)
            .expect("open registry");
    let project_id = "claim-project";
    let root_session_id = "claim-root";
    let first_child_id = "claim-child-first";

    let first = registry
        .claim_resident_session(
            agent_semantic_client_db::agent_session_registry::AgentSessionRegisterRequest {
                project_id,
                root_session_id,
                session_id: first_child_id,
                message_target_id: None,
                parent_session_id: Some(root_session_id),
                name: "asp-explore",
                role: "asp_explorer",
                model_observation: Some(
                    agent_semantic_client_db::AgentSessionModelObservationRef {
                        model: "gpt-5.4-mini",
                    source: agent_semantic_client_db::AgentSessionModelObservationSource::CodexSubagentStart,
                        observed_at: 10,
                        evidence_ref: Some("turn:test"),
                    },
                ),
                status: "pending-target",
                expires_at: None,
                metadata_json: "{}",
                now: 1,
            },
        )
        .expect("claim first resident child");
    let duplicate = registry
        .claim_resident_session(
            agent_semantic_client_db::agent_session_registry::AgentSessionRegisterRequest {
                project_id,
                root_session_id,
                session_id: "claim-child-duplicate",
                message_target_id: None,
                parent_session_id: Some(root_session_id),
                name: "asp-explore",
                role: "asp_explorer",
                model_observation: Some(
                    agent_semantic_client_db::AgentSessionModelObservationRef {
                        model: "gpt-5.4-mini",
                    source: agent_semantic_client_db::AgentSessionModelObservationSource::CodexSubagentStart,
                        observed_at: 20,
                        evidence_ref: Some("turn:test-2"),
                    },
                ),
                status: "pending-target",
                expires_at: None,
                metadata_json: "{}",
                now: 2,
            },
        )
        .expect("read existing resident child");

    assert_eq!(first.session_id, first_child_id);
    assert_eq!(duplicate.session_id, first_child_id);
    assert_eq!(
        registry
            .session_by_name(project_id, root_session_id, "asp-explore")
            .expect("lookup resident child")
            .expect("resident child row")
            .session_id,
        first_child_id
    );
    drop(registry);
    std::fs::remove_dir_all(state).expect("remove temporary state root");
}

#[test]
fn agent_session_claim_replaces_archived_resident_child_for_root_and_name() {
    let state = std::env::temp_dir().join(format!(
        "asp-agent-session-archived-claim-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos(),
    ));
    std::fs::create_dir_all(&state).expect("temporary state root");
    let registry =
        agent_semantic_client_db::AgentSessionRegistry::open_or_create_state_root(&state)
            .expect("open registry");
    let project_id = "archived-claim-project";
    let root_session_id = "archived-claim-root";
    let first_child_id = "archived-claim-first";

    registry
        .claim_resident_session(
            agent_semantic_client_db::agent_session_registry::AgentSessionRegisterRequest {
                project_id,
                root_session_id,
                session_id: first_child_id,
                message_target_id: None,
                parent_session_id: Some(root_session_id),
                name: "asp-explore",
                role: "asp_explorer",
                model_observation: Some(
                    agent_semantic_client_db::AgentSessionModelObservationRef {
                        model: "gpt-5.4-mini",
                    source: agent_semantic_client_db::AgentSessionModelObservationSource::CodexSubagentStart,
                        observed_at: 10,
                        evidence_ref: Some("turn:test"),
                    },
                ),
                status: "pending-target",
                expires_at: None,
                metadata_json: "{}",
                now: 1,
            },
        )
        .expect("claim first resident child");
    assert!(
        registry
            .archive_session(project_id, first_child_id, 2)
            .expect("archive first resident child")
    );

    let replacement = registry
        .claim_resident_session(
            agent_semantic_client_db::agent_session_registry::AgentSessionRegisterRequest {
                project_id,
                root_session_id,
                session_id: "archived-claim-replacement",
                message_target_id: None,
                parent_session_id: Some(root_session_id),
                name: "asp-explore",
                role: "asp_explorer",
                model_observation: Some(
                    agent_semantic_client_db::AgentSessionModelObservationRef {
                        model: "gpt-5.4-mini",
                    source: agent_semantic_client_db::AgentSessionModelObservationSource::CodexSubagentStart,
                        observed_at: 20,
                        evidence_ref: Some("turn:test-2"),
                    },
                ),
                status: "pending-target",
                expires_at: None,
                metadata_json: "{}",
                now: 3,
            },
        )
        .expect("claim replacement resident child");

    assert_eq!(replacement.session_id, "archived-claim-replacement");
    assert_eq!(
        registry
            .session_by_name(project_id, root_session_id, "asp-explore")
            .expect("lookup replacement resident child")
            .expect("replacement resident child row")
            .session_id,
        "archived-claim-replacement"
    );
    drop(registry);
    std::fs::remove_dir_all(state).expect("remove temporary state root");
}
