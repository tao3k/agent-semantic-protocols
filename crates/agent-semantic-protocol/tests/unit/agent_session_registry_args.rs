use super::{SessionArgs, SessionCommand, session_usage};

#[test]
fn agent_session_state_root_is_not_a_client_option() {
    let error = SessionArgs::parse(&[
        "bootstrap".to_string(),
        "--state-root".to_string(),
        "/tmp/not-a-client-authority".to_string(),
    ])
    .err()
    .expect("agent session clients must not select a registry path");

    assert_eq!(error, "unknown session flag `--state-root`");
    assert!(!session_usage().contains("--state-root"));
}

#[test]
fn agent_session_bootstrap_keeps_the_runtime_server_route() {
    let args = SessionArgs::parse(&["bootstrap".to_string()])
        .expect("bootstrap must remain a valid session command");

    assert!(matches!(args.command, SessionCommand::Bootstrap));
}
