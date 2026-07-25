fn artifact_event_fixture(
    artifact_path: String,
    timestamp_ms: i64,
    query: String,
    bytes: u64,
) -> Result<ClientDbArtifactEvent, String> {
    ClientDbArtifactEvent::builder()
        .artifact_path(artifact_path)
        .event_ordinal(0)
        .timestamp_ms(timestamp_ms)
        .kind("search/owner")
        .language(
            agent_semantic_client_core::LanguageId::try_new("rust")
                .expect("valid fixture language"),
        )
        .method("query")
        .target("owner")
        .query(query)
        .project_root("/tmp/project")
        .project_root_arg(".")
        .bytes(bytes)
        .build()
}

#[test]
fn db_engine_artifact_events_use_active_turso_path_without_retired_db_control() {
    let client_dir = temp_root("db-engine-artifact-events-client");
    let event = artifact_event_fixture(
        "prompt-output/rust.command.json".to_string(),
        1000,
        "ClientDbEngine".to_string(),
        128,
    )
    .expect("build artifact event");
    let rewritten = artifact_event_fixture(
        "prompt-output/rust.command.json".to_string(),
        1200,
        "ClientDbEngine".to_string(),
        256,
    )
    .expect("build rewritten artifact event");

    let written = ClientDbEngine::upsert_artifact_events_from_client_dir(
        &client_dir,
        &[event.clone(), rewritten],
    )
    .expect("write Turso artifact events");
    let all = ClientDbEngine::lookup_artifact_events_from_client_dir(&client_dir, None, 10)
        .expect("read Turso artifact events");

    assert_eq!(written, 2);
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].artifact_path(), event.artifact_path());
    assert_eq!(all[0].timestamp_ms(), 1200);
    assert_eq!(all[0].bytes(), 256);
    assert!(client_dir.join("facts.turso").exists());
    let _ = fs::remove_dir_all(client_dir);
}

#[test]
fn db_engine_artifact_event_writes_survive_concurrent_agent_stress() {
    let client_dir = Arc::new(temp_root("db-engine-artifact-events-concurrent-client"));
    ClientDbEngine::open_write_session_client_dir(client_dir.as_ref())
        .expect("stage canonical Turso database before concurrent artifact writes");
    let writer_count = 12usize;
    let start = Arc::new(Barrier::new(writer_count));
    let mut writers = Vec::new();

    for writer_id in 0..writer_count {
        let client_dir = Arc::clone(&client_dir);
        let start = Arc::clone(&start);
        writers.push(thread::spawn(move || {
            start.wait();
            let event = artifact_event_fixture(
                format!("prompt-output/agent-{writer_id}.command.json"),
                10_000 + writer_id as i64,
                format!("ConcurrentAgent{writer_id}"),
                128 + writer_id as u64,
            )
            .map_err(|error| format!("writer {writer_id} event is invalid: {error}"))?;
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
            all.iter().any(|event| event.artifact_path()
                == format!("prompt-output/agent-{writer_id}.command.json")),
            "missing writer {writer_id} event in {all:?}"
        );
    }
    let _ = fs::remove_dir_all(client_dir.as_ref());
}
