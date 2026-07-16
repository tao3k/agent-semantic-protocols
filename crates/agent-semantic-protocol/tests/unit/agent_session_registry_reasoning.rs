use super::{
    profile_attestation_identity, profile_attestation_is_valid,
    profile_attested_runtime_observation, typed_subagent_start_proves_canonical_typed_binding,
};
use agent_semantic_client_db::{
    AgentSessionRecord,
    agent_session_registry::{
        AgentSessionLoopState, ResidentChildBootstrapMenuInput, resident_child_bootstrap_menu,
        resident_child_runtime_verified_menu,
    },
};
use agent_semantic_hook::SubagentRuntimeRebindVerifiedObservation;

use crate::command::agent_session_registry::agent_session_registry_bootstrap::reasoning::{
    profile_attestation_evidence_source, profile_attestation_receipt,
    profile_attested_control_result, profile_attested_lifecycle_status,
};

fn observation(reasoning: Option<&str>) -> SubagentRuntimeRebindVerifiedObservation {
    SubagentRuntimeRebindVerifiedObservation {
        root_session_id: "root".into(),
        child_session_id: "child".into(),
        observed_agent_type: "asp_explorer".into(),
        expected_agent_type: "asp_explorer".into(),
        previous_observed_model: None,
        previous_observed_reasoning_effort: None,
        observed_model: "gpt-5.4-mini".into(),
        observed_reasoning_effort: reasoning.map(str::to_string),
        expected_model: "gpt-5.4-mini".into(),
        expected_reasoning_effort: Some("low".into()),
        observation_source: "subagent-start-profile-attestation",
        observation_count: 1,
    }
}

#[test]
fn attestation_applies_only_when_reasoning_is_unobservable() {
    let child = "child".to_string();
    let model = "gpt-5.4-mini".to_string();
    let reasoning = "low".to_string();
    assert!(profile_attestation_is_valid(
        Some(&observation(None)),
        Some(&reasoning),
        Some(&child),
        Some(&model),
        true,
        None,
    ));
    assert!(!profile_attestation_is_valid(
        Some(&observation(Some("high"))),
        Some(&reasoning),
        Some(&child),
        Some(&model),
        true,
        None,
    ));

    let wrong_child = "other-child".to_string();
    let wrong_model = "gpt-5.6".to_string();
    assert!(!profile_attestation_is_valid(
        Some(&observation(None)),
        Some(&reasoning),
        Some(&wrong_child),
        Some(&model),
        true,
        None,
    ));
    assert!(!profile_attestation_is_valid(
        Some(&observation(None)),
        Some(&reasoning),
        Some(&child),
        Some(&wrong_model),
        true,
        None,
    ));
    assert!(!profile_attestation_is_valid(
        Some(&observation(None)),
        Some(&reasoning),
        Some(&child),
        Some(&model),
        false,
        None,
    ));
}

#[test]
fn synthesized_attestation_does_not_invent_reasoning_evidence() {
    let child = "child".to_string();
    let model = "gpt-5.4-mini".to_string();
    let reasoning = "low".to_string();
    let observation = profile_attested_runtime_observation(
        Some("root"),
        Some(&child),
        Some(&model),
        Some(&model),
        Some(&reasoning),
        true,
        Some("subagent-start-profile-attestation"),
    )
    .expect("valid typed profile attestation");
    assert_eq!(observation.observed_reasoning_effort, None);
    assert_eq!(
        observation.expected_reasoning_effort.as_deref(),
        Some("low")
    );
}

fn live_typed_record() -> AgentSessionRecord {
    AgentSessionRecord {
        project_id: "project".into(),
        root_session_id: "root".into(),
        session_id: "child".into(),
        message_target_id: Some("/root/asp_explorer".into()),
        parent_session_id: Some("root".into()),
        name: "asp-explore".into(),
        role: "asp_explorer".into(),
        model: Some("gpt-5.4-mini".into()),
        model_observation_source: Some("codex.subagent-start".into()),
        model_observed_at: Some(1),
        model_evidence_ref: Some("subagent-start:child".into()),
        status: "idle".into(),
        created_at: 1,
        updated_at: 1,
        last_seen_at: Some(1),
        last_heartbeat_at: Some(1),
        expires_at: None,
        archived_at: None,
        last_tool_event: None,
        last_command: None,
        last_evidence_ref: None,
        metadata_json: serde_json::json!({
            "event": "subagent-start",
            "native": true,
            "rootSessionId": "root",
            "childSessionId": "child",
            "agentType": "asp_explorer",
            "messageTargetBinding": {
                "source": "codex.subagent-start",
                "boundRootSessionId": "root",
                "childSessionId": "child",
                "messageTargetId": "/root/asp_explorer"
            }
        })
        .to_string(),
    }
}

