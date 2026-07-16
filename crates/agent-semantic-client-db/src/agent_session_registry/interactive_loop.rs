use agent_semantic_loop::{Choice, HostRequirement, LoopReceipt, TraceStep};
use serde::Serialize;

use super::types::{AgentSessionRecord, agent_session_message_target_is_live_bound};

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
    RebindExistingChildTarget,
    Repair,
    Blocked,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SameChildRuntimeOverrideState {
    Active,
    ReplacementRequired,
}

impl SameChildRuntimeOverrideState {
    pub fn registry_status(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::ReplacementRequired => "replacement-required",
        }
    }
}

pub fn classify_same_child_runtime_override_state(
    _current_status: &str,
    runtime_matches: bool,
    _fresh_after_previous_observation: bool,
) -> SameChildRuntimeOverrideState {
    if runtime_matches {
        return SameChildRuntimeOverrideState::Active;
    }
    SameChildRuntimeOverrideState::ReplacementRequired
}

pub fn resident_child_host_runtime_refresh_eligible(
    registry_routable: bool,
    record: &AgentSessionRecord,
    current_root_session_id: &str,
) -> bool {
    registry_routable
        || (record.status == "replacement-required"
            && agent_session_message_target_is_live_bound(record, current_root_session_id))
}

/// Replace the normal create/reuse choices with typed-child replacement.
///
/// Codex does not expose a runtime model/profile override on follow-up or
/// resume. A drifted child must therefore be retired and recreated through the
/// registered custom-agent role. Runtime drift is diagnostic and must never
/// become a global tool-use blocker.
pub fn resident_child_runtime_repair_menu(
    mut menu: AgentSessionInteractiveMenu<'_>,
    _consecutive_observation_count: usize,
) -> AgentSessionInteractiveMenu<'_> {
    menu.state = AgentSessionLoopState::Repair;
    menu.trace = vec![
        AgentSessionLoopTraceStep {
            state: AgentSessionLoopState::Classify,
            result: "resident-child-runtime-drift",
        },
        AgentSessionLoopTraceStep {
            state: AgentSessionLoopState::Repair,
            result: "typed-resident-replacement-required",
        },
    ];
    menu.choices = resident_child_model_repair_choices();
    menu
}

/// Preserve a positive same-child runtime-switch receipt instead of collapsing
/// it into "no drift". Readiness additionally requires a routable registry row.
pub fn resident_child_runtime_verified_menu(
    mut menu: AgentSessionInteractiveMenu<'_>,
    registry_routable: bool,
) -> AgentSessionInteractiveMenu<'_> {
    menu.trace = vec![
        AgentSessionLoopTraceStep {
            state: AgentSessionLoopState::Classify,
            result: "same-resident-child-identity",
        },
        AgentSessionLoopTraceStep {
            state: AgentSessionLoopState::Repair,
            result: "fresh-runtime-observation-matches-expected",
        },
        AgentSessionLoopTraceStep {
            state: AgentSessionLoopState::Validate,
            result: "typed-resident-replacement-verified",
        },
    ];
    if registry_routable {
        menu.state = AgentSessionLoopState::Ready;
        menu.trace.push(AgentSessionLoopTraceStep {
            state: AgentSessionLoopState::Ready,
            result: "resident-child-ready-after-verified-runtime-switch",
        });
        menu.choices = vec![AgentSessionInteractiveChoice {
            id: "send-denied-asp-command",
            label: "Send the denied ASP command to the verified resident child.",
            platform_action: "Derive one dispatch identity from the root session, registered agentMessageTargetId, and exact denied ASP command. Use host-native message-agent send exactly once for that identity, then wait for a compact [asp-search-subagent] receipt. A timeout or repeated bootstrap may only poll/wait for the same receipt; it must never resend the command or concatenate a second search output block.",
            next_state: AgentSessionLoopState::WaitReceipt,
            required_inputs: &["deniedAspCommand", "dispatchIdentity"],
        }];
    } else {
        menu.state = AgentSessionLoopState::Register;
        menu.trace.push(AgentSessionLoopTraceStep {
            state: AgentSessionLoopState::Register,
            result: "verified-runtime-registry-rehydration-required",
        });
        menu.choices = vec![AgentSessionInteractiveChoice {
            id: "rehydrate-verified-existing-child-registry",
            label: "Rehydrate the verified same-child identity into the resident registry.",
            platform_action: "Use the ASP-owned verified lifecycle receipt to rehydrate this exact child and message target. Do not ask the user to enter ids, create another child, or infer success from task text. Re-enter bootstrap after ASP records the verified observation.",
            next_state: AgentSessionLoopState::Validate,
            required_inputs: &["aspOwnedRegistryRehydrationReceipt"],
        }];
    }
    menu
}

