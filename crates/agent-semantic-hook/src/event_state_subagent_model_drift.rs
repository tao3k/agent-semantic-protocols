//! Host lifecycle observations for resident-child runtime drift.

use std::path::Path;

use agent_semantic_runtime::ensure_project_hook_state_dir;
use serde_json::Value;

use crate::event_state::{HOOK_EVENT_STATE_FILE, read_hook_event_state_tail};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubagentRuntimeRootSessionId(String);

impl SubagentRuntimeRootSessionId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for SubagentRuntimeRootSessionId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for SubagentRuntimeRootSessionId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubagentRuntimeDriftError(String);

impl From<String> for SubagentRuntimeDriftError {
    fn from(value: String) -> Self {
        Self(value)
    }
}

macro_rules! subagent_runtime_text {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub struct $name(String);

        impl $name {
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_owned())
            }
        }
    };
}

subagent_runtime_text!(SubagentRuntimeChildSessionId);
subagent_runtime_text!(SubagentRuntimeAgentType);
subagent_runtime_text!(SubagentRuntimeModelId);
subagent_runtime_text!(SubagentRuntimeReasoningEffort);

/// Latest native subagent start whose runtime model or reasoning drifted from ASP config.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubagentRuntimeDriftObservation {
    root_session_id: SubagentRuntimeRootSessionId,
    child_session_id: SubagentRuntimeChildSessionId,
    observed_agent_type: SubagentRuntimeAgentType,
    expected_agent_type: SubagentRuntimeAgentType,
    observed_model: Option<SubagentRuntimeModelId>,
    observed_reasoning_effort: Option<SubagentRuntimeReasoningEffort>,
    pub consecutive_observation_count: usize,
}

/// Whether a host surface exposed a reasoning-effort value for one child.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReasoningEvidenceVisibility {
    Observed,
    FieldOmitted,
    HostUnsupported,
    TransportFailed,
}

/// Native source that supplied a reasoning-effort fact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReasoningEvidenceSource {
    CodexThreadRuntime,
    SubagentStart,
    RolloutHeader,
    TypedRoleProfile,
}

/// One provenance-preserving reasoning-effort fact for a physical child.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReasoningEvidence {
    pub root_session_id: String,
    pub child_session_id: String,
    pub resident_generation: Option<u64>,
    pub value: Option<String>,
    pub visibility: ReasoningEvidenceVisibility,
    pub source: ReasoningEvidenceSource,
    pub observed_at: Option<i64>,
    pub profile_digest: Option<String>,
}

/// Lifecycle decision produced from all reasoning evidence for one generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReasoningVerdict {
    DirectMatch,
    ProfileAttestedUnobservable,
    DirectMismatch,
    TransientlyUnavailable,
    StaleEvidence,
    ConflictingEvidence,
}

/// Reduced reasoning state consumed by the resident Ready gate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReasoningAssessment {
    pub verdict: ReasoningVerdict,
    pub observed_reasoning_effort: Option<String>,
    pub effective_reasoning_effort: Option<String>,
    pub evidence_source: ReasoningEvidenceSource,
}

