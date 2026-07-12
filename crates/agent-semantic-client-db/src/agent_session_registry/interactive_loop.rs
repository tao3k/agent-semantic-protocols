use agent_semantic_loop::{Choice, HostRequirement, LoopReceipt, TraceStep};
use serde::Serialize;

use super::types::AgentSessionRecord;

#[derive(Serialize)]
pub struct AgentSessionInteractiveMenu<'a> {
    #[serde(rename = "schemaId")]
    pub schema_id: &'a str,
    #[serde(rename = "schemaVersion")]
    pub schema_version: &'a str,
    pub owner: &'a str,
    pub state: AgentSessionLoopState,
    pub name: &'a str,
    #[serde(rename = "rootSessionId", skip_serializing_if = "Option::is_none")]
    pub root_session_id: Option<&'a str>,
    #[serde(rename = "expectedModel", skip_serializing_if = "Option::is_none")]
    pub expected_model: Option<&'a str>,
    #[serde(
        rename = "expectedReasoningEffort",
        skip_serializing_if = "Option::is_none"
    )]
    pub expected_reasoning_effort: Option<&'a str>,
    #[serde(
        rename = "rolloutHistoryStatus",
        skip_serializing_if = "Option::is_none"
    )]
    pub rollout_history_status: Option<&'a str>,
    #[serde(
        rename = "rolloutHistoryAction",
        skip_serializing_if = "Option::is_none"
    )]
    pub rollout_history_action: Option<&'a str>,
    #[serde(rename = "session", skip_serializing_if = "Option::is_none")]
    pub session: Option<AgentSessionInteractiveSession<'a>>,
    #[serde(rename = "hostRequirement")]
    pub host_requirement: AgentSessionHostRequirement<'a>,
    pub trace: Vec<AgentSessionLoopTraceStep<'a>>,
    pub choices: Vec<AgentSessionInteractiveChoice<'a>>,
    pub receipt: AgentSessionInteractiveReceipt<'a>,
}

#[derive(Serialize)]
pub struct AgentSessionInteractiveSession<'a> {
    #[serde(rename = "childSessionId")]
    pub child_session_id: &'a str,
    pub status: &'a str,
    pub role: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<&'a str>,
    #[serde(
        rename = "modelObservationSource",
        skip_serializing_if = "Option::is_none"
    )]
    pub model_observation_source: Option<&'a str>,
    #[serde(rename = "modelObservedAt", skip_serializing_if = "Option::is_none")]
    pub model_observed_at: Option<i64>,
    #[serde(rename = "modelEvidenceRef", skip_serializing_if = "Option::is_none")]
    pub model_evidence_ref: Option<&'a str>,
    #[serde(rename = "messageTargetStatus")]
    pub message_target_status: &'a str,
    #[serde(rename = "messageTargetId", skip_serializing_if = "Option::is_none")]
    pub message_target_id: Option<&'a str>,
}

pub type AgentSessionInteractiveChoice<'a> = Choice<'a, AgentSessionLoopState>;

pub type AgentSessionHostRequirement<'a> = HostRequirement<'a>;

pub type AgentSessionLoopTraceStep<'a> = TraceStep<'a, AgentSessionLoopState>;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum AgentSessionLoopState {
    Audit,
    Classify,
    Recover,
    Adopt,
    Cleanup,
    Create,
    Register,
    Validate,
    Ready,
    SendDeniedCommand,
    WaitReceipt,
    RetryOriginal,
}

pub type AgentSessionInteractiveReceipt<'a> = LoopReceipt<'a>;

pub struct ResidentChildBootstrapMenuInput<'a> {
    pub platform: &'a str,
    pub name: &'a str,
    pub root_session_id: Option<&'a str>,
    pub record: Option<&'a AgentSessionRecord>,
    pub expected_model: Option<&'a str>,
    pub expected_reasoning_effort: Option<&'a str>,
    pub rollout_history_status: Option<&'a str>,
    pub rollout_history_action: Option<&'a str>,
    pub now: i64,
}

pub fn resident_child_bootstrap_menu<'a>(
    input: ResidentChildBootstrapMenuInput<'a>,
) -> AgentSessionInteractiveMenu<'a> {
    let (state, choices, trace) = resident_child_state_and_choices(
        input.record,
        input.expected_model,
        input.rollout_history_status,
        input.now,
    );
    AgentSessionInteractiveMenu {
        schema_id: "agent.semantic-protocols.agent-session-interactive-menu",
        schema_version: "1",
        owner: "rust",
        state,
        name: input.name,
        root_session_id: input.root_session_id,
        expected_model: input.expected_model,
        expected_reasoning_effort: input.expected_reasoning_effort,
        rollout_history_status: input.rollout_history_status,
        rollout_history_action: input.rollout_history_action,
        session: input.record.map(interactive_session_record),
        host_requirement: AgentSessionHostRequirement {
            platform: input.platform,
            resident_child_name: input.name,
            managed_agent_kind: "asp_explorer",
            required_transport: "message-agent",
            required_outputs: &["childSessionId", "agentMessageTargetId"],
            blocked_when: &[
                "native-built-in-agent-type-only",
                "normal-thread-id-only",
                "agent-message-target-missing",
            ],
        },
        trace,
        choices,
        receipt: AgentSessionInteractiveReceipt {
            loop_name: "asp-resident-child-bootstrap",
            invariant: "choose one menu action, perform the platform-native action, then re-enter this loop until state=ready",
            no_next_command: true,
        },
    }
}

