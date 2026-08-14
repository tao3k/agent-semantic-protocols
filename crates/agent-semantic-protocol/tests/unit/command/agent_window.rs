use super::{AgentWindowRequest, session_control_plane_usage, session_platform_from_ids};
use crate::multi_agent_session::{hook_inbox_reconciliation_receipt, typed_runtime_failure_reason};

#[test]
fn session_command_forms_one_choice_plane_request() {
    let request = AgentWindowRequest::parse(&["--agents".to_owned(), "choice-plane".to_owned()])
        .expect("Agent window request");

    assert!(!request.json);
}

#[test]
fn client_selection_is_not_part_of_the_public_choice_plane() {
    let error = AgentWindowRequest::parse(&[
        "--agents".to_owned(),
        "choice-plane".to_owned(),
        "--client=claude".to_owned(),
    ])
    .expect_err("client selection must come from the host session environment");

    assert!(error.contains("unknown Agent window option --client=claude"));
}

#[test]
fn codex_session_or_thread_identity_selects_codex_without_a_client_flag() {
    assert_eq!(
        session_platform_from_ids(Some("session-1"), None, None, None),
        Ok("codex")
    );
    assert_eq!(
        session_platform_from_ids(None, Some("thread-1"), None, None),
        Ok("codex")
    );
}

#[test]
fn simultaneous_host_identities_fail_closed() {
    let error = session_platform_from_ids(Some("session-1"), None, Some("claude-1"), None)
        .expect_err("simultaneous host identities must be ambiguous");

    assert!(error.starts_with("agent-session-host-ambiguous:"));
}

#[test]
fn agent_or_task_argument_is_rejected_because_org_owns_the_choice_plane() {
    let error = AgentWindowRequest::parse(&[
        "--agents".to_owned(),
        "choice-plane".to_owned(),
        "@asp_explorer".to_owned(),
        "inspect owner drift".to_owned(),
    ])
    .expect_err("agent and task arguments must not be accepted");

    assert!(error.contains("current denied Hook event selects"));
}

#[test]
fn non_registry_address_is_rejected_without_session_fallback() {
    let error = AgentWindowRequest::parse(&["session".to_owned()])
        .expect_err("legacy session surface must not parse as an Agent window");

    assert!(error.contains("current denied Hook event selects"));
    assert_eq!(
        session_control_plane_usage(),
        "usage: asp session --agents choice-plane [--json]"
    );
}

#[test]
fn runtime_failure_preserves_a_typed_server_code_without_guessing_from_message() {
    assert_eq!(
        typed_runtime_failure_reason(
            "runtime-server-data-binding-mismatch: stale owner epoch",
            "runtime-server-session-control-plane-unavailable",
        ),
        "runtime-server-data-binding-mismatch"
    );
    assert_eq!(
        typed_runtime_failure_reason(
            "failed to connect workspace owner endpoint: permission denied",
            "runtime-server-session-control-plane-unavailable",
        ),
        "runtime-server-session-control-plane-unavailable"
    );
}

#[test]
fn hook_inbox_permission_failure_is_typed_degradation_not_choice_plane_blocker() {
    let receipt = hook_inbox_reconciliation_receipt(Some(
        r#"{"schemaId":"agent.semantic-protocols.hook.memory-inbox-failure","reasonKind":"lock-open","detail":"Operation not permitted"}"#,
    ));
    assert_eq!(receipt["authority"], "recovery-log-only");
    assert_eq!(receipt["state"], "degraded");
    assert_eq!(receipt["blocksChoicePlane"], false);
    assert!(
        receipt["failure"]
            .as_str()
            .is_some_and(|failure| failure.contains("Operation not permitted"))
    );
}
