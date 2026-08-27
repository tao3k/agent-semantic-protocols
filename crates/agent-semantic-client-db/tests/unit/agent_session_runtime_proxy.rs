use agent_semantic_client_db::{
    AgentSessionRegistry, publish_runtime_server_endpoint, runtime_server_endpoint_path,
};
use std::os::unix::fs::PermissionsExt;

use crate::test_support::{StateHomeGuard, TestDir, environment_lock, workspace};

#[tokio::test]
async fn project_registry_rejects_invalid_runtime_endpoint_descriptor() {
    let _environment = environment_lock();
    let fixture = TestDir::new("agent-session-proxy");
    let state_home = fixture.path().join("state");
    std::fs::create_dir_all(&state_home).expect("create State Home fixture");
    let _state_home = StateHomeGuard::install(&state_home);
    let (project_root, resolved, _) = workspace(fixture.path(), "project");
    let endpoint_path = runtime_server_endpoint_path(&resolved.state_home)
        .expect("derive Runtime Server endpoint path");
    let runtime_base = endpoint_path.parent().expect("runtime endpoint parent");
    std::fs::create_dir_all(runtime_base).expect("create runtime endpoint directory");
    std::fs::write(&endpoint_path, b"{}\n").expect("publish endpoint descriptor sentinel");
    std::fs::set_permissions(&endpoint_path, std::fs::Permissions::from_mode(0o600))
        .expect("make invalid endpoint descriptor private");

    assert!(endpoint_path.is_file());
    let error = AgentSessionRegistry::open_runtime_project_proxy(&project_root)
        .await
        .err()
        .expect("an invalid Runtime Server endpoint descriptor must fail before IPC");
    assert!(
        error.contains("reasonKind=runtime-server-endpoint-identity-incomplete"),
        "unexpected invalid endpoint error: {error}"
    );
    assert!(!state_home.join("session-registry.turso").exists());
}

#[tokio::test]
async fn project_registry_never_direct_opens_without_runtime_server_endpoint() {
    let _environment = environment_lock();
    let fixture = TestDir::new("agent-session-fail-closed");
    let state_home = fixture.path().join("state");
    std::fs::create_dir_all(&state_home).expect("create State Home fixture");
    let _state_home = StateHomeGuard::install(&state_home);
    let project_fixture = TestDir::new("agent-session-unix-proxy");
    let (project_root, resolved, _) = workspace(project_fixture.path(), "project");
    let endpoint_path = runtime_server_endpoint_path(&resolved.state_home)
        .expect("derive Runtime Server endpoint path");

    assert!(!endpoint_path.exists());
    assert!(
        AgentSessionRegistry::open_runtime_project_proxy(&project_root)
            .await
            .expect("inspect project registry route")
            .is_none(),
        "an unpublished Runtime Server endpoint must not fall back to a direct DB open"
    );
    let error = match AgentSessionRegistry::open_or_create_project(&project_root).await {
        Ok(_) => panic!("project clients must fail closed when typed IPC is unavailable"),
        Err(error) => error,
    };
    assert!(error.contains("Runtime Server"));
    assert!(error.contains("direct-open is forbidden"));
    assert!(!state_home.join("session-registry.turso").exists());
}

#[cfg(unix)]
#[tokio::test]
async fn project_registry_selects_runtime_proxy_from_unix_socket_endpoint() {
    let _environment = environment_lock();
    let fixture = tempfile::Builder::new()
        .prefix("asp-ipc-")
        .tempdir_in("/tmp")
        .expect("create short Unix endpoint proxy fixture");
    let state_home = fixture.path().join("state");
    std::fs::create_dir_all(&state_home).expect("create State Home fixture");
    let _state_home = StateHomeGuard::install(&state_home);
    let (project_root, resolved, _) = workspace(fixture.path(), "project");
    let endpoint_path = runtime_server_endpoint_path(&resolved.state_home)
        .expect("derive Runtime Server endpoint path");
    std::fs::create_dir_all(endpoint_path.parent().expect("runtime endpoint parent"))
        .expect("create runtime endpoint directory");
    let runtime_artifact = state_home.join("runtime/bin/asp");
    std::fs::create_dir_all(runtime_artifact.parent().expect("runtime artifact parent"))
        .expect("create Runtime artifact directory");
    std::fs::write(&runtime_artifact, b"test-runtime-artifact")
        .expect("write immutable Runtime artifact fixture");
    let _published_registry = AgentSessionRegistry::open_or_create_state_root(&state_home)
        .expect("publish current V1 instance registry before endpoint publication");
    let mut endpoint = agent_semantic_client_db::prepare_runtime_server_endpoint(
        &state_home,
        &runtime_artifact,
        &agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(
            b"test-runtime-artifact",
        ),
        "dev",
        "blake3-256:0000000000000000000000000000000000000000000000000000000000000000",
        1,
        "test-binding-token",
    )
    .await
    .expect("prepare typed Runtime Server endpoint");
    let runtime_binary_identity =
        agent_semantic_artifacts::runtime_artifact_catalog::RuntimeBinaryIdentity::from_bytes(
            b"test-runtime-artifact",
        );
    endpoint.binary_content_digest = match &runtime_binary_identity {
        agent_semantic_artifacts::runtime_artifact_catalog::RuntimeBinaryIdentity::Content {
            digest,
        } => digest.to_string(),
    };
    endpoint.runtime_binary_identity = runtime_binary_identity.clone();
    endpoint.observed_runtime_binary_identity = runtime_binary_identity;
    let endpoint_value = serde_json::to_value(&endpoint).expect("encode endpoint fixture shape");
    assert_eq!(
        endpoint_value
            .pointer("/runtimeBinaryIdentity/identity/digest")
            .and_then(serde_json::Value::as_str),
        Some(endpoint.binary_content_digest.as_str()),
        "unexpected typed endpoint identity shape: {endpoint_value}"
    );
    std::fs::create_dir_all(
        std::path::Path::new(&endpoint.socket_path)
            .parent()
            .expect("Runtime socket parent"),
    )
    .expect("create Runtime socket directory");
    let _listener = std::os::unix::net::UnixListener::bind(&endpoint.socket_path)
        .expect("bind Runtime Server control socket");
    publish_runtime_server_endpoint(&endpoint_path, &endpoint)
        .await
        .expect("publish typed Runtime Server endpoint descriptor fixture");

    assert!(endpoint_path.is_file());
    assert!(
        AgentSessionRegistry::open_runtime_project_proxy(&project_root)
            .await
            .expect("inspect project registry route")
            .is_some(),
        "a published Unix socket endpoint must select the typed runtime proxy"
    );
    assert!(!state_home.join("session-registry.turso").exists());
}
