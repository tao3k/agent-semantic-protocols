
#[test]
fn db_engine_artifact_events_use_active_turso_path_without_retired_db_control() {
    let client_dir = temp_root("db-engine-artifact-events-client");
    let event = ClientDbArtifactEvent {
        artifact_path: "prompt-output/rust.command.json".to_string(),
        event_ordinal: 0,
        timestamp_ms: 1000,
        kind: "search/owner".to_string(),
        language: "rust".to_string(),
        method: "query".to_string(),
        target: "owner".to_string(),
        query: "ClientDbEngine".to_string(),
        project_root: "/tmp/project".to_string(),
        project_root_arg: ".".to_string(),
        bytes: 128,
    };
    let rewritten = ClientDbArtifactEvent {
        timestamp_ms: 1200,
        bytes: 256,
        ..event.clone()
    };

    let written = ClientDbEngine::upsert_artifact_events_from_client_dir(
        &client_dir,
        &[event.clone(), rewritten],
    )
    .expect("write Turso artifact events");
    let all = ClientDbEngine::lookup_artifact_events_from_client_dir(&client_dir, None, 10)
        .expect("read Turso artifact events");

    assert_eq!(written, 2);
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].artifact_path, event.artifact_path);
    assert_eq!(all[0].timestamp_ms, 1200);
    assert_eq!(all[0].bytes, 256);
    assert!(client_dir.join("client.turso").exists());
    let _ = fs::remove_dir_all(client_dir);
}

#[test]
fn db_engine_artifact_event_writes_survive_concurrent_agent_stress() {
    let client_dir = Arc::new(temp_root("db-engine-artifact-events-concurrent-client"));
    let writer_count = 12usize;
    let start = Arc::new(Barrier::new(writer_count));
    let mut writers = Vec::new();

    for writer_id in 0..writer_count {
        let client_dir = Arc::clone(&client_dir);
        let start = Arc::clone(&start);
        writers.push(thread::spawn(move || {
            start.wait();
            let event = ClientDbArtifactEvent {
                artifact_path: format!("prompt-output/agent-{writer_id}.command.json"),
                event_ordinal: 0,
                timestamp_ms: 10_000 + writer_id as i64,
                kind: "search/owner".to_string(),
                language: "rust".to_string(),
                method: "query".to_string(),
                target: "owner".to_string(),
                query: format!("ConcurrentAgent{writer_id}"),
                project_root: "/tmp/project".to_string(),
                project_root_arg: ".".to_string(),
                bytes: 128 + writer_id as u64,
            };
            ClientDbEngine::upsert_artifact_events_from_client_dir(client_dir.as_ref(), &[event])
                .map_err(|error| format!("writer {writer_id} failed: {error}"))
        }));
    }

    let mut total_written = 0u32;
    for writer in writers {
        total_written += writer
            .join()
            .expect("join concurrent artifact event writer")
            .expect("write concurrent artifact event");
    }

    let all = ClientDbEngine::lookup_artifact_events_from_client_dir(
        client_dir.as_ref(),
        None,
        writer_count as u32,
    )
    .expect("read concurrent Turso artifact events");
    assert_eq!(total_written, writer_count as u32);
    assert_eq!(all.len(), writer_count);
    for writer_id in 0..writer_count {
        assert!(
            all.iter().any(|event| event.artifact_path
                == format!("prompt-output/agent-{writer_id}.command.json")),
            "missing writer {writer_id} event in {all:?}"
        );
    }
    let _ = fs::remove_dir_all(client_dir.as_ref());
}

#[test]
fn db_engine_artifact_event_process_writer_helper() {
    if env::var("ASP_TURSO_PROCESS_STRESS_CHILD").ok().as_deref() != Some("1") {
        return;
    }
    let client_dir = PathBuf::from(
        env::var("ASP_TURSO_PROCESS_STRESS_CLIENT_DIR")
            .expect("ASP_TURSO_PROCESS_STRESS_CLIENT_DIR"),
    );
    let writer_id: usize = env::var("ASP_TURSO_PROCESS_STRESS_WRITER_ID")
        .expect("ASP_TURSO_PROCESS_STRESS_WRITER_ID")
        .parse()
        .expect("parse ASP_TURSO_PROCESS_STRESS_WRITER_ID");
    let event = ClientDbArtifactEvent {
        artifact_path: format!("prompt-output/process-agent-{writer_id}.command.json"),
        event_ordinal: 0,
        timestamp_ms: 20_000 + writer_id as i64,
        kind: "search/owner".to_string(),
        language: "rust".to_string(),
        method: "query".to_string(),
        target: "owner".to_string(),
        query: format!("ProcessConcurrentAgent{writer_id}"),
        project_root: "/tmp/project".to_string(),
        project_root_arg: ".".to_string(),
        bytes: 256 + writer_id as u64,
    };
    ClientDbEngine::upsert_artifact_events_from_client_dir(&client_dir, &[event])
        .expect("process writer should write Turso artifact event");
}

#[test]
fn db_engine_artifact_event_writes_survive_concurrent_agent_process_stress() {
    let client_dir = temp_root("db-engine-artifact-events-concurrent-process-client");
    let writer_count = 6usize;
    let current_exe = env::current_exe().expect("locate current test binary");
    let mut children = Vec::new();

    for writer_id in 0..writer_count {
        children.push(
            Command::new(&current_exe)
                .arg("--exact")
                .arg("db_engine::db_engine_artifact_event_process_writer_helper")
                .arg("--nocapture")
                .env("ASP_TURSO_PROCESS_STRESS_CHILD", "1")
                .env("ASP_TURSO_PROCESS_STRESS_CLIENT_DIR", &client_dir)
                .env("ASP_TURSO_PROCESS_STRESS_WRITER_ID", writer_id.to_string())
                .spawn()
                .expect("spawn process artifact writer"),
        );
    }

    for mut child in children {
        let status = child.wait().expect("wait for process artifact writer");
        assert!(status.success(), "process artifact writer failed: {status}");
    }

    let all = ClientDbEngine::lookup_artifact_events_from_client_dir(
        &client_dir,
        None,
        writer_count as u32,
    )
    .expect("read process-concurrent Turso artifact events");
    assert_eq!(all.len(), writer_count);
    for writer_id in 0..writer_count {
        assert!(
            all.iter().any(|event| event.artifact_path
                == format!("prompt-output/process-agent-{writer_id}.command.json")),
            "missing process writer {writer_id} event in {all:?}"
        );
    }
    let _ = fs::remove_dir_all(client_dir);
}
