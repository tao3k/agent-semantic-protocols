use std::collections::BTreeMap;

use agent_semantic_hook::{
    DecisionKind, DecisionSubject, HOOK_DECISION_SCHEMA_ID, HOOK_DECISION_SCHEMA_VERSION,
    HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, HookDecision, ReasonKind,
};

use super::{
    HookGenerationAdmissionObservation, hook_event_requires_generation_admission,
    materialize_explicit_query_gate, observe, payload_changed_paths, payload_mutation_id,
};

fn decision_with_fields(fields: BTreeMap<String, serde_json::Value>) -> HookDecision {
    HookDecision {
        schema_id: HOOK_DECISION_SCHEMA_ID,
        schema_version: HOOK_DECISION_SCHEMA_VERSION,
        protocol_id: HOOK_PROTOCOL_ID,
        protocol_version: HOOK_PROTOCOL_VERSION,
        platform: "codex".to_owned(),
        event: "post-tool".to_owned(),
        decision: DecisionKind::Allow,
        reason_kind: ReasonKind::None,
        language_ids: Vec::new(),
        subject: DecisionSubject {
            tool_name: Some("apply_patch".to_owned()),
            command: None,
            paths: Vec::new(),
        },
        routes: Vec::new(),
        message: "workspace mutation".to_owned(),
        fields,
    }
}

#[test]
fn mutation_identity_is_derived_from_the_typed_hook_payload() {
    let payload = serde_json::json!({
        "session_id": "session-root",
        "tool_use_id": "tool-use-1"
    });

    assert_eq!(
        payload_mutation_id(&payload).as_deref(),
        Some("session-root/tool-use-1")
    );
}

#[test]
fn post_tool_paths_come_from_canonical_tool_actions_without_policy_fields() {
    let payload = serde_json::json!({
        "tool_name": "functions.apply_patch",
        "tool_input": {
            "patch": "*** Begin Patch\n*** Update File: crates/agent-semantic-protocol/src/command/hook_runtime.rs\n@@\n-old\n+new\n*** End Patch"
        }
    });

    assert_eq!(
        payload_changed_paths(&payload),
        vec!["crates/agent-semantic-protocol/src/command/hook_runtime.rs".to_owned()]
    );
}

#[test]
fn workspace_lifecycle_events_admit_generation() {
    for event in ["session-start", "user-prompt"] {
        assert!(hook_event_requires_generation_admission(
            event, false, false,
        ));
    }
}

#[test]
fn only_mutating_post_tool_edges_submit_reconciliation() {
    for event in ["pre-tool", "permission-request", "post-tool", "stop"] {
        assert!(!hook_event_requires_generation_admission(
            event, false, false,
        ));
    }
    assert!(hook_event_requires_generation_admission(
        "post-tool",
        true,
        false,
    ));
    assert!(!hook_event_requires_generation_admission(
        "pre-tool", true, false,
    ));
}

#[test]
fn explicit_asp_workspace_is_admitted_before_the_tool_executes() {
    assert!(hook_event_requires_generation_admission(
        "pre-tool", false, true,
    ));
}

#[test]
fn failed_post_tool_reconciliation_is_diagnostic_not_a_hook_error() {
    let observation = observe(
        "post-tool",
        true,
        Some("session-root/tool-use-1".to_owned()),
        vec!["src/lib.rs".to_owned()],
        false,
        |_, _| Err("provider project-resolution omitted scope".to_owned()),
        || panic!("mutation reconciliation must not call ensure"),
    );

    assert!(observation.receipt.is_none());
    assert!(observation.requested);
    assert_eq!(
        observation.error.as_deref(),
        Some("provider project-resolution omitted scope")
    );
}

#[test]
fn every_admission_event_contains_failure_instead_of_returning_it() {
    for event in ["session-start", "user-prompt", "post-tool"] {
        let workspace_mutated = event == "post-tool";
        let observation = observe(
            event,
            workspace_mutated,
            workspace_mutated.then(|| "session-root/tool-use-1".to_owned()),
            workspace_mutated
                .then(|| vec!["src/lib.rs".to_owned()])
                .unwrap_or_default(),
            false,
            |_, _| Err(format!("{event} generation failed")),
            || Err(format!("{event} generation failed")),
        );

        assert!(observation.receipt.is_none(), "event={event}");
        assert!(observation.requested, "event={event}");
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
        "user-prompt",
        false,
        None,
        Vec::new(),
        false,
        |_, _| panic!("lifecycle admission must not submit a mutation delta"),
        || Ok(receipt.clone()),
    );

    assert_eq!(observation.receipt, Some(receipt));
    assert!(observation.requested);
    assert!(observation.error.is_none());
}

