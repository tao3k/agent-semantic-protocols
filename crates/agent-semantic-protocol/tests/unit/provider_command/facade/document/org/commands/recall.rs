use crate::provider_command::support::{
    asp_command, make_executable, prepend_path, temp_project_root,
};

#[test]
fn asp_org_recall_plans_uses_explicit_memory_engine_binary() {
    let root = temp_project_root("org-document-command-recall-plans-binary");
    let org_artifacts = root
        .join(".cache")
        .join("agent-semantic-protocol")
        .join("artifacts")
        .join("org");
    let plans = org_artifacts.join("flow").join("plans");
    std::fs::create_dir_all(&plans).expect("create plans dir");
    std::fs::write(
        plans.join("agent-plan-binary-plan.org"),
        "* TODO Binary backed recall plan :agent:plan:\n:PROPERTIES:\n:CONTRACT_ORG: agent.plan.v1\n:ID: binary-plan\n:OBJECTIVE: Binary backed recall plan\n:NEXT_ACTION: keep the memory engine on a packaged binary path\n:END:\n",
    )
    .expect("write binary plan");
    let bin_dir = root.join("bin");
    std::fs::create_dir_all(&bin_dir).expect("create binary dir");
    let memory_engine = bin_dir.join("asp-memory-engine-test-binary");
    std::fs::write(
        &memory_engine,
        "#!/bin/sh\ncat >/dev/null\nprintf '%s\\n' '{\"plans\":[{\"id\":\"binary-plan\",\"score\":9.0,\"contextScore\":0.0,\"memoryScore\":9.0,\"recencyScore\":0.0}]}'\n",
    )
    .expect("write fake memory engine binary");
    make_executable(&memory_engine);

    let output = asp_command(&root)
        .env("ASP_MEMORY_ENGINE", &memory_engine)
        .args([
            "org",
            "recall",
            "plans",
            "--artifacts-root",
            org_artifacts.to_str().unwrap(),
            "--project",
            "repo",
            "--top-k",
            "1",
        ])
        .output()
        .expect("run asp org recall plans with explicit binary");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("recall stdout");
    assert!(stdout.contains("plan=\"binary-plan\""), "{stdout}");
    assert!(stdout.contains("memoryTransport=\"process\""), "{stdout}");
    assert!(
        stdout.contains("selectedBy=\"memory-engine+org-graph\""),
        "{stdout}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_org_recall_plans_prefers_source_runtime_over_path_memory_engine_binary() {
    let root = temp_project_root("org-document-command-recall-plans-path-binary");
    let org_artifacts = root
        .join(".cache")
        .join("agent-semantic-protocol")
        .join("artifacts")
        .join("org");
    let plans = org_artifacts.join("flow").join("plans");
    std::fs::create_dir_all(&plans).expect("create plans dir");
    std::fs::write(
        plans.join("agent-plan-path-binary-plan.org"),
        "* TODO PATH backed recall plan :agent:plan:\n:PROPERTIES:\n:CONTRACT_ORG: agent.plan.v1\n:ID: path-binary-plan\n:OBJECTIVE: PATH backed recall plan\n:NEXT_ACTION: prefer source runtime over path memory engine\n:END:\n",
    )
    .expect("write path binary plan");
    let bin_dir = root.join("bin");
    std::fs::create_dir_all(&bin_dir).expect("create binary dir");
    let memory_engine = bin_dir.join("asp-memory-engine");
    std::fs::write(
        &memory_engine,
        "#!/bin/sh\ncat >/dev/null\nprintf '%s\\n' '{\"plans\":[{\"id\":\"path-binary-plan\",\"score\":7.0,\"contextScore\":0.0,\"memoryScore\":7.0,\"recencyScore\":0.0}]}'\n",
    )
    .expect("write fake memory engine binary");
    make_executable(&memory_engine);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .args([
            "org",
            "recall",
            "plans",
            "--artifacts-root",
            org_artifacts.to_str().unwrap(),
            "--project",
            "repo",
            "--top-k",
            "1",
        ])
        .output()
        .expect("run asp org recall plans with path binary");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("recall stdout");
    assert!(stdout.contains("plan=\"path-binary-plan\""), "{stdout}");
    assert!(!stdout.contains("score=7.000"), "{stdout}");
    assert!(stdout.contains("memoryTransport=\"process\""), "{stdout}");
    assert!(stdout.contains("selectedBy="), "{stdout}");

    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
#[ignore = "org recall plans is not exposed by the current asp CLI"]
fn asp_org_recall_plans_uses_memory_engine_socket_worker() {
    let root = temp_project_root("org-document-command-recall-plans-socket-worker");
    let org_artifacts = root
        .join(".cache")
        .join("agent-semantic-protocol")
        .join("artifacts")
        .join("org");
    let plans = org_artifacts.join("flow").join("plans");
    std::fs::create_dir_all(&plans).expect("create plans dir");
    std::fs::write(
        plans.join("agent-plan-socket-worker-plan.org"),
        "* TODO Socket worker recall plan :agent:plan:\n:PROPERTIES:\n:CONTRACT_ORG: agent.plan.v1\n:ID: socket-worker-plan\n:OBJECTIVE: Socket worker recall plan\n:NEXT_ACTION: rank through resident memory worker\n:END:\n",
    )
    .expect("write socket worker plan");
    let socket_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let socket_path = std::path::PathBuf::from(format!(
        "/tmp/asp-memory-worker-{}-{socket_id}.sock",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&socket_path);
    let listener =
        std::os::unix::net::UnixListener::bind(&socket_path).expect("bind memory worker socket");
    let handle = std::thread::spawn(move || {
        listener
            .set_nonblocking(true)
            .expect("set worker listener nonblocking");
        let accept_started = std::time::Instant::now();
        let (mut stream, _) = loop {
            match listener.accept() {
                Ok(accepted) => break accepted,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if accept_started.elapsed() > std::time::Duration::from_secs(10) {
                        panic!("timed out waiting for memory worker request");
                    }
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                Err(error) => panic!("accept memory worker request: {error}"),
            }
        };
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .expect("set worker read timeout");
        let mut request = Vec::new();
        let mut reader = std::io::BufReader::new(stream.try_clone().expect("clone worker stream"));
        match std::io::BufRead::read_until(&mut reader, b'\n', &mut request) {
            Ok(_) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(error) => panic!("read worker request: {error}"),
        }
        let request = String::from_utf8_lossy(&request);
        assert!(request.contains("\"command\":\"rank-plans\""), "{request}");
        assert!(request.contains("\"payload\""), "{request}");
        assert!(request.contains("\"socket-worker-plan\""), "{request}");
        std::io::Write::write_all(
            &mut stream,
            b"{\"plans\":[{\"id\":\"socket-worker-plan\",\"score\":6.0,\"contextScore\":0.0,\"memoryScore\":6.0,\"recencyScore\":0.0}]}\n",
        )
        .expect("write worker response");
    });

    let mut child = asp_command(&root)
        .env("ASP_MEMORY_ENGINE_SOCKET", &socket_path)
        .args([
            "org",
            "recall",
            "plans",
            "--artifacts-root",
            org_artifacts.to_str().unwrap(),
            "--project",
            "repo",
            "--top-k",
            "1",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn asp org recall plans with socket worker");
    let started = std::time::Instant::now();
    let output = loop {
        if child
            .try_wait()
            .expect("poll asp org recall plans")
            .is_some()
        {
            break child
                .wait_with_output()
                .expect("collect asp org recall plans output");
        }
        if started.elapsed() > std::time::Duration::from_secs(10) {
            let _ = child.kill();
            let output = child
                .wait_with_output()
                .expect("collect timed out asp org recall plans output");
            panic!(
                "asp org recall plans with socket worker timed out after 10s\nstdout: {}\nstderr: {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    handle.join().expect("worker socket thread");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("recall stdout");
    assert!(stdout.contains("plan=\"socket-worker-plan\""), "{stdout}");
    assert!(
        stdout.contains("memoryTransport=\"socket:env\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("selectedBy=\"memory-engine+org-graph\""),
        "{stdout}"
    );

    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_file(socket_path);
}
