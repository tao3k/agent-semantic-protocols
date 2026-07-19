//! Runtime profile and evidence verdicts for resident lifecycle menus.

use super::interactive_loop_actions::model_repair_action;
use super::interactive_loop_types::{
    AgentSessionInteractiveChoice, AgentSessionInteractiveMenu, AgentSessionLoopState,
    AgentSessionLoopTraceStep,
};
use super::types::{AgentSessionRecord, agent_session_message_target_is_live_bound};

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
    menu.choices = resident_child_model_repair_choices(menu.name);
    menu
}

pub fn typed_runtime_observation_matches_profile(
    observed_agent_type: &str,
    expected_agent_type: &str,
    observed_model: &str,
    observed_reasoning_effort: Option<&str>,
    observation_source: &str,
    expected_model: Option<&str>,
    expected_reasoning_effort: Option<&str>,
) -> bool {
    observed_agent_type == expected_agent_type
        && matches!(expected_agent_type, "asp_explorer" | "asp_testing")
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
        platform_action: std::borrow::Cow::Borrowed(
            "ASP already attempted the host-owned Codex thread/resume metadata surface without sending a child turn or applying overrides. Report bootstrapBlocked=host-runtime-reasoning-evidence-unavailable and allow unrelated tool use. Preserve the configured resident; do not follow up merely to retrigger SubagentStart, close, replace, or duplicate the typed child.",
        ),
        next_state: AgentSessionLoopState::Audit,
        required_inputs: &["hostRuntimeReasoningEvidenceGapReceipt"],
    }];
    menu.trace.push(AgentSessionLoopTraceStep {
        state: AgentSessionLoopState::Blocked,
        result: "host-runtime-reasoning-evidence-unavailable",
    });
    menu
}

pub(super) fn resident_child_model_repair_choices<'a>(
    name: &str,
) -> Vec<AgentSessionInteractiveChoice<'a>> {
    vec![
        AgentSessionInteractiveChoice {
            id: "retire-drifted-child-and-create-configured-replacement",
            label: "Retire the drifted child and create one typed replacement from the registered profile.",
            platform_action: std::borrow::Cow::Borrowed(model_repair_action(name)),
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
        platform_action: std::borrow::Cow::Borrowed(
            "Report host-typed-resident-replacement-unavailable when the host lacks either native child retirement/path release or agent_type-aware creation. Keep ASP resident routing degraded, but do not deny unrelated Codex tools and do not re-enter bootstrap from the child. Never send natural-language model-switch instructions or create a generic replacement.",
        ),
        next_state: AgentSessionLoopState::Blocked,
        required_inputs: &["hostTypedReplacementCapabilityGap"],
    }
}