/// Reduces evidence without promoting rollout or profile values into direct
/// runtime observations.
pub fn reduce_reasoning_evidence(
    expected: &str,
    evidence: &[ReasoningEvidence],
) -> ReasoningAssessment {
    let direct = evidence.iter().filter(|fact| {
        fact.visibility == ReasoningEvidenceVisibility::Observed
            && matches!(
                fact.source,
                ReasoningEvidenceSource::CodexThreadRuntime
                    | ReasoningEvidenceSource::SubagentStart
            )
    });
    let direct_values = direct
        .filter_map(|fact| fact.value.as_deref())
        .collect::<std::collections::BTreeSet<_>>();
    if direct_values.len() > 1 {
        return ReasoningAssessment {
            verdict: ReasoningVerdict::ConflictingEvidence,
            observed_reasoning_effort: None,
            effective_reasoning_effort: None,
            evidence_source: ReasoningEvidenceSource::CodexThreadRuntime,
        };
    }
    if let Some(observed) = direct_values.first().copied() {
        return ReasoningAssessment {
            verdict: if observed == expected {
                ReasoningVerdict::DirectMatch
            } else {
                ReasoningVerdict::DirectMismatch
            },
            observed_reasoning_effort: Some(observed.to_string()),
            effective_reasoning_effort: Some(observed.to_string()),
            evidence_source: evidence
                .iter()
                .find(|fact| fact.value.as_deref() == Some(observed))
                .map(|fact| fact.source)
                .unwrap_or(ReasoningEvidenceSource::CodexThreadRuntime),
        };
    }
    if evidence
        .iter()
        .any(|fact| fact.visibility == ReasoningEvidenceVisibility::TransportFailed)
    {
        return ReasoningAssessment {
            verdict: ReasoningVerdict::TransientlyUnavailable,
            observed_reasoning_effort: None,
            effective_reasoning_effort: None,
            evidence_source: ReasoningEvidenceSource::CodexThreadRuntime,
        };
    }
    let host_unobservable = evidence.iter().any(|fact| {
        matches!(
            fact.visibility,
            ReasoningEvidenceVisibility::FieldOmitted
                | ReasoningEvidenceVisibility::HostUnsupported
        ) && matches!(
            fact.source,
            ReasoningEvidenceSource::CodexThreadRuntime | ReasoningEvidenceSource::SubagentStart
        )
    });
    let profile_attested = evidence.iter().any(|fact| {
        fact.source == ReasoningEvidenceSource::TypedRoleProfile
            && fact.visibility == ReasoningEvidenceVisibility::Observed
            && fact.value.as_deref() == Some(expected)
            && fact.profile_digest.is_some()
    });
    if host_unobservable && profile_attested {
        return ReasoningAssessment {
            verdict: ReasoningVerdict::ProfileAttestedUnobservable,
            observed_reasoning_effort: None,
            effective_reasoning_effort: Some(expected.to_string()),
            evidence_source: ReasoningEvidenceSource::TypedRoleProfile,
        };
    }
    let rollout = evidence.iter().find(|fact| {
        fact.source == ReasoningEvidenceSource::RolloutHeader
            && fact.visibility == ReasoningEvidenceVisibility::Observed
    });
    ReasoningAssessment {
        verdict: ReasoningVerdict::StaleEvidence,
        observed_reasoning_effort: None,
        effective_reasoning_effort: rollout.and_then(|fact| fact.value.clone()),
        evidence_source: ReasoningEvidenceSource::RolloutHeader,
    }
}

#[cfg(test)]
#[path = "../tests/unit/reasoning_evidence.rs"]
mod reasoning_evidence_tests;

/// Positive receipt proving that a fresh typed child observation matches the
/// runtime required by the preceding replacement state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubagentRuntimeRebindVerifiedObservation {
    pub root_session_id: SubagentRuntimeRootSessionId,
    pub child_session_id: SubagentRuntimeChildSessionId,
    pub observed_agent_type: SubagentRuntimeAgentType,
    pub expected_agent_type: SubagentRuntimeAgentType,
    pub previous_observed_model: Option<SubagentRuntimeModelId>,
    pub previous_observed_reasoning_effort: Option<SubagentRuntimeReasoningEffort>,
    pub observed_model: SubagentRuntimeModelId,
    pub observed_reasoning_effort: Option<SubagentRuntimeReasoningEffort>,
    pub expected_model: SubagentRuntimeModelId,
    pub expected_reasoning_effort: Option<SubagentRuntimeReasoningEffort>,
    /// Host-owned reasoning evidence captured at the same lifecycle boundary.
    /// This intentionally preserves an omitted field instead of coalescing it
    /// with rollout or profile configuration.
    pub reasoning_evidence: Vec<ReasoningEvidence>,
    /// Deterministic verdict over `reasoning_evidence` at capture time.
    pub reasoning_assessment: ReasoningAssessment,
    pub observation_source: &'static str,
    pub observation_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SubagentRuntimeRebindObservation {
    Drift(SubagentRuntimeDriftObservation),
    Verified(SubagentRuntimeRebindVerifiedObservation),
}

/// Backward-compatible name for callers that only inspected model drift.
pub type SubagentModelDriftObservation = SubagentRuntimeDriftObservation;
/// Backward-compatible names for the original profile/unmanaged diagnostics.
pub type SubagentProfileDriftObservation = SubagentRuntimeDriftObservation;
/// Backward-compatible name for the original unmanaged-child diagnostic.
pub type UnmanagedSubagentStartObservation = SubagentRuntimeDriftObservation;

