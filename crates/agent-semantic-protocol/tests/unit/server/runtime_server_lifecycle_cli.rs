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
fn query_scope_derives_identity_without_catalog_or_bootstrap() {
    let project_root = std::env::current_dir()
        .expect("current project root")
        .canonicalize()
        .expect("canonical project root");
    let expected = agent_semantic_client_db::AgentSessionRegistry::workspace_id(&project_root)
        .expect("derive workspace identity");
    let (actual, canonical_root) =
        super::runtime_server_query_workspace_scope(&project_root).expect("derive query scope");
    assert_eq!(actual, expected);
    assert_eq!(canonical_root, project_root);
}

#[tokio::test]
async fn unchanged_provider_catalog_does_not_reconcile_runtime() {
    let state_home = tempfile::tempdir().expect("isolated ASP State Home");
    let disposition =
        super::reconcile_runtime_server_after_provider_catalog_change(state_home.path(), false)
            .await
            .expect("unchanged catalog reconciliation");
    assert_eq!(disposition, "current");
}

#[tokio::test]
async fn provider_install_does_not_start_an_absent_runtime() {
    let state_home = tempfile::tempdir().expect("isolated ASP State Home");
    let disposition =
        super::reconcile_runtime_server_after_provider_catalog_change(state_home.path(), true)
            .await
            .expect("absent Runtime reconciliation");
    assert_eq!(disposition, "not-running");
}
