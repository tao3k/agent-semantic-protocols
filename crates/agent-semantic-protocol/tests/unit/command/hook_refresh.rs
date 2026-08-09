use super::run_refresh;

#[test]
fn refresh_rejects_a_caller_supplied_workspace_path() {
    let error = run_refresh(&[
        "--client".to_owned(),
        "codex".to_owned(),
        "/tmp/not-the-current-workspace".to_owned(),
    ])
    .expect_err("Hook refresh must derive its workspace identity");

    assert!(error.contains("current directory"), "{error}");
}

#[test]
fn refresh_rejects_a_missing_client_value() {
    let error = run_refresh(&["--client".to_owned()])
        .expect_err("Hook refresh must reject a missing client value");

    assert!(error.contains("requires a value"), "{error}");
}
