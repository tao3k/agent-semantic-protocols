use agent_semantic_client_db::agent_session_registry::AgentSessionRecord;
use agent_semantic_hook::{
    SubagentRuntimeDriftObservation, SubagentRuntimeRebindVerifiedObservation,
};

pub(super) fn rollout_proves_canonical_typed_binding(
    child_session_id: &str,
    root_session_id: &str,
    expected_agent_type: &str,
    canonical_target: &str,
) -> bool {
    agent_semantic_runtime::codex_rollout_session_metadata(child_session_id).is_ok_and(|metadata| {
        metadata.is_some_and(|metadata| {
            metadata.session_id == child_session_id
                && metadata.root_session_id.as_deref() == Some(root_session_id)
                && metadata.parent_thread_id.as_deref() == Some(root_session_id)
                && metadata.agent_role.as_deref() == Some(expected_agent_type)
                && metadata.agent_path.as_deref() == Some(canonical_target)
        })
    })
}

pub(super) fn profile_attestation_identity(
    record: Option<&AgentSessionRecord>,
    subagent_start_child_id: Option<&String>,
    root_session_id: Option<&str>,
    expected_agent_type: &str,
    canonical_target: &str,
    target_present: bool,
) -> Option<(String, &'static str)> {
    if let Some(child_id) = subagent_start_child_id {
        return Some((child_id.clone(), "subagent-start-profile-attestation"));
    }
    let record = record?;
    let root_session_id = root_session_id?;
    (target_present
        && record.message_target_id.as_deref() == Some(canonical_target)
        && agent_semantic_client_db::agent_session_registry::agent_session_message_target_is_live_bound(
            record,
            root_session_id,
        )
        && (stored_rollout_recovery_binding_is_valid(record)
            || rollout_proves_canonical_typed_binding(
                &record.session_id,
                root_session_id,
                expected_agent_type,
                canonical_target,
            )))
    .then(|| {
        (
            record.session_id.clone(),
            "rollout-recovery-profile-attestation",
        )
    })
}

fn stored_rollout_recovery_binding_is_valid(record: &AgentSessionRecord) -> bool {
    // The caller has already passed the registry's live-binding validator,
    // which checks the trusted source allowlist and exact root/child/target
    // metadata. Do not couple recovery attestation to one internal source name.
    record.model_observation_source.as_deref() == Some("codex.rollout")
}

pub(super) fn typed_subagent_start_proves_canonical_typed_binding(
    existing: &AgentSessionRecord,
    root_session_id: &str,
    expected_agent_type: &str,
) -> bool {
    if existing.configured_agent_type.as_deref() != Some(expected_agent_type)
        || existing.root_session_id != root_session_id
        || existing.parent_session_id.as_deref() != Some(root_session_id)
    {
        return false;
    }
    existing
        .profile_evidence_json
        .as_deref()
        .is_some_and(|evidence| {
            serde_json::from_str::<serde_json::Value>(evidence).is_ok_and(|metadata| {
                metadata.get("event").and_then(serde_json::Value::as_str) == Some("subagent-start")
                    && metadata.get("native").and_then(serde_json::Value::as_bool) == Some(true)
                    && metadata
                        .get("rootSessionId")
                        .and_then(serde_json::Value::as_str)
                        == Some(root_session_id)
                    && metadata
                        .get("childSessionId")
                        .and_then(serde_json::Value::as_str)
                        == Some(existing.session_id.as_str())
                    && metadata
                        .get("agentType")
                        .and_then(serde_json::Value::as_str)
                        == Some(expected_agent_type)
            })
        })
}

pub(super) fn typed_subagent_start_binding_is_valid(
    record: &AgentSessionRecord,
    root_session_id: &str,
    expected_agent_type: &str,
    live_bound: bool,
) -> bool {
    live_bound
        && typed_subagent_start_proves_canonical_typed_binding(
            record,
            root_session_id,
            expected_agent_type,
        )
}

pub(super) fn profile_attested_control_result(attested: bool) -> &'static str {
    if attested {
        "typed-profile-attested-runtime-reasoning-unobservable"
    } else {
        "observed-runtime-matches-expected"
    }
}

pub(super) fn profile_attested_lifecycle_status(attested: bool) -> &'static str {
    if attested {
        "resident-child-typed-replacement-profile-attested"
    } else {
        "resident-child-typed-replacement-verified"
    }
}

pub(super) fn profile_attestation_evidence_source(observation_source: &str) -> &'static str {
    if observation_source == "rollout-recovery-profile-attestation" {
        "asp-rollout-recovery-profile-attestation"
    } else {
        "asp-typed-role-profile-attestation"
    }
}

