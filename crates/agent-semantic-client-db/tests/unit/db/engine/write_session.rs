
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
                    Some("client.turso")
                );
                let read_session = ClientDbEngine::open_read_session_client_dir(client_dir.as_path())
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

#[test]
fn db_engine_cache_status_process_pressure_helper() {
    if env::var("ASP_TURSO_CACHE_PROCESS_PRESSURE_CHILD")
        .ok()
        .as_deref()
        != Some("1")
    {
        return;
    }
    let state_home = PathBuf::from(
        env::var("ASP_TURSO_CACHE_PROCESS_PRESSURE_STATE_HOME")
            .expect("ASP_TURSO_CACHE_PROCESS_PRESSURE_STATE_HOME"),
    );
    let project_root = PathBuf::from(
        env::var("ASP_TURSO_CACHE_PROCESS_PRESSURE_PROJECT_ROOT")
            .expect("ASP_TURSO_CACHE_PROCESS_PRESSURE_PROJECT_ROOT"),
    );
    let writer_id: usize = env::var("ASP_TURSO_CACHE_PROCESS_PRESSURE_WRITER_ID")
        .expect("ASP_TURSO_CACHE_PROCESS_PRESSURE_WRITER_ID")
        .parse()
        .expect("parse ASP_TURSO_CACHE_PROCESS_PRESSURE_WRITER_ID");
    fs::create_dir_all(project_root.join("src")).expect("create process pressure src dir");
    fs::write(
        project_root.join("src/lib.rs"),
        "pub fn concurrent_process_cache_status_fixture() {}\n",
    )
    .expect("write process pressure fixture");
    let state = ResolvedState::resolve_with_state_home(&project_root, &state_home)
        .expect("resolve process pressure state");

    for iteration in 0..4 {
        let manifest =
            process_cache_status_manifest(&state.paths.client_dir, &project_root, writer_id, iteration);
        let operation_started = std::time::Instant::now();
        let mut write_session = ClientDbEngine::open_write_session_client_dir(&state.paths.client_dir)
            .expect("open process pressure write session");
        write_session
            .import_manifest(&manifest)
            .expect("import process pressure manifest");
        let read_session = ClientDbEngine::open_read_session_client_dir(&state.paths.client_dir)
            .expect("open process pressure read session")
            .expect("process pressure read session exists");
        let fingerprint = format!("fnv64:process-cache-status-{writer_id}-{iteration}");
        let hit = read_session
            .lookup_generation_request(
                &LanguageId::from("rust"),
                &ProviderId::from("rs-harness"),
                &project_root,
                &CacheExportMethod::from("search/prime"),
                Some(fingerprint),
            )
            .expect("lookup process pressure generation")
            .expect("process pressure generation exists");
        assert_eq!(hit.artifact_ids.len(), 1);
        assert!(
            operation_started.elapsed() <= std::time::Duration::from_millis(750),
            "process pressure DB operation exceeded millisecond/subsecond target: writer={writer_id} iteration={iteration} elapsed={:?}",
            operation_started.elapsed()
        );
    }
}

#[test]
fn db_engine_cache_status_survives_concurrent_process_read_write_pressure() {
    let project_root = temp_root("db-engine-cache-process-pressure-project");
    let state_home = temp_root("db-engine-cache-process-pressure-state-home");
    fs::create_dir_all(project_root.join("src")).expect("create process pressure src dir");
    fs::write(
        project_root.join("src/lib.rs"),
        "pub fn concurrent_process_cache_status_fixture() {}\n",
    )
    .expect("write process pressure fixture");
    let state = ResolvedState::resolve_with_state_home(&project_root, &state_home)
        .expect("resolve process pressure state");
    let process_count = 8usize;
    let current_exe = env::current_exe().expect("locate current test binary");
    let mut children = Vec::new();

    for writer_id in 0..process_count {
        children.push((
            writer_id,
            Command::new(&current_exe)
                .arg("--exact")
                .arg("db_engine::db_engine_cache_status_process_pressure_helper")
                .arg("--nocapture")
                .env("ASP_TURSO_CACHE_PROCESS_PRESSURE_CHILD", "1")
                .env("ASP_TURSO_CACHE_PROCESS_PRESSURE_PROJECT_ROOT", &project_root)
                .env("ASP_TURSO_CACHE_PROCESS_PRESSURE_STATE_HOME", &state_home)
                .env("ASP_TURSO_CACHE_PROCESS_PRESSURE_WRITER_ID", writer_id.to_string())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("spawn process cache pressure writer"),
        ));
    }

    for (writer_id, child) in children {
        let output = child
            .wait_with_output()
            .expect("wait for process cache pressure writer");
        assert!(
            output.status.success(),
            "process cache pressure writer {writer_id} failed: status={} stdout={} stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let read_session = ClientDbEngine::open_read_session_client_dir(&state.paths.client_dir)
        .expect("open final process pressure read session")
        .expect("final process pressure read session exists");
    for writer_id in 0..process_count {
        let fingerprint = format!("fnv64:process-cache-status-{writer_id}-3");
        let hit = read_session
            .lookup_generation_request(
                &LanguageId::from("rust"),
                &ProviderId::from("rs-harness"),
                &project_root,
                &CacheExportMethod::from("search/prime"),
                Some(fingerprint),
            )
            .expect("lookup final process pressure generation")
            .unwrap_or_else(|| panic!("missing process pressure generation writer={writer_id}"));
        assert_eq!(
            hit.artifact_ids[0].as_str(),
            format!("search/process-cache-status-{writer_id}-3.json")
        );
    }
    assert!(state.paths.client_dir.join("client.turso").exists());

    let _ = std::fs::remove_dir_all(project_root);
    let _ = std::fs::remove_dir_all(state_home);
}

fn concurrent_cache_status_manifest(
    client_dir: &PathBuf,
    project_root: &PathBuf,
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

fn process_cache_status_manifest(
    client_dir: &PathBuf,
    project_root: &PathBuf,
    writer_id: usize,
    iteration: usize,
) -> ClientCacheManifest {
    serde_json::from_value(json!({
        "schemaId": "agent.semantic-protocols.client-cache-manifest",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.client",
        "protocolVersion": "1",
        "cacheRoot": client_dir.display().to_string(),
        "generations": [
            {
                "generationId": format!("rust-process-cache-status-{writer_id}-{iteration}"),
                "languageId": "rust",
                "providerId": "rs-harness",
                "providerVersion": "0.1.0",
                "exportMethod": "search/prime",
                "projectRoot": project_root.display().to_string(),
                "packageRoot": ".",
                "schemaIds": ["agent.semantic-protocols.semantic-search-packet"],
                "cacheStatus": "hit",
                "rawSourceStored": false,
                "requestFingerprint": format!("fnv64:process-cache-status-{writer_id}-{iteration}"),
                "fileHashes": [{
                    "path": "src/lib.rs",
                    "sha256": "2222222222222222222222222222222222222222222222222222222222222222",
                    "byteLen": 1,
                    "mtimeMs": writer_id * 10 + iteration
                }],
                "artifactIds": [format!("search/process-cache-status-{writer_id}-{iteration}.json")]
            }
        ]
    }))
    .expect("process cache status manifest fixture")
}
