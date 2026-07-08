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

pub fn resident_child_bootstrap_menu<'a>(
    platform: &'a str,
    name: &'a str,
    root_session_id: Option<&'a str>,
    record: Option<&'a AgentSessionRecord>,
    expected_model: Option<&'a str>,
    rollout_history_status: Option<&'a str>,
    rollout_history_action: Option<&'a str>,
    now: i64,
) -> AgentSessionInteractiveMenu<'a> {
    let (state, choices, trace) =
        resident_child_state_and_choices(record, expected_model, rollout_history_status, now);
    AgentSessionInteractiveMenu {
        schema_id: "agent.semantic-protocols.agent-session-interactive-menu",
        schema_version: "1",
        owner: "rust",
        state,
        name,
        root_session_id,
        expected_model,
        rollout_history_status,
        rollout_history_action,
        session: record.map(interactive_session_record),
        host_requirement: AgentSessionHostRequirement {
            platform,
            resident_child_name: name,
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
                    platform_action: "Use the {platform} native subagent creation surface, not a shell command: create the configured {managedAgentKind} resident child with the configured model, then capture the returned childSessionId and agentMessageTargetId.",
                    next_state: AgentSessionLoopState::Register,
                    required_inputs: &["configuredModel", "childSessionId", "agentMessageTargetId"],
                },
                AgentSessionInteractiveChoice {
                    id: "report-host-managed-agent-target-unavailable",
                    label: "Report that the host cannot create the managed ASP resident child.",
                    platform_action: "If {platform} exposes only native built-in agent types such as generic/default/explorer/worker, or returns only a normal thread id without agentMessageTargetId, report bootstrapBlocked=host-managed-agent-target-unavailable and do not create or register a generic replacement.",
                    next_state: AgentSessionLoopState::Create,
                    required_inputs: &["hostAgentTypesObserved"],
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
                    platform_action: "Use the {platform} native close/archive action for the existing ASP-managed child; do not create a generic replacement before cleanup.",
                    next_state: AgentSessionLoopState::Audit,
                    required_inputs: &["childSessionId"],
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
    if record.message_target_id().is_none() {
        return (
            AgentSessionLoopState::Recover,
            vec![
                AgentSessionInteractiveChoice {
                    id: "recover-native-message-target",
                    label: "Recover the native message target for this resident child.",
                    platform_action: "Use {platform} native agent messaging metadata for this existing child; agentMessageTargetId must be the host {requiredTransport} target. If the host exposes one single agent id and native send accepts it, register that id as agentMessageTargetId; never derive it from a normal thread id or rollout path.",
                    next_state: AgentSessionLoopState::Register,
                    required_inputs: &["agentMessageTargetId"],
                },
                AgentSessionInteractiveChoice {
                    id: "cleanup-unrecoverable-child",
                    label: "Close the child if its native message target cannot be recovered.",
                    platform_action: "Use the {platform} native close/archive action for the existing ASP-managed child, then re-enter this loop for Audit and Classify.",
                    next_state: AgentSessionLoopState::Cleanup,
                    required_inputs: &["childSessionId"],
                },
            ],
            vec![
                AgentSessionLoopTraceStep {
                    state: AgentSessionLoopState::Classify,
                    result: "registered-child-needs-recovery",
                },
                AgentSessionLoopTraceStep {
                    state: AgentSessionLoopState::Recover,
                    result: "native-message-target-missing",
                },
            ],
        );
    }
    let model_mismatch = expected_model
        .filter(|model| !model.trim().is_empty())
        .is_some_and(|expected| record.model() != Some(expected));
    if record.model().is_none() || model_mismatch {
        return (
            AgentSessionLoopState::Validate,
            vec![
                AgentSessionInteractiveChoice {
                    id: "confirm-configured-model",
                    label: "Confirm or switch the resident child to the configured model.",
                    platform_action: "Send a {platform} native {requiredTransport} follow-up to the existing child asking it to confirm or switch to the configured ASP model, then re-enter this loop.",
                    next_state: AgentSessionLoopState::Validate,
                    required_inputs: &["agentMessageTargetId", "configuredModel"],
                },
                AgentSessionInteractiveChoice {
                    id: "register-observed-model",
                    label: "Register the observed model after native confirmation.",
                    platform_action: "Update the same registry row with the model reported by the resident child; do not create a replacement only for model confirmation.",
                    next_state: AgentSessionLoopState::Validate,
                    required_inputs: &["childSessionId", "agentMessageTargetId", "observedModel"],
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
        vec![AgentSessionInteractiveChoice {
            id: "send-denied-asp-command",
            label: "Send the denied ASP command to the resident child.",
            platform_action: "{platform} native {requiredTransport} send to the registered agentMessageTargetId; wait for a compact [asp-search-subagent] receipt.",
            next_state: AgentSessionLoopState::WaitReceipt,
            required_inputs: &["deniedAspCommand"],
        }],
        vec![
            AgentSessionLoopTraceStep {
                state: AgentSessionLoopState::Classify,
                result: "registered-child-routable",
            },
            AgentSessionLoopTraceStep {
                state: AgentSessionLoopState::Validate,
                result: "role-nickname-model-target-pass",
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
        message_target_status: if record.message_target_id().is_some() {
            "ready"
        } else {
            "missing"
        },
        message_target_id: record.message_target_id(),
    }
}
