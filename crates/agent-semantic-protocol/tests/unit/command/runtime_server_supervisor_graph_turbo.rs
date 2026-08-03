use std::path::Path;

use super::{
    configured_graph_turbo_python_at_state_home, launchd_graph_turbo_environment,
    systemd_graph_turbo_environment, validate_graph_turbo_python,
};

#[test]
fn missing_graph_turbo_artifact_omits_supervisor_environment() {
    assert_eq!(launchd_graph_turbo_environment(None).unwrap(), "");
    assert_eq!(systemd_graph_turbo_environment(None).unwrap(), "");
}

#[test]
fn launchd_graph_turbo_artifact_is_xml_escaped() {
    let rendered =
        launchd_graph_turbo_environment(Some(Path::new("/runtime/graph&turbo/python<3>"))).unwrap();
    assert!(rendered.contains("<key>ASP_GRAPH_TURBO_PYTHON</key>"));
    assert!(rendered.contains("/runtime/graph&amp;turbo/python&lt;3&gt;"));
}

#[test]
fn systemd_graph_turbo_artifact_is_quoted_and_escaped() {
    let rendered =
        systemd_graph_turbo_environment(Some(Path::new("/runtime/graph\\turbo/python\"3")))
            .unwrap();
    assert_eq!(
        rendered,
        "Environment=\"ASP_GRAPH_TURBO_PYTHON=/runtime/graph\\\\turbo/python\\\"3\""
    );
}

#[cfg(unix)]
#[test]
fn validated_graph_turbo_python_preserves_a_virtualenv_symlink_locator() {
    let directory = tempfile::tempdir().unwrap();
    let target = directory.path().join("base-python");
    let locator = directory.path().join("venv-python");
    std::fs::write(&target, b"python").unwrap();
    std::os::unix::fs::symlink(&target, &locator).unwrap();

    assert_eq!(
        validate_graph_turbo_python(locator.clone()).unwrap(),
        locator
    );
}

#[test]
fn graph_turbo_python_locator_survives_environmentless_reconcile() {
    let state_home = tempfile::tempdir().unwrap();
    let locator = state_home.path().join("python");
    std::fs::write(&locator, b"python").unwrap();

    let configured =
        configured_graph_turbo_python_at_state_home(state_home.path(), Some(locator.clone()))
            .unwrap();
    let reconciled = configured_graph_turbo_python_at_state_home(state_home.path(), None).unwrap();

    assert_eq!(configured, Some(locator.clone()));
    assert_eq!(reconciled, Some(locator));
}

#[test]
fn graph_turbo_python_config_rejects_unknown_schema_version() {
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
        .expect_err("unknown config versions must fail closed");

    assert_eq!(error, "Graph Turbo resident config schema mismatch");
}