pub(super) fn profile_attestation_receipt(
    profile_attested: bool,
    observation: &SubagentRuntimeRebindVerifiedObservation,
    managed_agent_kind: &str,
    evidence: (Option<&String>, Option<&String>),
) -> serde_json::Value {
    if !profile_attested {
        return serde_json::Value::Null;
    }
    let (child_id, expected_reasoning) = evidence;
    serde_json::json!({
        "managedAgentKind": managed_agent_kind,
        "typedSpawnIdentityVerified": observation.observation_source == "subagent-start-profile-attestation",
        "rolloutRecoveryIdentityVerified": observation.observation_source == "rollout-recovery-profile-attestation",
        "attestedChildId": child_id,
        "attestationOrigin": observation.observation_source,
        "observedModelMatchesProfile": true,
        "expectedReasoningEffort": expected_reasoning,
        "observedReasoningEffort": serde_json::Value::Null,
        "effectiveReasoningEffort": expected_reasoning,
        "reasoningVisibility": "field-omitted",
        "reasoningVerdict": "profile-attested-unobservable",
        "reasoningAssurance": "config-attested",
        "contradictoryReasoningObservation": false,
        "policy": "accept-host-enforced-profile-when-runtime-reasoning-is-unobservable"
    })
}

pub(super) fn profile_attested_runtime_observation(
    root_session_id: Option<&str>,
    child_id: Option<&String>,
    expected_agent_type: &str,
    observed_model: Option<&String>,
    expected_model: Option<&String>,
    expected_reasoning: Option<&String>,
    target_present: bool,
    observation_source: Option<&'static str>,
) -> Option<SubagentRuntimeRebindVerifiedObservation> {
    let root_session_id = root_session_id?;
    let child_id = child_id?;
    let observed_model = observed_model?;
    let expected_model = expected_model?;
    let expected_reasoning = expected_reasoning?;
    let observation_source = observation_source?;
    (observed_model == expected_model && target_present).then(|| {
        let profile_digest = format!("{expected_agent_type}|{expected_model}|{expected_reasoning}");
        let reasoning_evidence = vec![
            agent_semantic_hook::ReasoningEvidence {
                root_session_id: root_session_id.to_string(),
                child_session_id: child_id.clone(),
                resident_generation: None,
                value: None,
                source: agent_semantic_hook::ReasoningEvidenceSource::SubagentStart,
                visibility: agent_semantic_hook::ReasoningEvidenceVisibility::FieldOmitted,
                observed_at: None,
                profile_digest: None,
            },
            agent_semantic_hook::ReasoningEvidence {
                root_session_id: root_session_id.to_string(),
                child_session_id: child_id.clone(),
                resident_generation: None,
                value: Some(expected_reasoning.clone()),
                source: agent_semantic_hook::ReasoningEvidenceSource::TypedRoleProfile,
                visibility: agent_semantic_hook::ReasoningEvidenceVisibility::Observed,
                observed_at: None,
                profile_digest: Some(profile_digest),
            },
        ];
        let reasoning_assessment = agent_semantic_hook::reduce_reasoning_evidence(
            expected_reasoning.as_str(),
            &reasoning_evidence,
        );
        SubagentRuntimeRebindVerifiedObservation {
            root_session_id: root_session_id.to_string(),
            child_session_id: child_id.clone(),
            observed_agent_type: expected_agent_type.to_string(),
            expected_agent_type: expected_agent_type.to_string(),
            previous_observed_model: None,
            previous_observed_reasoning_effort: None,
            observed_model: observed_model.clone(),
            observed_reasoning_effort: None,
            expected_model: expected_model.clone(),
            expected_reasoning_effort: Some(expected_reasoning.clone()),
            reasoning_evidence,
            reasoning_assessment,
            observation_source,
            observation_count: 1,
        }
    })
}

pub(super) fn profile_attestation_is_valid(
    observation: Option<&SubagentRuntimeRebindVerifiedObservation>,
    expected_agent_type: &str,
    expected_reasoning: Option<&String>,
    child_id: Option<&String>,
    expected_model: Option<&String>,
    target_present: bool,
    runtime_drift: Option<&SubagentRuntimeDriftObservation>,
) -> bool {
    profile_attestation_reasoning_assessment(
        observation,
        expected_agent_type,
        expected_reasoning,
        child_id,
        expected_model,
        target_present,
        runtime_drift,
    )
    .is_some_and(|assessment| {
        assessment.verdict == agent_semantic_hook::ReasoningVerdict::ProfileAttestedUnobservable
    })
}

