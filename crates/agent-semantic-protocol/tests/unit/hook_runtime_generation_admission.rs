use super::{hook_event_requires_generation_admission, observe};

#[test]
fn workspace_lifecycle_events_admit_generation() {
    for event in ["session-start", "user-prompt"] {
        assert!(hook_event_requires_generation_admission(
            &["--event".to_owned(), event.to_owned(),],
            false,
            false,
        ));
    }
}

#[test]
fn only_mutating_post_tool_edges_submit_reconciliation() {
    for event in ["pre-tool", "permission-request", "post-tool", "stop"] {
        assert!(!hook_event_requires_generation_admission(
            &["--event".to_owned(), event.to_owned(),],
            false,
            false,
        ));
    }
    assert!(hook_event_requires_generation_admission(
        &["--event".to_owned(), "post-tool".to_owned()],
        true,
        false,
    ));
    assert!(!hook_event_requires_generation_admission(
        &["--event".to_owned(), "pre-tool".to_owned()],
        true,
        false,
    ));
}

#[test]
fn explicit_asp_workspace_is_admitted_before_the_tool_executes() {
    assert!(hook_event_requires_generation_admission(
        &["--event".to_owned(), "pre-tool".to_owned()],
        false,
        true,
    ));
}

#[test]
fn failed_post_tool_reconciliation_is_diagnostic_not_a_hook_error() {
    let observation = observe(
        &["--event".to_owned(), "post-tool".to_owned()],
        true,
        false,
        || Err("provider project-resolution omitted scope".to_owned()),
    );

    assert!(observation.receipt.is_none());
    assert_eq!(
        observation.error.as_deref(),
        Some("provider project-resolution omitted scope")
    );
}

#[test]
fn every_admission_event_contains_failure_instead_of_returning_it() {
    for event in ["session-start", "user-prompt", "post-tool"] {
        let observation = observe(
            &["--event".to_owned(), event.to_owned()],
            event == "post-tool",
            false,
            || Err(format!("{event} generation failed")),
        );

        assert!(observation.receipt.is_none(), "event={event}");
        assert_eq!(
            observation.error.as_deref(),
            Some(format!("{event} generation failed").as_str()),
            "event={event}"
        );
    }
}

#[test]
fn ready_admission_preserves_the_typed_receipt_for_decision_evidence() {
    let receipt = serde_json::json!({
        "schemaId": "agent.semantic-protocols.workspace-generation-admission.v1",
        "schemaVersion": "1",
        "state": "ready",
        "workspaceIdentity": "workspace-test"
    });
    let observation = observe(
        &["--event".to_owned(), "user-prompt".to_owned()],
        false,
        false,
        || Ok(receipt.clone()),
    );

    assert_eq!(observation.receipt, Some(receipt));
    assert!(observation.error.is_none());
}

#[test]
fn lifecycle_admission_submits_once_without_waiting_for_ready() {
    let source = include_str!("../../src/command/hook_runtime_generation_admission.rs");

    assert!(source.contains("admit_runtime_generation().await"));
    assert!(!source.contains("ensure_runtime_generation().await"));
    assert!(!source.contains("WorkspaceGenerationAdmissionState::Ready"));

    let receipt = serde_json::json!({
        "schemaId": "agent.semantic-protocols.workspace-generation-admission.v1",
        "schemaVersion": "1",
        "state": "building",
        "workspaceIdentity": "workspace-test"
    });
    let observation = observe(
        &["--event".to_owned(), "user-prompt".to_owned()],
        false,
        false,
        || Ok(receipt.clone()),
    );
    assert_eq!(observation.receipt, Some(receipt));
    assert!(observation.error.is_none());
}

#[test]
fn non_admission_events_do_not_call_runtime_server() {
    let observation = observe(
        &["--event".to_owned(), "post-tool".to_owned()],
        false,
        false,
        || panic!("non-admission event must not call the Runtime Server"),
    );

    assert!(observation.receipt.is_none());
    assert!(observation.error.is_none());
}