/// Return the newest host-observed subagent runtime drift for one root session.
pub fn latest_subagent_runtime_rebind_observation(
    project_root: &Path,
    root_session_id: &SubagentRuntimeRootSessionId,
) -> Result<Option<SubagentRuntimeRebindObservation>, SubagentRuntimeDriftError> {
    let state_path = ensure_project_hook_state_dir(project_root)?.join(HOOK_EVENT_STATE_FILE);
    if !state_path.is_file() {
        return Ok(None);
    }
    let lines = read_hook_event_state_tail(&state_path)?;
    let mut active: Option<ActiveRuntimeDrift> = None;
    let mut verified: Option<SubagentRuntimeRebindVerifiedObservation> = None;
    for line in &lines {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let event_root_session_id = event
            .pointer("/fields/rootSessionId")
            .or_else(|| event.pointer("/fields/hookObservedRootSessionId"))
            .and_then(Value::as_str);
        if event_root_session_id != Some(root_session_id) {
            continue;
        }
        let observed_child_id = event
            .pointer("/fields/hookObservedChildId")
            .or_else(|| event.pointer("/fields/agentSessionObservedChildId"))
            .or_else(|| event.pointer("/fields/childSessionId"))
            .and_then(Value::as_str);
        let action = event
            .pointer("/fields/agentSessionAction")
            .and_then(Value::as_str);
        match event.get("event").and_then(Value::as_str) {
            Some("subagent-start") => {
                let Some(child_session_id) = observed_child_id else {
                    continue;
                };
                if !is_runtime_drift_action(&event, action) {
                    if let Some(drift) = active.as_ref() {
                        let observed_agent_type =
                            string_at(&event, "/fields/hookObservedAgentType").or_else(|| {
                                string_at(&event, "/fields/agentSessionObservedAgentType")
                            });
                        if observed_agent_type.as_deref()
                            != Some(drift.observation.expected_agent_type.as_str())
                        {
                            continue;
                        }
                        let observed_model = string_at(&event, "/fields/hookObservedModel")
                            .or_else(|| string_at(&event, "/fields/agentSessionObservedModel"));
                        let observed_reasoning_effort =
                            string_at(&event, "/fields/hookObservedReasoningEffort").or_else(
                                || string_at(&event, "/fields/agentSessionObservedReasoningEffort"),
                            );
                        if runtime_matches_expected(
                            observed_model.as_deref(),
                            observed_reasoning_effort.as_deref(),
                            drift.expected_model.as_deref(),
                            drift.expected_reasoning_effort.as_deref(),
                        ) && let (Some(observed_model), Some(expected_model)) =
                            (observed_model, drift.expected_model.clone())
                        {
                            verified = Some(verified_runtime_rebind_observation(
                                drift,
                                &event,
                                child_session_id,
                                observed_model,
                                observed_reasoning_effort,
                                expected_model,
                                "subagent-start",
                            ));
                            active = None;
                        }
                    }
                    continue;
                }
                let Some(observed_agent_type) = event
                    .pointer("/fields/agentSessionObservedAgentType")
                    .and_then(Value::as_str)
                else {
                    continue;
                };
                let Some(expected_agent_type) = event
                    .pointer("/fields/agentSessionExpectedAgentType")
                    .and_then(Value::as_str)
                else {
                    continue;
                };
                let count = active
                    .as_ref()
                    .filter(|drift| drift.observation.child_session_id == child_session_id)
                    .map_or(1, |drift| {
                        drift.observation.consecutive_observation_count + 1
                    });
                verified = None;
                active = Some(ActiveRuntimeDrift {
                    observation: SubagentRuntimeDriftObservation {
                        root_session_id: root_session_id.to_string(),
                        child_session_id: child_session_id.to_string(),
                        observed_agent_type: observed_agent_type.to_string(),
                        expected_agent_type: expected_agent_type.to_string(),
                        observed_model: string_at(&event, "/fields/agentSessionObservedModel"),
                        observed_reasoning_effort: string_at(
                            &event,
                            "/fields/agentSessionObservedReasoningEffort",
                        ),
                        consecutive_observation_count: count,
                    },
                    expected_model: string_at(&event, "/fields/agentSessionExpectedModel")
                        .or_else(|| string_at(&event, "/fields/expectedModel")),
                    expected_reasoning_effort: string_at(
                        &event,
                        "/fields/agentSessionExpectedReasoningEffort",
                    )
                    .or_else(|| string_at(&event, "/fields/expectedReasoningEffort")),
                    awaiting_start_stop_pair: true,
                });
            }
            Some("subagent-stop") => {
                let Some(child_session_id) = observed_child_id else {
                    continue;
                };
                let Some(drift) = active
                    .as_mut()
                    .filter(|drift| drift.observation.child_session_id == child_session_id)
                else {
                    continue;
                };
                if action == Some("subagent-stop-archived-managed-child") {
                    active = None;
                    verified = None;
                    continue;
                }
                if drift.awaiting_start_stop_pair {
                    // Codex emits one Stop for the same turn as SubagentStart. It
                    // closes that observation but is not a second repair attempt.
                    drift.awaiting_start_stop_pair = false;
                    continue;
                }
                let observed_agent_type = string_at(&event, "/fields/hookObservedAgentType")
                    .or_else(|| string_at(&event, "/fields/agentSessionObservedAgentType"));
                let observed_model = string_at(&event, "/fields/hookObservedModel")
                    .or_else(|| string_at(&event, "/fields/agentSessionObservedModel"));
                let observed_reasoning_effort =
                    string_at(&event, "/fields/hookObservedReasoningEffort").or_else(|| {
                        string_at(&event, "/fields/agentSessionObservedReasoningEffort")
                    });
                if observed_model.is_none() && observed_reasoning_effort.is_none() {
                    continue;
                }
                if observed_agent_type.as_deref()
                    == Some(drift.observation.expected_agent_type.as_str())
                    && runtime_matches_expected(
                        observed_model.as_deref(),
                        observed_reasoning_effort.as_deref(),
                        drift.expected_model.as_deref(),
                        drift.expected_reasoning_effort.as_deref(),
                    )
                    && let (Some(observed_model), Some(expected_model)) =
                        (observed_model.clone(), drift.expected_model.clone())
                {
                    verified = Some(verified_runtime_rebind_observation(
                        drift,
                        &event,
                        child_session_id,
                        observed_model,
                        observed_reasoning_effort,
                        expected_model,
                        "subagent-stop-resume",
                    ));
                    active = None;
                    continue;
                }
                if let Some(observed_model) = observed_model {
                    drift.observation.observed_model = Some(observed_model);
                }
                if let Some(observed_reasoning_effort) = observed_reasoning_effort {
                    drift.observation.observed_reasoning_effort = Some(observed_reasoning_effort);
                }
                drift.observation.consecutive_observation_count += 1;
                verified = None;
            }
            _ => {}
        }
    }
    Ok(active
        .map(|drift| SubagentRuntimeRebindObservation::Drift(drift.observation))
        .or_else(|| verified.map(SubagentRuntimeRebindObservation::Verified)))
}