#[test]
fn durable_rollout_binding_rehydrates_profile_attestation_without_rollout_file() {
    let mut record = live_typed_record();
    record.model_observation_source = Some("codex.rollout".into());
    record.metadata_json = serde_json::json!({
        "messageTargetBinding": {
            "source": "codex-rollout-session-meta-plus-native-host-tree",
            "boundRootSessionId": "root",
            "childSessionId": "child",
            "messageTargetId": "/root/asp_explorer"
        }
    })
    .to_string();

    assert_eq!(
        profile_attestation_identity(Some(&record), None, Some("root"), true),
        Some(("child".to_string(), "rollout-recovery-profile-attestation"))
    );
}

#[test]
fn trusted_rollout_recovery_can_attest_unobservable_reasoning() {
    let mut recovered = observation(None);
    recovered.observation_source = "rollout-recovery-profile-attestation";
    let reasoning = "low".to_string();
    let child = recovered.child_session_id.clone();
    let model = recovered.observed_model.clone();

    assert!(profile_attestation_is_valid(
        Some(&recovered),
        Some(&reasoning),
        Some(&child),
        Some(&model),
        true,
        None,
    ));
    assert!(recovered.observed_reasoning_effort.is_none());
}

#[test]
fn trusted_profile_attestation_makes_unobservable_reasoning_ready() {
    let record = live_typed_record();
    assert!(typed_subagent_start_proves_canonical_typed_binding(
        &record, "root"
    ));
    let reasoning = "low".to_string();
    let child = record.session_id.clone();
    let model = record.model.clone().expect("model");
    let attested = profile_attestation_is_valid(
        Some(&observation(None)),
        Some(&reasoning),
        Some(&child),
        Some(&model),
        true,
        None,
    );
    assert!(attested);
    let attested_observation = profile_attested_runtime_observation(
        Some("root"),
        Some(&child),
        Some(&model),
        Some(&model),
        Some(&reasoning),
        true,
        Some("subagent-start-profile-attestation"),
    )
    .expect("typed profile attestation observation");
    assert_eq!(attested_observation.observed_reasoning_effort, None);
    assert_eq!(
        attested_observation.expected_reasoning_effort.as_deref(),
        Some("low")
    );
    assert_eq!(
        profile_attested_control_result(attested),
        "typed-profile-attested-runtime-reasoning-unobservable"
    );
    assert_eq!(
        profile_attested_lifecycle_status(attested),
        "resident-child-typed-replacement-profile-attested"
    );
    assert_eq!(
        profile_attestation_evidence_source(attested_observation.observation_source),
        "asp-typed-role-profile-attestation"
    );
    let receipt = profile_attestation_receipt(
        attested,
        &attested_observation,
        (Some(&child), Some(&reasoning)),
    );
    assert_eq!(receipt["typedSpawnIdentityVerified"], true);
    assert_eq!(receipt["rolloutRecoveryIdentityVerified"], false);
    assert_eq!(receipt["attestedChildId"], child);
    assert_eq!(
        receipt["attestationOrigin"],
        "subagent-start-profile-attestation"
    );
    assert_eq!(receipt["expectedReasoningEffort"], "low");
    assert_eq!(receipt["contradictoryReasoningObservation"], false);

    let menu = resident_child_bootstrap_menu(ResidentChildBootstrapMenuInput {
        platform: "codex",
        name: "asp-explore",
        root_session_id: Some("root"),
        record: Some(&record),
        expected_model: Some("gpt-5.4-mini"),
        expected_reasoning_effort: Some("low"),
        rollout_history_status: Some("typed-replacement-observed"),
        rollout_history_action: Some("validate-runtime-before-ready"),
        now: 2,
    });
    let menu = resident_child_runtime_verified_menu(menu, attested);

    assert_eq!(menu.state, AgentSessionLoopState::Ready);
    assert_eq!(menu.choices.len(), 1);
    assert_eq!(menu.choices[0].id, "send-denied-asp-command");
    assert!(menu.choices.iter().all(|choice| {
        !matches!(
            choice.id,
            "resume-existing-child-for-runtime-observation"
                | "resume-existing-child-for-live-target-rebind"
                | "close-stale-resident-child"
                | "create-canonical-typed-child-after-orphaned-owner"
                | "retire-drifted-child-and-create-configured-replacement"
        )
    }));
}