/// Apply a fresh native host-tree observation to the resident menu.
///
/// Persisted registry identity never proves that the current collaboration
/// runtime can still resolve the canonical target after a host restart.
pub fn typed_runtime_observation_matches_profile(
    observed_agent_type: &str,
    expected_agent_type: &str,
    observed_model: &str,
    observed_reasoning_effort: Option<&str>,
    observation_source: &str,
    expected_model: Option<&str>,
    expected_reasoning_effort: Option<&str>,
) -> bool {
    observed_agent_type == "asp_explorer"
        && expected_agent_type == "asp_explorer"
        && expected_model.is_some_and(|expected| observed_model == expected)
        && expected_reasoning_effort
            .is_none_or(|expected| observed_reasoning_effort == Some(expected))
        && matches!(
            observation_source,
            "subagent-start" | "codex-app-server-thread-resume-after-subagent-start"
        )
}

pub fn resident_child_runtime_evidence_incomplete_menu<'a>(
    mut menu: AgentSessionInteractiveMenu<'a>,
) -> AgentSessionInteractiveMenu<'a> {
    menu.state = AgentSessionLoopState::Blocked;
    menu.choices = vec![AgentSessionInteractiveChoice {
        id: "report-host-runtime-reasoning-evidence-unavailable",
        label: "Report that Codex runtime reasoning evidence is unavailable.",
        platform_action: "ASP already attempted the host-owned Codex thread/resume metadata surface without sending a child turn or applying overrides. Report bootstrapBlocked=host-runtime-reasoning-evidence-unavailable and allow unrelated tool use. Preserve /root/asp_explorer; do not follow up merely to retrigger SubagentStart, close, replace, or duplicate the typed child.",
        next_state: AgentSessionLoopState::Audit,
        required_inputs: &["hostRuntimeReasoningEvidenceGapReceipt"],
    }];
    menu.trace.push(AgentSessionLoopTraceStep {
        state: AgentSessionLoopState::Blocked,
        result: "host-runtime-reasoning-evidence-unavailable",
    });
    menu
}

pub fn resident_child_host_tree_audit_required_menu<'a>(
    mut menu: AgentSessionInteractiveMenu<'a>,
) -> AgentSessionInteractiveMenu<'a> {
    if menu.session.is_none() || menu.state != AgentSessionLoopState::RebindExistingChildTarget {
        return menu;
    }
    menu.state = AgentSessionLoopState::Audit;
    menu.choices = vec![AgentSessionInteractiveChoice {
        id: "audit-host-agent-tree-before-live-target-rebind",
        label: "Audit the current native host tree before attempting Resume.",
        platform_action: "Call the native collaboration list-agents surface for this root task. If /root/asp_explorer is absent, record `direnv exec . asp agent session observe-host-tree --name asp-explore --resident-target-status absent` and re-enter bootstrap. If it is present, record the corresponding present observation before attempting follow-up. A historical rollout ID alone is never a callable target.",
        next_state: AgentSessionLoopState::Classify,
        required_inputs: &["freshHostAgentTreeObservation"],
    }];
    menu.trace.push(AgentSessionLoopTraceStep {
        state: AgentSessionLoopState::Audit,
        result: "host-tree-observation-required-before-rebind",
    });
    menu
}

