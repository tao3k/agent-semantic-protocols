#[test]
fn public_server_help_exposes_only_global_lifecycle_commands() {
    let help = super::runtime_server_command().render_help().to_string();
    for command in ["start", "status", "stop", "restart"] {
        assert!(
            help.contains(command),
            "missing public lifecycle command {command}"
        );
    }
    for internal in ["reconcile", "daemon", "telemetry", "workspace"] {
        assert!(
            !help.contains(internal),
            "public lifecycle help leaked internal surface {internal}: {help}"
        );
    }
}

#[test]
fn public_server_lifecycle_rejects_workspace_identity() {
    use clap::Parser;

    for command in ["start", "status", "stop", "restart"] {
        let parsed = super::ServerArgs::try_parse_from([
            "asp server",
            command,
            "--workspace",
            "/tmp/project",
        ]);
        assert!(
            parsed.is_err(),
            "{command} must not expose workspace lifecycle identity"
        );
    }
}

#[test]
fn operator_cold_start_budget_is_distinct_from_non_blocking_runtime_ensure() {
    assert_eq!(
        super::OPERATOR_RUNTIME_SERVER_STARTUP_BUDGET,
        std::time::Duration::from_secs(5)
    );
    assert_eq!(
        super::RUNTIME_SERVER_SUPERVISOR_EXECUTION_BUDGET,
        std::time::Duration::from_millis(800)
    );
    assert!(
        super::OPERATOR_RUNTIME_SERVER_STARTUP_BUDGET
            > super::RUNTIME_SERVER_SUPERVISOR_EXECUTION_BUDGET
    );
}
