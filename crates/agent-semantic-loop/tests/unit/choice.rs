use super::ResidentInteractiveCommand;

#[test]
fn resident_bootstrap_command_is_one_v1_argv() {
    let command = ResidentInteractiveCommand::bootstrap("asp-testing", Some("root-session-test"));
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
    let command = ResidentInteractiveCommand::bootstrap_with_dispatch(
        "asp-testing",
        Some("root-session-test"),
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
