use super::{configured_graph_turbo_python_at_state_home, validate_graph_turbo_python};

#[cfg(unix)]
#[tokio::test]
async fn managed_graph_turbo_symlink_resolves_to_its_content_authority() {
    let state_home = tempfile::tempdir().unwrap();
    let runtime = state_home.path().join("runtime");
    std::fs::create_dir_all(&runtime).unwrap();
    let target = runtime.join("base-python");
    let locator = runtime.join("python");
    std::fs::write(&target, b"python").unwrap();
    std::os::unix::fs::symlink(&target, &locator).unwrap();

    assert_eq!(
        validate_graph_turbo_python(state_home.path(), locator)
            .await
            .unwrap(),
        target.canonicalize().unwrap()
    );
}

#[tokio::test]
async fn graph_turbo_python_locator_survives_environmentless_reconcile() {
    let state_home = tempfile::tempdir().unwrap();
    let locator = state_home
        .path()
        .join("runtime")
        .join("artifacts")
        .join("python");
    std::fs::create_dir_all(locator.parent().unwrap()).unwrap();
    std::fs::write(&locator, b"python").unwrap();

    let configured =
        configured_graph_turbo_python_at_state_home(state_home.path(), Some(locator.clone()))
            .await
            .unwrap();
    let reconciled = configured_graph_turbo_python_at_state_home(state_home.path(), None)
        .await
        .unwrap();

    let canonical = locator.canonicalize().unwrap();
    assert_eq!(configured, Some(canonical.clone()));
    assert_eq!(reconciled, Some(canonical));
}

#[tokio::test]
async fn project_virtual_environment_cannot_become_runtime_artifact_authority() {
    let state_home = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(state_home.path().join("runtime")).unwrap();
    let project = tempfile::tempdir().unwrap();
    let locator = project.path().join(".venv").join("bin").join("python3");
    std::fs::create_dir_all(locator.parent().unwrap()).unwrap();
    std::fs::write(&locator, b"python").unwrap();

    let error = configured_graph_turbo_python_at_state_home(state_home.path(), Some(locator))
        .await
        .expect_err("project-local Python must not become a managed Runtime artifact");
    assert!(
        error.contains("outside the managed Runtime root"),
        "{error}"
    );
}

#[tokio::test]
async fn graph_turbo_python_config_rejects_unknown_schema_version() {
    let state_home = tempfile::tempdir().unwrap();
    let config_dir = state_home.path().join("runtime").join("server");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(
        config_dir.join("graph-turbo-resident-config.v1.json"),
        br#"{
          "schemaId": "agent.semantic-protocols.semantic-graph-turbo-resident-config",
          "schemaVersion": "2",
          "pythonExecutionLocator": "/invalid/python"
        }"#,
    )
    .unwrap();

    let error = configured_graph_turbo_python_at_state_home(state_home.path(), None)
        .await
        .expect_err("unknown config versions must fail closed");

    assert_eq!(error, "Graph Turbo resident config schema mismatch");
}