fn profile_attestation_reasoning_assessment(
    observation: Option<&SubagentRuntimeRebindVerifiedObservation>,
    expected_agent_type: &str,
    expected_reasoning: Option<&String>,
    child_id: Option<&String>,
    expected_model: Option<&String>,
    target_present: bool,
    runtime_drift: Option<&SubagentRuntimeDriftObservation>,
) -> Option<agent_semantic_hook::ReasoningAssessment> {
    let observation = observation?;
    let expected_reasoning = expected_reasoning?;
    let child_id = child_id?;
    let expected_model = expected_model?;
    let identity_attested = observation.observed_reasoning_effort.is_none()
        && matches!(
            observation.observation_source,
            "subagent-start"
                | "subagent-start-profile-attestation"
                | "rollout-recovery-profile-attestation"
        )
        && child_id == &observation.child_session_id
        && observation.observed_agent_type == expected_agent_type
        && observation.expected_agent_type == expected_agent_type
        && observation.observed_model == *expected_model
        && target_present
        && runtime_drift.is_none_or(|drift| drift.child_session_id != observation.child_session_id);
    if !identity_attested {
        return None;
    }

    let profile_digest = format!("{expected_agent_type}|{expected_model}|{expected_reasoning}");
    let evidence = [
        agent_semantic_hook::ReasoningEvidence {
            root_session_id: observation.root_session_id.clone(),
            child_session_id: observation.child_session_id.clone(),
            resident_generation: None,
            value: None,
            visibility: agent_semantic_hook::ReasoningEvidenceVisibility::FieldOmitted,
            source: agent_semantic_hook::ReasoningEvidenceSource::SubagentStart,
            observed_at: None,
            profile_digest: None,
        },
        agent_semantic_hook::ReasoningEvidence {
            root_session_id: observation.root_session_id.clone(),
            child_session_id: observation.child_session_id.clone(),
            resident_generation: None,
            value: Some(expected_reasoning.clone()),
            visibility: agent_semantic_hook::ReasoningEvidenceVisibility::Observed,
            source: agent_semantic_hook::ReasoningEvidenceSource::TypedRoleProfile,
            observed_at: None,
            profile_digest: Some(profile_digest),
        },
    ];
    Some(agent_semantic_hook::reduce_reasoning_evidence(
        expected_reasoning,
        &evidence,
    ))
}

pub(super) fn insert_runtime_evidence_incomplete_receipt(
    rendered: &mut serde_json::Value,
    observation: &SubagentRuntimeRebindVerifiedObservation,
    target: &str,
    managed_agent_kind: &str,
    expected_model: Option<&str>,
    expected_reasoning: Option<&str>,
    runtime_reasoning_from_host: bool,
    registry_routable: bool,
) {
    let Some(object) = rendered.as_object_mut() else {
        return;
    };
    object.insert(
        "hostControlDirective".to_string(),
        serde_json::json!({
            "schemaId": "agent.semantic-protocols.agent-session-host-control-directive",
            "schemaVersion": "1",
            "intent": "report-host-runtime-reasoning-evidence-unavailable",
            "target": target,
            "childSessionId": observation.child_session_id,
            "managedAgentKind": managed_agent_kind,
            "identityPolicy": "preserve-existing-typed-child",
            "createPolicy": "forbidden-runtime-evidence-incomplete",
            "mainAgentAction": serde_json::Value::Null,
            "bootstrapBlocked": "host-runtime-reasoning-evidence-unavailable",
            "verification": {
                "source": "fresh-codex-subagent-start-lifecycle-observation",
                "requiredMatches": ["managedAgentKind", "model", "reasoningEffort"]
            }
        }),
    );
    object.insert(
        "hostLifecycleObservation".to_string(),
        serde_json::json!({
            "status": "typed-replacement-host-runtime-evidence-unavailable",
            "rootSessionId": observation.root_session_id,
            "childSessionId": observation.child_session_id,
            "typedReplacementVerified": false,
            "verificationSource": observation.observation_source,
            "observedAgentType": observation.observed_agent_type,
            "expectedAgentType": managed_agent_kind,
            "observedModel": observation.observed_model,
            "observedReasoningEffort": observation.observed_reasoning_effort,
            "expectedModel": expected_model,
            "expectedReasoningEffort": expected_reasoning,
            "modelMatchesExpected": expected_model.is_some_and(|expected| observation.observed_model == expected),
            "reasoningMatchesExpected": expected_reasoning.is_none_or(|expected| observation.observed_reasoning_effort.as_deref() == Some(expected)),
            "reasoningEvidenceSource": if runtime_reasoning_from_host {
                serde_json::Value::String("codex-app-server-thread-resume".to_string())
            } else if observation.observed_reasoning_effort.is_some() {
                serde_json::Value::String(observation.observation_source.to_string())
            } else {
                serde_json::Value::Null
            },
            "registryRoutable": registry_routable,
            "nextAction": "report-host-runtime-reasoning-evidence-unavailable",
            "replacementAllowed": false,
            "cleanupAllowed": false
        }),
    );
}

#[cfg(test)]
#[path = "../../tests/unit/agent_session_registry_reasoning.rs"]
mod tests;
pub(super) fn direct_reasoning_receipt(
    root_session_id: &str,
    child_session_id: &str,
    reasoning_effort: &str,
) -> (
    Vec<agent_semantic_hook::ReasoningEvidence>,
    agent_semantic_hook::ReasoningAssessment,
) {
    let source = agent_semantic_hook::ReasoningEvidenceSource::CodexThreadRuntime;
    let effort = Some(reasoning_effort.to_string());
    (
        vec![agent_semantic_hook::ReasoningEvidence {
            root_session_id: root_session_id.to_string(),
            child_session_id: child_session_id.to_string(),
            resident_generation: None,
            value: effort.clone(),
            visibility: agent_semantic_hook::ReasoningEvidenceVisibility::Observed,
            source,
            observed_at: None,
            profile_digest: None,
        }],
        agent_semantic_hook::ReasoningAssessment {
            verdict: agent_semantic_hook::ReasoningVerdict::DirectMatch,
            observed_reasoning_effort: effort.clone(),
            effective_reasoning_effort: effort,
            evidence_source: source,
        },
    )
}
