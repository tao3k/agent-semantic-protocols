// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

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
fn legacy_bootstrap_is_rejected_by_the_registry_cli() {
    let error = SessionArgs::parse(&["bootstrap".to_string()])
        .err()
        .expect("the removed Rust-owned lifecycle command must not parse");

    assert_eq!(error, "unknown session flag `bootstrap`");
    assert!(!session_usage().contains("bootstrap"));
}

#[test]
fn control_plane_observation_commands_are_explicit() {
    let refresh = SessionArgs::parse(&[
        "control-plane".to_string(),
        "refresh".to_string(),
        "--root-session-id".to_string(),
        "root-1".to_string(),
    ])
    .expect("control-plane refresh must parse");
    let show = SessionArgs::parse(&["control-plane".to_string(), "show".to_string()])
        .expect("control-plane show must parse");

    assert!(matches!(
        refresh.command,
        SessionCommand::ControlPlaneRefresh
    ));
    assert_eq!(refresh.root_session_id.as_deref(), Some("root-1"));
    assert!(matches!(show.command, SessionCommand::ControlPlaneShow));
}

#[test]
fn control_plane_rejects_execution_plane_subcommands() {
    let error = SessionArgs::parse(&["control-plane".to_string(), "archive".to_string()])
        .err()
        .expect("ASP control-plane adapter must not expose Codex execution operations");

    assert_eq!(
        error,
        "unknown asp agent session control-plane subcommand `archive`"
    );
}
