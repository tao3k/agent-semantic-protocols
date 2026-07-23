use super::interactive_loop_actions::{
    managed_resident_create_action, ready_dispatch_action, runtime_observation_action,
    typed_spawn_audit_action,
};
use super::interactive_loop_runtime::resident_child_model_repair_choices;
use super::interactive_loop_types::{
    AgentSessionHostRequirement, AgentSessionInteractiveChoice, AgentSessionInteractiveMenu,
    AgentSessionInteractiveReceipt, AgentSessionInteractiveSession, AgentSessionLoopState,
    AgentSessionLoopTraceStep, ResidentChildBootstrapMenuInput,
};
use super::types::{AgentSessionRecord, agent_session_message_target_is_live_bound};

pub fn resident_child_bootstrap_menu<'a>(
    input: ResidentChildBootstrapMenuInput<'a>,
) -> AgentSessionInteractiveMenu<'a> {
    let (state, choices, trace) = resident_child_state_and_choices(
        input.name,
        input.record,
        input.root_session_id,
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
        session: input
            .record
            .map(|record| interactive_session_record(record, input.root_session_id)),
        host_requirement: AgentSessionHostRequirement {
            platform: input.platform,
            resident_child_name: input.name,
            managed_agent_kind: super::interactive_loop_actions::managed_agent_kind(input.name),
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
    name: &str,
    record: Option<&AgentSessionRecord>,
    root_session_id: Option<&str>,
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
                    platform_action: std::borrow::Cow::Borrowed(
                        "Run the loop-owned lifecycle audit for this root session and classify registered, rollout-only, stale, duplicate, and orphan-risk ASP children.",
                    ),
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
            AgentSessionLoopState::Audit,
            vec![
                AgentSessionInteractiveChoice {
                    id: "audit-host-agent-tree-for-existing-resident-child",
                    label: "Audit the native host agent tree before creating anything.",
                    platform_action: std::borrow::Cow::Borrowed(
                        "Use the native collaboration list-agents surface as the existence authority. Look for the canonical ASP resident task path, including completed, idle, interrupted, or running instances. Registry absence is not child absence. Do not create a child from this step.",
                    ),
                    next_state: AgentSessionLoopState::Classify,
                    required_inputs: &["hostAgentTreeSnapshot"],
                },
                AgentSessionInteractiveChoice {
                    id: "resume-existing-host-resident-child",
                    label: "Resume the same existing ASP resident child identity.",
                    platform_action: std::borrow::Cow::Borrowed(
                        "Only when the host agent tree contains the canonical ASP resident child, use the native follow-up-task surface on that same canonical target. For a running target, do not create a duplicate; wait for an idle boundary or interrupt only when the main agent intentionally redirects the current turn, then follow up on the same target. Re-enter this pane after the fresh host lifecycle observation so ASP can audit runtime settings and rehydrate the registry.",
                    ),
                    next_state: AgentSessionLoopState::Audit,
                    required_inputs: &["existingResidentChild"],
                },
                AgentSessionInteractiveChoice {
                    id: "audit-host-typed-spawn-schema",
                    label: "Audit that the native spawn tool exposes agent_type.",
                    platform_action: std::borrow::Cow::Borrowed(typed_spawn_audit_action(name)),
                    next_state: AgentSessionLoopState::Classify,
                    required_inputs: &["hostTypedSpawnObservation"],
                },
                AgentSessionInteractiveChoice {
                    id: "create-managed-resident-child-after-host-tree-miss",
                    label: "Create the configured ASP resident child only after both audits miss.",
                    platform_action: managed_resident_create_action(
                        name,
                        super::interactive_loop_actions::managed_agent_kind(name).as_ref(),
                    )
                    .into(),
                    next_state: AgentSessionLoopState::Audit,
                    required_inputs: &["hostTreeNoResidentChild", "typedSpawnAgentSchema"],
                },
                AgentSessionInteractiveChoice {
                    id: "report-host-agent-tree-audit-unavailable",
                    label: "Report that the host agent tree cannot be audited.",
                    platform_action: std::borrow::Cow::Borrowed(
                        "If the host exposes no native agent-tree listing surface, report bootstrapBlocked=host-agent-tree-audit-unavailable. Registry and rollout misses are insufficient evidence for Create; do not create or register a replacement.",
                    ),
                    next_state: AgentSessionLoopState::Audit,
                    required_inputs: &["hostTreeAuditGapObserved"],
                },
            ],
            vec![
                AgentSessionLoopTraceStep {
                    state: AgentSessionLoopState::Audit,
                    result: "checked-no-reusable-rollout",
                },
                AgentSessionLoopTraceStep {
                    state: AgentSessionLoopState::Classify,
                    result: "registry-missing-host-tree-audit-required",
                },
            ],
        );
    };
    if matches!(
        record.status.as_str(),
        "archived" | "closed" | "deleted" | "expired" | "invalid" | "missing" | "orphan-risk"
    ) {
        return (
            AgentSessionLoopState::Cleanup,
            vec![
                AgentSessionInteractiveChoice {
                    id: "close-stale-resident-child",
                    label: "Close or archive the stale ASP resident child with the host native action.",
                    platform_action: std::borrow::Cow::Borrowed(
                        "Use the host-native close/archive action only when the current host can resolve the existing ASP-managed child. A historical-only child with no native target is already non-rebindable and must not be resumed by ID. Wait for terminal host status or the SubagentStop receipt when a live target exists; do not rotate to another rollout candidate.",
                    ),
                    next_state: AgentSessionLoopState::Cleanup,
                    required_inputs: &["nativeStopReceiptOrHistoricalTargetAbsent"],
                },
                AgentSessionInteractiveChoice {
                    id: "audit-after-cleanup",
                    label: "Re-enter audit after cleanup.",
                    platform_action: std::borrow::Cow::Borrowed(
                        "Run the same interactive loop again so cleanup is followed by Audit and Classify, not direct replacement.",
                    ),
                    next_state: AgentSessionLoopState::Audit,
                    required_inputs: &["rootSessionId"],
                },
            ],
            vec![AgentSessionLoopTraceStep {
                state: AgentSessionLoopState::Classify,
                result: "historical-or-stale-child-not-live-rebindable",
            }],
        );
    }
    let live_message_target_bound = root_session_id
        .is_some_and(|root| agent_session_message_target_is_live_bound(record, root));
    if !live_message_target_bound {
        return (
            AgentSessionLoopState::RebindExistingChildTarget,
            vec![AgentSessionInteractiveChoice {
                id: "resume-existing-child-for-live-target-rebind",
                label: "Resume the same existing resident child to establish a live target binding.",
                platform_action: std::borrow::Cow::Borrowed(
                    "Use the main agent's native same-child follow-up/resume surface for the canonical existing managed target, record a fresh same-root host-tree target observation, then immediately re-enter this pane. The binding CAS must combine that verified-live target with the generation's durable typed SubagentStart profile evidence. Preserve the child identity; do not create a replacement, controller sibling, or manually register an id.",
                ),
                next_state: AgentSessionLoopState::Audit,
                required_inputs: &["freshSameRootHostTargetObservation"],
            }],
            vec![
                AgentSessionLoopTraceStep {
                    state: AgentSessionLoopState::Classify,
                    result: "existing-child-discovered",
                },
                AgentSessionLoopTraceStep {
                    state: AgentSessionLoopState::RebindExistingChildTarget,
                    result: "live-collaboration-target-unbound",
                },
            ],
        );
    }
    if !record.is_routable_at(now) {
        return (
            AgentSessionLoopState::Cleanup,
            vec![AgentSessionInteractiveChoice {
                id: "close-stale-resident-child",
                label: "Close or archive the expired ASP resident child with the host native action.",
                platform_action: std::borrow::Cow::Borrowed(
                    "The registry lease is no longer routable. Close/archive the live target when the host can resolve it, then re-enter Audit. Do not treat an expired binding as Ready and do not rotate to another historical rollout candidate.",
                ),
                next_state: AgentSessionLoopState::Cleanup,
                required_inputs: &["nativeStopReceiptOrExpiredBindingCleanup"],
            }],
            vec![AgentSessionLoopTraceStep {
                state: AgentSessionLoopState::Classify,
                result: "registered-child-binding-expired-or-non-routable",
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
    if model_unverified {
        return (
            AgentSessionLoopState::Validate,
            vec![AgentSessionInteractiveChoice {
                id: "resume-existing-child-for-runtime-observation",
                label: "Resume the same resident child to obtain a fresh runtime observation.",
                platform_action: std::borrow::Cow::Borrowed(runtime_observation_action(name)),
                next_state: AgentSessionLoopState::Audit,
                required_inputs: &["freshSameRootSubagentStartRuntimeObservation"],
            }],
            vec![
                AgentSessionLoopTraceStep {
                    state: AgentSessionLoopState::Classify,
                    result: "registered-child-routable",
                },
                AgentSessionLoopTraceStep {
                    state: AgentSessionLoopState::Validate,
                    result: "model-observation-missing",
                },
            ],
        );
    }
    if model_mismatch {
        return (
            AgentSessionLoopState::Repair,
            resident_child_model_repair_choices(name),
            vec![
                AgentSessionLoopTraceStep {
                    state: AgentSessionLoopState::Classify,
                    result: "registered-child-routable",
                },
                AgentSessionLoopTraceStep {
                    state: AgentSessionLoopState::Repair,
                    result: "model-mismatch",
                },
            ],
        );
    }
    (
        AgentSessionLoopState::Ready,
        vec![AgentSessionInteractiveChoice {
            id: "dispatch-resident-command",
            label: "Dispatch the routed command to the configured resident.",
            platform_action: std::borrow::Cow::Borrowed(ready_dispatch_action(name)),
            next_state: AgentSessionLoopState::WaitReceipt,
            required_inputs: &["deniedAspCommand", "receiptKind"],
        }],
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

fn interactive_session_record<'a>(
    record: &'a AgentSessionRecord,
    root_session_id: Option<&str>,
) -> AgentSessionInteractiveSession<'a> {
    let live_message_target_bound = root_session_id
        .is_some_and(|root| agent_session_message_target_is_live_bound(record, root));
    AgentSessionInteractiveSession {
        child_session_id: &record.session_id,
        status: &record.status,
        role: &record.role,
        model: record.model(),
        model_observation_source: record.model_observation_source.as_deref(),
        model_observed_at: record.model_observed_at,
        model_evidence_ref: record.model_evidence_ref.as_deref(),
        message_target_status: if live_message_target_bound {
            "ready"
        } else {
            "unbound"
        },
        message_target_id: record.message_target_id(),
    }
}
