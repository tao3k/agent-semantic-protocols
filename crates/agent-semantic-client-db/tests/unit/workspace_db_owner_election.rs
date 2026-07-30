use tempfile::TempDir;

use agent_semantic_client_db::workspace_db_ipc::{
    WorkspaceDbOwnerEndpoint, WorkspaceDbOwnerRetirement, prepare_workspace_db_owner_endpoint,
    remove_stale_workspace_db_owner_socket, try_acquire_workspace_db_owner_election,
    try_retire_workspace_db_owner_endpoint,
};

#[test]
fn one_workspace_has_one_process_lifetime_owner_lease() {
    let temp = TempDir::new().expect("create owner election tempfile");
    let runtime_base = temp.path().join("runtime");
    let first = try_acquire_workspace_db_owner_election(&runtime_base, "workspace-a")
        .expect("acquire first owner election")
        .expect("first owner wins election");
    assert!(
        try_acquire_workspace_db_owner_election(&runtime_base, "workspace-a")
            .expect("probe competing owner election")
            .is_none()
    );

    drop(first);

    assert!(
        try_acquire_workspace_db_owner_election(&runtime_base, "workspace-a")
            .expect("reacquire released owner election")
            .is_some()
    );
}

#[test]
fn elected_owner_removes_only_its_stable_socket() {
    let temp = TempDir::new().expect("create stale socket tempfile");
    let runtime_base = temp.path().join("runtime");
    let election = try_acquire_workspace_db_owner_election(&runtime_base, "workspace-a")
        .expect("acquire owner election")
        .expect("owner wins election");
    let socket_path = runtime_base.join("workspace-a.sock");
    std::fs::write(&socket_path, b"stale").expect("write stale socket fixture");
    let endpoint = WorkspaceDbOwnerEndpoint {
        schema_id: "agent.semantic-protocols.workspace-db-owner-endpoint.v1".to_owned(),
        schema_version: "1".to_owned(),
        workspace_identity: "workspace-a".to_owned(),
        transport_contract_digest:
            agent_semantic_client_db::workspace_db_ipc::workspace_db_owner_transport_contract_digest(
            ),
        owner_epoch: 1,
        owner_pid: 7001,
        runtime_binary_path: "/runtime/asp".to_owned(),
        runtime_binary_digest: "runtime-digest-1".to_owned(),
        binding_token: "binding".to_owned(),
        socket_path: socket_path.to_string_lossy().into_owned(),
    };

    remove_stale_workspace_db_owner_socket(&election, &endpoint)
        .expect("remove elected owner's stale socket");

    assert!(!socket_path.exists());
}

#[test]
fn workspace_socket_identity_is_unique_across_owner_epochs() {
    let temp = TempDir::new().expect("create owner epoch socket tempfile");
    let runtime_base = temp.path().join("runtime");
    let election = try_acquire_workspace_db_owner_election(&runtime_base, "workspace-a")
        .expect("acquire owner election")
        .expect("owner wins election");
    let first = prepare_workspace_db_owner_endpoint(
        &runtime_base,
        "workspace-a",
        1,
        7001,
        std::path::Path::new("/runtime/asp"),
        "runtime-digest-1",
        "binding-a",
    )
    .expect("prepare first endpoint");
    let successor = prepare_workspace_db_owner_endpoint(
        &runtime_base,
        "workspace-a",
        2,
        7002,
        std::path::Path::new("/runtime/asp"),
        "runtime-digest-2",
        "binding-b",
    )
    .expect("prepare successor endpoint");

    assert_ne!(
        first.socket_path, successor.socket_path,
        "stale cleanup for one owner epoch must not unlink its successor's listener"
    );
    std::fs::write(&first.socket_path, b"stale").expect("write stale owner socket");
    std::fs::write(&successor.socket_path, b"current").expect("write successor owner socket");
    remove_stale_workspace_db_owner_socket(&election, &first)
        .expect("remove only the stale owner epoch socket");
    assert!(!std::path::Path::new(&first.socket_path).exists());
    assert!(
        std::path::Path::new(&successor.socket_path).exists(),
        "the current owner epoch socket must survive stale cleanup"
    );
}