fn verified_runtime_rebind_observation(
    drift: &ActiveRuntimeDrift,
    event: &Value,
    child_session_id: &str,
    observed_model: String,
    observed_reasoning_effort: Option<String>,
    expected_model: String,
    observation_source: &'static str,
) -> SubagentRuntimeRebindVerifiedObservation {
    let reasoning_evidence = vec![ReasoningEvidence {
        root_session_id: drift.observation.root_session_id.clone(),
        child_session_id: child_session_id.to_string(),
        resident_generation: None,
        value: observed_reasoning_effort.clone(),
        source: if observation_source == "codex.subagent-start" {
            ReasoningEvidenceSource::SubagentStart
        } else {
            ReasoningEvidenceSource::CodexThreadRuntime
        },
        visibility: if observed_reasoning_effort.is_some() {
            ReasoningEvidenceVisibility::Observed
        } else {
            ReasoningEvidenceVisibility::FieldOmitted
        },
        observed_at: None,
        profile_digest: None,
    }];
    let reasoning_assessment = reduce_reasoning_evidence(
        drift
            .expected_reasoning_effort
            .as_deref()
            .unwrap_or_default(),
        &reasoning_evidence,
    );

    SubagentRuntimeRebindVerifiedObservation {
        root_session_id: drift.observation.root_session_id.clone(),
        child_session_id: child_session_id.to_string(),
        observed_agent_type: string_at(event, "/fields/hookObservedAgentType")
            .or_else(|| string_at(event, "/fields/agentSessionObservedAgentType"))
            .unwrap_or_else(|| drift.observation.observed_agent_type.clone()),
        expected_agent_type: drift.observation.expected_agent_type.clone(),
        previous_observed_model: drift.observation.observed_model.clone(),
        previous_observed_reasoning_effort: drift.observation.observed_reasoning_effort.clone(),
        observed_model,
        observed_reasoning_effort,
        expected_model,
        expected_reasoning_effort: drift.expected_reasoning_effort.clone(),
        reasoning_evidence,
        reasoning_assessment,
        observation_source,
        observation_count: drift.observation.consecutive_observation_count + 1,
    }
}