fn resident_child_state_and_choices<'a>(
    record: Option<&AgentSessionRecord>,
    expected_model: Option<&str>,
    rollout_history_status: Option<&str>,
    now: i64,
) -> (
    AgentSessionLoopState,
    Vec<AgentSessionInteractiveChoice<'a>>,
    Vec<AgentSessionLoopTraceStep<'a>>,
) {
    let Some(record) = record else {
        let preflight_checked = rollout_history_status == Some("checked-no-reusable-rollout");
        if !preflight_checked {
            return (
                AgentSessionLoopState::Audit,
                vec![AgentSessionInteractiveChoice {
                    id: "audit-resident-candidates",
                    label: "Audit current resident-child candidates before creating anything.",
                    platform_action: "Run the loop-owned lifecycle audit for this root session and classify registered, rollout-only, stale, duplicate, and orphan-risk ASP children.",
                    next_state: AgentSessionLoopState::Classify,
                    required_inputs: &["rootSessionId"],
                }],
                vec![AgentSessionLoopTraceStep {
                    state: AgentSessionLoopState::Audit,
                    result: "resident-preflight-required",
                }],
            );
        }
        return (
            AgentSessionLoopState::Create,
            vec![
                AgentSessionInteractiveChoice {
                    id: "create-managed-resident-child",
                    label: "Create the configured ASP resident child.",
                    platform_action: "Use the detected platform-native managed-agent creation surface, not a shell command. Create the configured managed resident child once; do not create generic fallback agents or normal threads. Immediately re-enter this pane after the native create call returns; do not wait for SubagentStart as a child message. The pane observes host registration. If it still reports Create for this root, choose report-host-managed-agent-lifecycle-unavailable instead of creating a duplicate. Do not copy child ids, message targets, or model claims into this pane.",
                    next_state: AgentSessionLoopState::Audit,
                    required_inputs: &[],
                },
                AgentSessionInteractiveChoice {
                    id: "report-host-managed-agent-lifecycle-unavailable",
                    label: "Report that the host cannot start the managed ASP lifecycle.",
                    platform_action: "If the host cannot create the configured managed agent type, or native creation emits no SubagentStart event, report bootstrapBlocked=host-managed-agent-lifecycle-unavailable. Do not create or register a generic replacement.",
                    next_state: AgentSessionLoopState::Create,
                    required_inputs: &["hostLifecycleGapObserved"],
                },
            ],
            vec![
                AgentSessionLoopTraceStep {
                    state: AgentSessionLoopState::Audit,
                    result: "checked-no-reusable-rollout",
                },
                AgentSessionLoopTraceStep {
                    state: AgentSessionLoopState::Classify,
                    result: "no-resident-child",
                },
            ],
        );
    };
    if record.message_target_id().is_none() {
        return (
            AgentSessionLoopState::Recover,
            vec![
                AgentSessionInteractiveChoice {
                    id: "resume-managed-child-for-native-start",
                    label: "Resume the configured resident child through the native managed profile.",
                    platform_action: "Use the host-native resume action once for this existing managed child, then immediately re-enter this pane. Do not wait for SubagentStart as a child message. The pane observes whether the host event refreshed child identity, message target, type, and model atomically. Do not verify or register a target through child text or command flags.",
                    next_state: AgentSessionLoopState::Audit,
                    required_inputs: &[],
                },
                AgentSessionInteractiveChoice {
                    id: "cleanup-unrecoverable-child",
                    label: "Close the child if its native message target cannot be verified.",
                    platform_action: "Use the host-native close/archive action for the existing ASP-managed child. A close request whose previous status is running is not completion: wait for terminal host status or the SubagentStop receipt before re-entering this loop. Do not delete registry state manually.",
                    next_state: AgentSessionLoopState::Cleanup,
                    required_inputs: &["nativeStopReceipt"],
                },
            ],
            vec![
                AgentSessionLoopTraceStep {
                    state: AgentSessionLoopState::Classify,
                    result: "registered-child-needs-recovery",
                },
                AgentSessionLoopTraceStep {
                    state: AgentSessionLoopState::Recover,
                    result: "native-message-target-unverified",
                },
            ],
        );
    }
    if matches!(
        record.status.as_str(),
        "archived" | "closed" | "deleted" | "expired" | "invalid" | "missing" | "orphan-risk"
    ) || !record.is_routable_at(now)
    {
        return (
            AgentSessionLoopState::Cleanup,
            vec![
                AgentSessionInteractiveChoice {
                    id: "close-stale-resident-child",
                    label: "Close or archive the stale ASP resident child with the host native action.",
                    platform_action: "Use the host-native close/archive action for the existing ASP-managed child. Wait for terminal host status or the SubagentStop receipt; previous_status=running only confirms the request. Do not create a replacement or re-enter Audit before shutdown completes.",
                    next_state: AgentSessionLoopState::Cleanup,
                    required_inputs: &["nativeStopReceipt"],
                },
                AgentSessionInteractiveChoice {
                    id: "audit-after-cleanup",
                    label: "Re-enter audit after cleanup.",
                    platform_action: "Run the same interactive loop again so cleanup is followed by Audit and Classify, not direct replacement.",
                    next_state: AgentSessionLoopState::Audit,
                    required_inputs: &["rootSessionId"],
                },
            ],
            vec![AgentSessionLoopTraceStep {
                state: AgentSessionLoopState::Classify,
                result: "registered-child-stale-or-non-routable",
            }],
        );
    }
    let observed_model = record.model().filter(|model| {
        let normalized = model.trim();
        !normalized.is_empty() && !normalized.eq_ignore_ascii_case("unknown")
    });
    let expected_model = expected_model.filter(|model| !model.trim().is_empty());
    let model_unverified = expected_model.is_some() && observed_model.is_none();
    let model_mismatch = observed_model
        .is_some_and(|actual| expected_model.is_some_and(|expected| actual != expected));
    if (model_unverified || model_mismatch)
        && !(record.is_routable_at(now) && record.message_target_id().is_some())
    {
        return (
            AgentSessionLoopState::Validate,
            vec![
                AgentSessionInteractiveChoice {
                    id: "resume-managed-child-for-native-profile-observation",
                    label: "Resume the resident child through its configured managed profile.",
                    platform_action: "Use the host-native resume action once and immediately re-enter this pane. Do not wait for SubagentStart as a child message. Only the host event observed by this pane may update the model and message target. Do not ask the child to describe or switch its own model.",
                    next_state: AgentSessionLoopState::Audit,
                    required_inputs: &[],
                },
                AgentSessionInteractiveChoice {
                    id: "stop-mismatched-managed-child",
                    label: "Stop the mismatched managed child before creating another.",
                    platform_action: "Use the host-native stop action and wait until terminal host status or the SubagentStop receipt. previous_status=running is not shutdown completion. Re-enter only after the event retires this child route; do not delete registry state manually.",
                    next_state: AgentSessionLoopState::Cleanup,
                    required_inputs: &["nativeStopReceipt"],
                },
            ],
            vec![
                AgentSessionLoopTraceStep {
                    state: AgentSessionLoopState::Classify,
                    result: "registered-child-routable",
                },
                AgentSessionLoopTraceStep {
                    state: AgentSessionLoopState::Validate,
                    result: "model-missing-or-mismatch",
                },
            ],
        );
    }
    (
        AgentSessionLoopState::Ready,
        vec![
            AgentSessionInteractiveChoice {
                id: "send-denied-asp-command",
                label: "Send the denied ASP command to the resident child.",
                platform_action: "Use host-native message-agent send to the registered agentMessageTargetId; wait for a compact [asp-search-subagent] receipt.",
                next_state: AgentSessionLoopState::WaitReceipt,
                required_inputs: &["deniedAspCommand"],
            },
            AgentSessionInteractiveChoice {
                id: "record-native-child-retirement",
                label: "Retire the completed resident child before the next loop.",
                platform_action: "Use the host-native stop or archive action, then wait for terminal host status or the SubagentStop receipt. previous_status=running means shutdown was requested, not completed. Re-enter only after the event retires the matching route; do not return child identity or status as pane flags.",
                next_state: AgentSessionLoopState::Cleanup,
                required_inputs: &["nativeStopReceipt"],
            },
        ],
        vec![
            AgentSessionLoopTraceStep {
                state: AgentSessionLoopState::Classify,
                result: "registered-child-routable",
            },
            AgentSessionLoopTraceStep {
                state: AgentSessionLoopState::Validate,
                result: if observed_model.is_none() {
                    "profile-observation-pending"
                } else {
                    "role-nickname-model-target-pass"
                },
            },
            AgentSessionLoopTraceStep {
                state: AgentSessionLoopState::Ready,
                result: "resident-child-ready",
            },
        ],
    )
}

fn interactive_session_record(record: &AgentSessionRecord) -> AgentSessionInteractiveSession<'_> {
    AgentSessionInteractiveSession {
        child_session_id: &record.session_id,
        status: &record.status,
        role: &record.role,
        model: record.model(),
        model_observation_source: record.model_observation_source.as_deref(),
        model_observed_at: record.model_observed_at,
        model_evidence_ref: record.model_evidence_ref.as_deref(),
        message_target_status: if record.message_target_id().is_some() {
            "ready"
        } else {
            "missing"
        },
        message_target_id: record.message_target_id(),
    }
}