#[test]
fn endpoint_retirement_is_atomic_with_owner_election() {
    let temp = TempDir::new().expect("create atomic retirement fixture");
    let runtime_base = temp.path().join("runtime");
    let endpoint_path = temp.path().join("endpoint.json");
    let endpoint = prepare_workspace_db_owner_endpoint(
        &runtime_base,
        "workspace-atomic-retirement",
        19,
        7019,
        std::path::Path::new("/runtime/asp"),
        "runtime-digest-19",
        "binding-19",
    )
    .expect("prepare retirement endpoint");
    std::fs::write(&endpoint.socket_path, b"listener").expect("write socket fixture");
    std::fs::write(
        &endpoint_path,
        serde_json::to_vec(&endpoint).expect("encode endpoint fixture"),
    )
    .expect("write endpoint fixture");

    let owner =
        try_acquire_workspace_db_owner_election(&runtime_base, "workspace-atomic-retirement")
            .expect("acquire current owner election")
            .expect("current owner wins election");
    assert_eq!(
        try_retire_workspace_db_owner_endpoint(
            &runtime_base,
            "workspace-atomic-retirement",
            &endpoint_path,
        )
        .expect("probe retirement while owner is active"),
        WorkspaceDbOwnerRetirement::ElectionHeld
    );
    assert!(endpoint_path.exists());
    assert!(std::path::Path::new(&endpoint.socket_path).exists());

    drop(owner);
    assert_eq!(
        try_retire_workspace_db_owner_endpoint(
            &runtime_base,
            "workspace-atomic-retirement",
            &endpoint_path,
        )
        .expect("retire endpoint after owner releases election"),
        WorkspaceDbOwnerRetirement::Retired
    );
    assert!(!endpoint_path.exists());
    assert!(!std::path::Path::new(&endpoint.socket_path).exists());
}

#[test]
fn competing_process_cannot_open_the_same_workspace_owner_lane() {
    const CHILD_MARKER: &str = "workspace-owner-election-child-ready";
    const CHILD_ENV: &str = "ASP_WORKSPACE_OWNER_ELECTION_CHILD";
    const RUNTIME_ENV: &str = "ASP_WORKSPACE_OWNER_ELECTION_RUNTIME";

    if std::env::var_os(CHILD_ENV).is_some() {
        let runtime_base =
            std::path::PathBuf::from(std::env::var_os(RUNTIME_ENV).expect("child runtime path"));
        let _lease = try_acquire_workspace_db_owner_election(&runtime_base, "workspace-a")
            .expect("child election")
            .expect("child owns workspace lane");
        println!("{CHILD_MARKER}");
        std::io::Write::flush(&mut std::io::stdout()).expect("flush child marker");
        let mut release = [0_u8; 1];
        let _ = std::io::Read::read(&mut std::io::stdin(), &mut release);
        return;
    }

    let temp = TempDir::new().expect("create process election tempfile");
    let runtime_base = temp.path().join("runtime");
    let child_test_name = std::thread::current()
        .name()
        .expect("test harness thread name")
        .to_owned();
    let mut child =
        std::process::Command::new(std::env::current_exe().expect("current test binary"))
            .arg("--exact")
            .arg(child_test_name)
            .arg("--nocapture")
            .env(CHILD_ENV, "1")
            .env(RUNTIME_ENV, &runtime_base)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("spawn elected child owner");
    let child_stdout = child.stdout.take().expect("child stdout");
    let mut child_stdout = std::io::BufReader::new(child_stdout);
    loop {
        let mut line = String::new();
        let bytes = std::io::BufRead::read_line(&mut child_stdout, &mut line)
            .expect("read child handshake");
        assert_ne!(bytes, 0, "child exited before acquiring owner lane");
        if line.contains(CHILD_MARKER) {
            break;
        }
    }

    assert!(
        try_acquire_workspace_db_owner_election(&runtime_base, "workspace-a")
            .expect("competing process election")
            .is_none(),
        "a second process must fail before opening Turso"
    );
    child.kill().expect("force elected child owner exit");
    let status = child.wait().expect("reap forced child owner");
    assert!(!status.success(), "forced owner exit must be observable");
    assert!(
        try_acquire_workspace_db_owner_election(&runtime_base, "workspace-a")
            .expect("successor process election")
            .is_some(),
        "the lane must recover immediately after owner process exit"
    );
}

#[test]
fn losing_owner_election_is_submillisecond() {
    const SAMPLES: u32 = 128;
    const AVERAGE_BUDGET_NANOS: u128 = 1_000_000;

    let temp = TempDir::new().expect("create election performance tempfile");
    let runtime_base = temp.path().join("runtime");
    let _owner = try_acquire_workspace_db_owner_election(&runtime_base, "workspace-a")
        .expect("acquire elected owner")
        .expect("first owner wins");
    let started = std::time::Instant::now();
    for _ in 0..SAMPLES {
        assert!(
            try_acquire_workspace_db_owner_election(&runtime_base, "workspace-a")
                .expect("probe losing election")
                .is_none()
        );
    }
    let elapsed = started.elapsed();
    let average_nanos = elapsed.as_nanos() / u128::from(SAMPLES);
    eprintln!(
        "[workspace-db-owner-election-performance] samples={SAMPLES} elapsedMicros={} averageNanos={average_nanos} budgetNanos={AVERAGE_BUDGET_NANOS}",
        elapsed.as_micros()
    );
    assert!(
        average_nanos < AVERAGE_BUDGET_NANOS,
        "losing owner election averaged {average_nanos}ns; budget is {AVERAGE_BUDGET_NANOS}ns"
    );
}