/// Return only the active drift branch for compatibility with existing callers.
pub fn latest_subagent_runtime_drift(
    project_root: &Path,
    root_session_id: &SubagentRuntimeRootSessionId,
) -> Result<Option<SubagentRuntimeDriftObservation>, SubagentRuntimeDriftError> {
    Ok(
        match latest_subagent_runtime_rebind_observation(project_root, root_session_id)? {
            Some(SubagentRuntimeRebindObservation::Drift(observation)) => Some(observation),
            _ => None,
        },
    )
}

pub fn latest_subagent_runtime_rebind_verified(
    project_root: &Path,
    root_session_id: &SubagentRuntimeRootSessionId,
) -> Result<Option<SubagentRuntimeRebindVerifiedObservation>, SubagentRuntimeDriftError> {
    Ok(
        match latest_subagent_runtime_rebind_observation(project_root, root_session_id)? {
            Some(SubagentRuntimeRebindObservation::Verified(observation)) => Some(observation),
            _ => None,
        },
    )
}

struct ActiveRuntimeDrift {
    observation: SubagentRuntimeDriftObservation,
    expected_model: Option<String>,
    expected_reasoning_effort: Option<String>,
    awaiting_start_stop_pair: bool,
}

fn string_at(event: &Value, pointer: &str) -> Option<String> {
    event
        .pointer(pointer)
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn runtime_matches_expected(
    observed_model: Option<&str>,
    observed_reasoning_effort: Option<&str>,
    expected_model: Option<&str>,
    expected_reasoning_effort: Option<&str>,
) -> bool {
    expected_model.is_some_and(|expected| observed_model == Some(expected))
        && expected_reasoning_effort
            .is_none_or(|expected| observed_reasoning_effort == Some(expected))
}

fn is_runtime_drift_action(event: &Value, action: Option<&str>) -> bool {
    action == Some("replace-drifted-native-subagent")
        || action == Some("repair-native-subagent-runtime")
        || action == Some("repair-native-subagent-model")
        || action == Some("repair-native-subagent-profile")
        || (action == Some("ignore-unmanaged-native-subagent")
            && event
                .pointer("/fields/bootstrapBlocked")
                .and_then(Value::as_str)
                == Some("host-created-unmanaged-agent"))
}

pub fn latest_subagent_model_drift(
    project_root: &Path,
    root_session_id: &SubagentRuntimeRootSessionId,
) -> Result<Option<SubagentModelDriftObservation>, SubagentRuntimeDriftError> {
    latest_subagent_runtime_drift(project_root, root_session_id)
}

pub fn latest_subagent_profile_drift(
    project_root: &Path,
    root_session_id: &SubagentRuntimeRootSessionId,
) -> Result<Option<SubagentProfileDriftObservation>, SubagentRuntimeDriftError> {
    latest_subagent_runtime_drift(project_root, root_session_id)
}

/// Return the newest unmanaged native subagent start for one root session.
pub fn latest_unmanaged_subagent_start(
    project_root: &Path,
    root_session_id: &SubagentRuntimeRootSessionId,
) -> Result<Option<UnmanagedSubagentStartObservation>, SubagentRuntimeDriftError> {
    latest_subagent_runtime_drift(project_root, root_session_id)
}
