use super::ResidentInteractiveCommand;

#[test]
fn resident_bootstrap_command_is_one_v1_argv() {
    let resident_name = "asp-testing".into();
    let root_session_id = "root-session-test".into();
    let command = ResidentInteractiveCommand::bootstrap(&resident_name, Some(&root_session_id));
    assert_eq!(command.schema_version, "1");
    assert_eq!(
        command.argv,
        [
            "asp",
            "agent",
            "session",
            "bootstrap",
            "--name",
            "asp-testing",
            "--root-session-id",
            "root-session-test",
        ]
    );
}

#[test]
fn resident_bootstrap_command_carries_semantic_dispatch_inputs() {
    let resident_name = "asp-testing".into();
    let root_session_id = "root-session-test".into();
    let command = ResidentInteractiveCommand::bootstrap_with_dispatch(
        &resident_name,
        Some(&root_session_id),
        Some("dispatch-execution-receipt.v1"),
        Some("[\"/usr/bin/true\"]"),
    );
    assert_eq!(
        command.argv,
        [
            "asp",
            "agent",
            "session",
            "bootstrap",
            "--name",
            "asp-testing",
            "--root-session-id",
            "root-session-test",
            "--receipt-kind",
            "dispatch-execution-receipt.v1",
            "--command-json",
            "[\"/usr/bin/true\"]",
        ]
    );
}