#[test]
fn lifecycle_admission_ensures_once_without_submitting_a_mutation_delta() {
    let source = include_str!("../../src/command/hook_runtime_generation_admission.rs");

    assert!(source.contains("ensure_runtime_generation().await"));
    assert!(!source.contains("generation_admission_started"));
    assert!(!source.contains("WorkspaceGenerationAdmissionState::Ready"));

    let receipt = serde_json::json!({
        "schemaId": "agent.semantic-protocols.workspace-generation-admission.v1",
        "schemaVersion": "1",
        "state": "building",
        "workspaceIdentity": "workspace-test"
    });
    let observation = observe(
        "user-prompt",
        false,
        None,
        Vec::new(),
        false,
        |_, _| panic!("lifecycle admission must not submit a mutation delta"),
        || Ok(receipt.clone()),
    );
    assert_eq!(observation.receipt, Some(receipt));
    assert!(observation.requested);
    assert!(observation.error.is_none());
}

#[test]
fn explicit_asp_commands_require_terminal_generation_repair_before_dispatch() {
    let source = include_str!("../../src/command/hook_runtime.rs");
    let admission_source = include_str!("../../src/command/hook_runtime_generation_admission.rs");

    assert!(source.contains("hook_runtime_generation_admission::ensure_ready"));
    assert!(source.contains("explicit_asp_workspace"));
    assert!(source.contains("materialize_explicit_query_gate"));
    assert!(admission_source.contains("ensure_runtime_generation_ready().await"));
    assert!(!admission_source.contains("match session.ensure_runtime_generation().await"));
}

#[test]
fn explicit_asp_generation_failure_blocks_the_tool_call() {
    let mut decision = decision_with_fields(BTreeMap::new());
    materialize_explicit_query_gate(
        &mut decision,
        true,
        HookGenerationAdmissionObservation {
            requested: true,
            receipt: None,
            error: Some("terminal generation failed".to_owned()),
        },
    );

    assert_eq!(decision.decision, DecisionKind::Block);
    assert_eq!(decision.reason_kind, ReasonKind::ActivationUnavailable);
    assert_eq!(
        decision
            .fields
            .get("runtimeGenerationAdmissionStatus")
            .and_then(serde_json::Value::as_str),
        Some("failed-closed")
    );
    assert!(decision.message.contains("terminal generation failed"));
}

#[test]
fn non_admission_events_do_not_call_runtime_server() {
    let observation = observe(
        "post-tool",
        false,
        None,
        Vec::new(),
        false,
        |_, _| panic!("non-admission event must not admit a mutation delta"),
        || panic!("non-admission event must not ensure a generation"),
    );

    assert!(observation.receipt.is_none());
    assert!(!observation.requested);
    assert!(observation.error.is_none());
}

#[test]
fn mutating_post_tool_requires_normalized_changed_paths() {
    let observation = observe(
        "post-tool",
        true,
        Some("session-root/tool-use-1".to_owned()),
        Vec::new(),
        false,
        |_, _| panic!("pathless mutation must fail before Runtime Server admission"),
        || panic!("mutation reconciliation must not call ensure"),
    );

    assert!(observation.receipt.is_none());
    assert!(observation.requested);
    assert_eq!(
        observation.error.as_deref(),
        Some("post-tool workspace mutation omitted normalized changed paths")
    );
}

#[test]
fn mutating_post_tool_requires_typed_mutation_identity() {
    let observation = observe(
        "post-tool",
        true,
        None,
        vec!["src/lib.rs".to_owned()],
        false,
        |_, _| panic!("identity-less mutation must fail before Runtime Server admission"),
        || panic!("mutation reconciliation must not call ensure"),
    );

    assert!(observation.receipt.is_none());
    assert!(observation.requested);
    assert_eq!(
        observation.error.as_deref(),
        Some("post-tool workspace mutation omitted typed mutation identity")
    );
}

#[test]
fn mutating_post_tool_forwards_the_complete_changed_path_set() {
    let receipt = serde_json::json!({
        "schemaId": "agent.semantic-protocols.workspace-generation-mutation-submission-receipt",
        "schemaVersion": "1",
        "mutationId": "session-root/tool-use-1",
        "workspaceIdentity": "workspace-root",
        "changedPathCount": 2,
        "state": "queued"
    });
    let observation = observe(
        "post-tool",
        true,
        Some("session-root/tool-use-1".to_owned()),
        vec![
            "crates/parent/src/lib.rs".to_owned(),
            "languages/rust-lang-project-harness/src/lib.rs".to_owned(),
        ],
        false,
        |mutation_id, paths| {
            assert_eq!(mutation_id, "session-root/tool-use-1");
            assert_eq!(
                paths,
                vec![
                    "crates/parent/src/lib.rs".to_owned(),
                    "languages/rust-lang-project-harness/src/lib.rs".to_owned(),
                ]
            );
            Ok(receipt.clone())
        },
        || panic!("mutation reconciliation must not call ensure"),
    );

    assert_eq!(observation.receipt, Some(receipt));
    assert!(observation.requested);
    assert!(observation.error.is_none());
}
