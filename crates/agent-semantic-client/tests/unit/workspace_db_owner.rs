use std::io::BufRead;

const CHILD_ENV: &str = "ASP_WORKSPACE_RESIDENT_READINESS_CHILD";
const WORKSPACE_ENV: &str = "ASP_WORKSPACE_RESIDENT_READINESS_WORKSPACE";

#[test]
fn readiness_receipt_is_immediately_healthy_and_typed_shutdown_exits() {
    if std::env::var_os(CHILD_ENV).is_some() {
        let workspace = std::path::PathBuf::from(
            std::env::var_os(WORKSPACE_ENV).expect("child workspace path"),
        );
        super::serve(&workspace, "runtime-digest-readiness")
            .expect("serve resident readiness child");
        return;
    }

    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("resident readiness clock")
        .as_nanos();
    let fixture = std::env::temp_dir().join(format!(
        "asp-resident-readiness-{}-{unique}",
        std::process::id()
    ));
    let workspace = fixture.join("workspace");
    std::fs::create_dir_all(&workspace).expect("create resident readiness workspace");
    let state_home = fixture.join("state");
    let test_name = std::thread::current()
        .name()
        .expect("test harness thread name")
        .to_owned();
    let mut child =
        std::process::Command::new(std::env::current_exe().expect("current test binary"))
            .arg("--exact")
            .arg(test_name)
            .arg("--nocapture")
            .env(CHILD_ENV, "1")
            .env(WORKSPACE_ENV, &workspace)
            .env("ASP_STATE_HOME", &state_home)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("spawn resident readiness child");
    let stdout = child.stdout.take().expect("resident readiness stdout");
    let mut stdout = std::io::BufReader::new(stdout);
    let endpoint_path = loop {
        let mut line = String::new();
        let bytes = stdout
            .read_line(&mut line)
            .expect("read resident readiness receipt");
        assert_ne!(bytes, 0, "resident exited before readiness");
        if !line.starts_with("[workspace-resident-service-ready]") {
            continue;
        }
        break line
            .split_whitespace()
            .find_map(|field| field.strip_prefix("endpoint="))
            .map(std::path::PathBuf::from)
            .expect("readiness receipt carries endpoint path");
    };
    let endpoint: agent_semantic_client_db::workspace_db_ipc::WorkspaceDbOwnerEndpoint =
        serde_json::from_slice(
            &std::fs::read(&endpoint_path).expect("read published resident endpoint"),
        )
        .expect("decode published resident endpoint");
    let started = std::time::Instant::now();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build readiness client runtime");
    runtime.block_on(async {
        let session =
            agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession::new(endpoint);
        session
            .health()
            .await
            .expect("readiness must imply immediate transport health");
        session
            .shutdown()
            .await
            .expect("typed shutdown must retire readiness child");
    });
    assert!(
        started.elapsed() < std::time::Duration::from_millis(50),
        "readiness health and typed shutdown exceeded 50ms: {:?}",
        started.elapsed()
    );
    let status = child.wait().expect("wait for typed resident shutdown");
    assert!(
        status.success(),
        "resident child must exit successfully after typed shutdown: {status}"
    );
    std::fs::remove_dir_all(&fixture).expect("remove resident readiness fixture");
}
