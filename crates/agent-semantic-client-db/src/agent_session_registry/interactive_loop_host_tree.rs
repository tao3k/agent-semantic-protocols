//! Fresh native host-tree transitions for resident lifecycle menus.

use super::interactive_loop_actions::{
    host_tree_audit_action, host_tree_resume_action, orphan_replacement_action,
    typed_spawn_audit_action,
};
use super::interactive_loop_types::{
    AgentSessionInteractiveChoice, AgentSessionInteractiveMenu, AgentSessionLoopState,
    AgentSessionLoopTraceStep,
};

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
        platform_action: host_tree_audit_action(menu.name),
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
                platform_action: host_tree_resume_action(menu.name),
                next_state: AgentSessionLoopState::Audit,
                required_inputs: &["freshSameRootHostTargetObservation"],
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
            platform_action: orphan_replacement_action(menu.name),
            next_state: AgentSessionLoopState::Audit,
            required_inputs: &[
                "freshHostTreeAbsentObservation",
                "freshTypedSpawnPresentObservation",
            ],
        }],
        Some("absent") => {
            menu.state = AgentSessionLoopState::Blocked;
            vec![AgentSessionInteractiveChoice {
                id: "report-host-typed-spawn-capability-unavailable",
                label: "Report the local resident-command capability blocker.",
                platform_action: "Report bootstrapBlocked=host-typed-spawn-unavailable. Do not execute the resident-routed command, create a generic child, use a historical target, or run an inline parser fallback. Unrelated Codex tools remain available.",
                next_state: AgentSessionLoopState::Blocked,
                required_inputs: &[
                    "freshHostTreeAbsentObservation",
                    "freshHostTypedSpawnAbsentObservation",
                ],
            }]
        }
        _ => vec![AgentSessionInteractiveChoice {
            id: "audit-host-typed-spawn-schema",
            label: "Audit typed spawn before replacing the orphaned registry owner.",
            platform_action: typed_spawn_audit_action(menu.name),
            next_state: AgentSessionLoopState::Classify,
            required_inputs: &["hostTypedSpawnObservation"],
        }],
    };
    menu
}
