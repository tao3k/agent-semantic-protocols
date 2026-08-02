use agent_semantic_client_db::{AgentSessionRegistry, runtime_server_endpoint_path};
use tempfile::TempDir;

use crate::test_support::{StateHomeGuard, environment_lock, workspace};

#[test]
fn project_registry_selects_runtime_proxy_from_published_descriptor() {
    let _environment = environment_lock();
    let fixture = TempDir::new().expect("create agent-session proxy fixture");
    let state_home = fixture.path().join("state");
    std::fs::create_dir_all(&state_home).expect("create State Home fixture");
    let _state_home = StateHomeGuard::install(&state_home);
    let (project_root, resolved, _) = workspace(fixture.path(), "project");
    let endpoint_path = runtime_server_endpoint_path(&resolved.state_home);
    std::fs::create_dir_all(endpoint_path.parent().expect("runtime endpoint parent"))
        .expect("create runtime endpoint directory");
    std::fs::write(&endpoint_path, b"{}\n").expect("publish endpoint descriptor sentinel");

    assert!(endpoint_path.is_file());
    let registry = AgentSessionRegistry::open_existing_project_read_only(&project_root)
        .expect("open project registry")
        .expect(
            "a published runtime endpoint descriptor must select the typed runtime proxy even when the local registry does not exist",
        );
    let error = registry
        .query_sessions("proxy-test-project", None, None)
        .expect_err("the intentionally incomplete endpoint descriptor must reject the IPC call");
    assert!(error.contains("Runtime Server") || error.contains("runtime server"));
    assert!(!error.contains("Turso agent session registry"));
}

#[test]
fn project_registry_never_direct_opens_without_runtime_server_endpoint() {
    let _environment = environment_lock();
    let fixture = TempDir::new().expect("create agent-session fail-closed fixture");
    let state_home = fixture.path().join("state");
    std::fs::create_dir_all(&state_home).expect("create State Home fixture");
    let _state_home = StateHomeGuard::install(&state_home);
    let (project_root, resolved, _) = workspace(fixture.path(), "project");
    let endpoint_path = runtime_server_endpoint_path(&resolved.state_home);

    assert!(!endpoint_path.exists());
    let error = match AgentSessionRegistry::open_or_create_project(&project_root) {
        Ok(_) => panic!("project clients must fail closed when typed IPC is unavailable"),
        Err(error) => error,
    };
    assert!(error.contains("Runtime Server"));
    assert!(error.contains("direct-open is forbidden"));
    assert!(!state_home.join("session-registry.turso").exists());
}

#[cfg(unix)]
#[test]
fn project_registry_selects_runtime_proxy_from_unix_socket_endpoint() {
    let _environment = environment_lock();
    let fixture = TempDir::new().expect("create Unix endpoint proxy fixture");
    let state_home = fixture.path().join("state");
    std::fs::create_dir_all(&state_home).expect("create State Home fixture");
    let _state_home = StateHomeGuard::install(&state_home);
    let (project_root, resolved, _) = workspace(fixture.path(), "project");
    let endpoint_path = runtime_server_endpoint_path(&resolved.state_home);
    std::fs::create_dir_all(endpoint_path.parent().expect("runtime endpoint parent"))
        .expect("create runtime endpoint directory");
    let _listener = std::os::unix::net::UnixListener::bind(&endpoint_path)
        .expect("bind Runtime Server endpoint socket");

    assert!(!endpoint_path.is_file());
    assert!(
        AgentSessionRegistry::open_existing_project_read_only(&project_root)
            .expect("inspect project registry route")
            .is_some(),
        "a published Unix socket endpoint must select the typed runtime proxy"
    );
    assert!(!state_home.join("session-registry.turso").exists());
}