pub fn resident_child_host_tree_observation_menu<'a>(
    mut menu: AgentSessionInteractiveMenu<'a>,
    target_status: &str,
    typed_spawn_status: Option<&str>,
) -> AgentSessionInteractiveMenu<'a> {
    if target_status == "present" {
        if menu.session.is_none() {
            menu.choices
                .retain(|choice| choice.id == "resume-existing-host-resident-child");
            menu.trace.push(AgentSessionLoopTraceStep {
                state: AgentSessionLoopState::Classify,
                result: "canonical-host-target-present-registry-rebind-required",
            });
        } else if matches!(
            menu.state,
            AgentSessionLoopState::Cleanup | AgentSessionLoopState::RebindExistingChildTarget
        ) {
            let completed_resident_is_resumable = menu.state == AgentSessionLoopState::Cleanup;
            menu.state = AgentSessionLoopState::RebindExistingChildTarget;
            menu.choices = vec![AgentSessionInteractiveChoice {
                id: "resume-existing-child-for-live-target-rebind",
                label: if completed_resident_is_resumable {
                    "Resume the completed host-visible canonical resident child."
                } else {
                    "Resume the host-visible canonical resident child."
                },
                platform_action: "Use the main agent's native follow-up surface for /root/asp_explorer. Completed or idle native children remain resumable and must keep the same identity. If the host returns target/path/id not found, treat that native failure as a fresh absence observation: run `direnv exec . asp agent session observe-host-tree --name asp-explore --resident-target-status absent`, then re-enter bootstrap. Do not close a host-visible child or retry a historical child ID.",
                next_state: AgentSessionLoopState::Audit,
                required_inputs: &["freshSameRootSubagentStartBindingOrTargetAbsentObservation"],
            }];
            menu.trace.push(AgentSessionLoopTraceStep {
                state: AgentSessionLoopState::Classify,
                result: if completed_resident_is_resumable {
                    "canonical-host-target-present-completed-resumable"
                } else {
                    "canonical-host-target-present-resume-once"
                },
            });
        }
        return menu;
    }
    if menu.session.is_none() {
        menu.choices.retain(|choice| {
            !matches!(
                choice.id,
                "audit-host-agent-tree-for-existing-resident-child"
                    | "resume-existing-host-resident-child"
                    | "report-host-agent-tree-audit-unavailable"
            )
        });
        menu.trace.push(AgentSessionLoopTraceStep {
            state: AgentSessionLoopState::Classify,
            result: "canonical-host-target-absent",
        });
        return menu;
    }
    menu.state = AgentSessionLoopState::Audit;
    menu.trace = vec![
        AgentSessionLoopTraceStep {
            state: AgentSessionLoopState::Classify,
            result: "persisted-resident-owner-found",
        },
        AgentSessionLoopTraceStep {
            state: AgentSessionLoopState::Audit,
            result: "canonical-host-target-absent-registry-orphan-risk",
        },
    ];
    menu.choices = match typed_spawn_status {
        Some("present") => vec![AgentSessionInteractiveChoice {
            id: "create-canonical-typed-child-after-orphaned-owner",
            label: "Create one canonical typed child after the host-tree miss.",
            platform_action: "The fresh native host-tree receipt proves /root/asp_explorer is absent. The registry owner is orphan-risk, so create exactly one child with agent_type=asp_explorer, task_name=asp_explorer, and fork_turns=none. SubagentStart atomically releases the orphaned registry owner and registers the new native identity.",
            next_state: AgentSessionLoopState::Audit,
            required_inputs: &[
                "freshHostTreeAbsentObservation",
                "freshTypedSpawnPresentObservation",
            ],
        }],
        Some("absent") => vec![AgentSessionInteractiveChoice {
            id: "activate-inline-parser-fallback",
            label: "Use the exact parser-owned ASP command without creating a generic child.",
            platform_action: "The fresh host-tree receipt proves the old canonical target is absent and the fresh typed-spawn receipt proves agent_type is unavailable. Rerun only the exact denied parser-owned ASP command with ASP_INLINE_PARSER_FALLBACK=1 before direnv exec. Do not register, resume, or create a generic child.",
            next_state: AgentSessionLoopState::Audit,
            required_inputs: &[
                "freshHostTreeAbsentObservation",
                "freshHostTypedSpawnAbsentObservation",
                "exactDeniedAspCommand",
            ],
        }],
        _ => vec![AgentSessionInteractiveChoice {
            id: "audit-host-typed-spawn-schema",
            label: "Audit typed spawn before replacing the orphaned registry owner.",
            platform_action: "Inspect collaboration.spawn_agent and record the result with `direnv exec . asp agent session observe-host-capability --name asp-explore --agent-type-field present|absent`, then re-enter bootstrap. Do not create anything before this receipt exists.",
            next_state: AgentSessionLoopState::Classify,
            required_inputs: &["hostTypedSpawnObservation"],
        }],
    };
    menu
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
            AgentSessionLoopState::Audit,
            vec![
                AgentSessionInteractiveChoice {
                    id: "audit-host-agent-tree-for-existing-resident-child",
                    label: "Audit the native host agent tree before creating anything.",
                    platform_action: "Use the native collaboration list-agents surface as the existence authority. Look for the canonical ASP resident task path, including completed, idle, interrupted, or running instances. Registry absence is not child absence. Do not create a child from this step.",
                    next_state: AgentSessionLoopState::Classify,
                    required_inputs: &["hostAgentTreeSnapshot"],
                },
                AgentSessionInteractiveChoice {
                    id: "resume-existing-host-resident-child",
                    label: "Resume the same existing ASP resident child identity.",
                    platform_action: "Only when the host agent tree contains the canonical ASP resident child, use the native follow-up-task surface on that same canonical target. For a running target, do not create a duplicate; wait for an idle boundary or interrupt only when the main agent intentionally redirects the current turn, then follow up on the same target. Re-enter this pane after the fresh host lifecycle observation so ASP can audit runtime settings and rehydrate the registry.",
                    next_state: AgentSessionLoopState::Audit,
                    required_inputs: &["existingResidentChild"],
                },
                AgentSessionInteractiveChoice {
                    id: "audit-host-typed-spawn-schema",
                    label: "Audit that the native spawn tool exposes agent_type.",
                    platform_action: "Before Create, inspect the currently exposed native collaboration.spawn_agent tool schema. It must contain an agent_type field that can be set to asp_explorer. task_name, message, and fork_turns alone are not typed-spawn capability. Record the observation through `direnv exec . asp agent session observe-host-capability --name asp-explore --agent-type-field present|absent`; do not merely report it in prose. Then re-enter bootstrap. If agent_type is absent, do not create any child.",
                    next_state: AgentSessionLoopState::Classify,
                    required_inputs: &["hostTypedSpawnObservation"],
                },
                AgentSessionInteractiveChoice {
                    id: "activate-inline-parser-fallback",
                    label: "Use the current session for the exact parser-owned ASP command when typed spawn is unavailable.",
                    platform_action: "Only after the native spawn schema audit proves that agent_type is unavailable, do not create a generic child. Rerun the exact denied parser-owned ASP search/query with ASP_INLINE_PARSER_FALLBACK=1 before direnv exec. This permits only that classified ASP reasoning command in the current root session; it does not authorize raw reads, shell search substitutes, or resident-child registration.",
                    next_state: AgentSessionLoopState::Audit,
                    required_inputs: &[
                        "freshHostTypedSpawnAbsentObservation",
                        "exactDeniedAspCommand",
                    ],
                },
                AgentSessionInteractiveChoice {
                    id: "create-managed-resident-child-after-host-tree-miss",
                    label: "Create the configured ASP resident child only after both audits miss.",
                    platform_action: "Only when rollout history and the native host agent tree both prove that no reusable ASP resident child exists, use a platform-native creation surface that explicitly exposes agent_type. Set agent_type=asp_explorer to select the registered profile, task_name=asp_explorer to reserve the canonical /root/asp_explorer path, and fork_turns=none, then create once. message text does not select the registered profile. If agent_type is not exposed, report host-agent-type-unavailable; do not create generic fallback agents or normal threads. Re-enter this pane after the native create call returns so SubagentStart can be audited.",
                    next_state: AgentSessionLoopState::Audit,
                    required_inputs: &["hostTreeNoResidentChild", "typedSpawnAgentSchema"],
                },
                AgentSessionInteractiveChoice {
                    id: "report-host-agent-tree-audit-unavailable",
                    label: "Report that the host agent tree cannot be audited.",
                    platform_action: "If the host exposes no native agent-tree listing surface, report bootstrapBlocked=host-agent-tree-audit-unavailable. Registry and rollout misses are insufficient evidence for Create; do not create or register a replacement.",
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
                    platform_action: "Use the host-native close/archive action only when the current host can resolve the existing ASP-managed child. A historical-only child with no native target is already non-rebindable and must not be resumed by ID. Wait for terminal host status or the SubagentStop receipt when a live target exists; do not rotate to another rollout candidate.",
                    next_state: AgentSessionLoopState::Cleanup,
                    required_inputs: &["nativeStopReceiptOrHistoricalTargetAbsent"],
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
                platform_action: "Use the main agent's native same-child follow-up/resume surface for the canonical existing managed target, then immediately re-enter this pane. A fresh same-root SubagentStart lifecycle hook must attest the collaboration target before Ready. Preserve the child identity; do not create a replacement, controller sibling, or manually register an id.",
                next_state: AgentSessionLoopState::Audit,
                required_inputs: &["freshSameRootSubagentStartBinding"],
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
                platform_action: "The registry lease is no longer routable. Close/archive the live target when the host can resolve it, then re-enter Audit. Do not treat an expired binding as Ready and do not rotate to another historical rollout candidate.",
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
                platform_action: "Use the main agent's native follow-up/resume surface for the same canonical /root/asp_explorer target. Wait for a fresh same-root SubagentStart model and reasoning observation, then re-enter bootstrap. Missing observation is not drift: do not retire the child or create a replacement.",
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
            resident_child_model_repair_choices(),
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
            id: "send-denied-asp-command",
            label: "Send the denied ASP command to the resident child.",
            platform_action: "Derive one dispatch identity from the root session, canonical /root/asp_explorer target, and exact denied ASP command. Send or follow up exactly once for that identity, then wait for a compact [asp-search-subagent] receipt. A timeout or repeated bootstrap may only poll/wait for the same receipt; it must never resend the command or concatenate a second search output block. Keep the resident child for later reuse. Do not retire it merely because one search turn completed.",
            next_state: AgentSessionLoopState::WaitReceipt,
            required_inputs: &["deniedAspCommand", "dispatchIdentity"],
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

fn resident_child_model_repair_choices<'a>() -> Vec<AgentSessionInteractiveChoice<'a>> {
    vec![
        AgentSessionInteractiveChoice {
            id: "retire-drifted-child-and-create-configured-replacement",
            label: "Retire the drifted child and create one typed replacement from the registered profile.",
            platform_action: "Use the host-native retire/archive action for the existing canonical child and wait for terminal status and /root/asp_explorer path release. Then create exactly one replacement through a spawn surface that exposes agent_type, with agent_type=asp_explorer, task_name=asp_explorer, and fork_turns=none. agent_type selects the registered role; task_name reserves its canonical collaboration path; message/natural-language task text is only payload. Let Codex load the full role configuration from ~/.codex/agents or the active higher-precedence agents directory; do not copy model or reasoning values into task text. If agent_type is unavailable, choose the blocker option instead of spawning. Re-enter this pane only after the replacement SubagentStart receipt.",
            next_state: AgentSessionLoopState::Audit,
            required_inputs: &["nativeRetireReceipt", "typedSubagentStartReceipt"],
        },
        resident_child_runtime_override_unavailable_choice(),
    ]
}

fn resident_child_runtime_override_unavailable_choice<'a>() -> AgentSessionInteractiveChoice<'a> {
    AgentSessionInteractiveChoice {
        id: "report-host-typed-replacement-unavailable",
        label: "Report that the host cannot retire and recreate the typed resident child.",
        platform_action: "Report host-typed-resident-replacement-unavailable when the host lacks either native child retirement/path release or agent_type-aware creation. Keep ASP resident routing degraded, but do not deny unrelated Codex tools and do not re-enter bootstrap from the child. Never send natural-language model-switch instructions or create a generic replacement.",
        next_state: AgentSessionLoopState::Blocked,
        required_inputs: &["hostTypedReplacementCapabilityGap"],
    }
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
