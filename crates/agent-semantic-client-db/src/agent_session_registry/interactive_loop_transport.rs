//! Live host transport gates for the resident-child interactive loop.

use super::interactive_loop_types::{
    AgentSessionInteractiveChoice, AgentSessionInteractiveMenu, AgentSessionLoopState,
    AgentSessionLoopTraceStep,
};

/// Preserve a positive runtime receipt without allowing historical registry
/// evidence to masquerade as a callable native message target.
pub fn resident_child_runtime_verified_menu(
    mut menu: AgentSessionInteractiveMenu<'_>,
    registry_routable: bool,
    live_transport_verified: bool,
) -> AgentSessionInteractiveMenu<'_> {
    menu.trace = verified_runtime_trace();
    match (registry_routable, live_transport_verified) {
        (true, true) => ready_menu(menu),
        (true, false) => {
            live_transport_rebind_menu(menu, "registry-routable-live-transport-unverified")
        }
        (false, _) => registry_rehydration_menu(menu),
    }
}

/// Apply this final gate after every runtime/profile projection.  No later
/// classifier may restore `Ready` from a historical rollout or registry row.
pub fn resident_child_live_transport_gate(
    menu: AgentSessionInteractiveMenu<'_>,
    live_transport_verified: bool,
) -> AgentSessionInteractiveMenu<'_> {
    if menu.state != AgentSessionLoopState::Ready || live_transport_verified {
        return menu;
    }
    live_transport_rebind_menu(menu, "ready-rejected-live-transport-unverified")
}

fn verified_runtime_trace() -> Vec<AgentSessionLoopTraceStep<'static>> {
    vec![
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
    ]
}

fn ready_menu(mut menu: AgentSessionInteractiveMenu<'_>) -> AgentSessionInteractiveMenu<'_> {
    menu.state = AgentSessionLoopState::Ready;
    menu.trace.push(AgentSessionLoopTraceStep {
        state: AgentSessionLoopState::Ready,
        result: "resident-child-ready-after-verified-runtime-switch",
    });
    menu.choices = vec![AgentSessionInteractiveChoice {
        id: "send-denied-asp-command",
        label: "Send the denied ASP command to the verified resident child.",
        platform_action: std::borrow::Cow::Borrowed(ready_dispatch_action(menu.name)),
        next_state: AgentSessionLoopState::WaitReceipt,
        required_inputs: &["deniedAspCommand", "dispatchIdentity"],
    }];
    menu
}

fn live_transport_rebind_menu<'a>(
    mut menu: AgentSessionInteractiveMenu<'a>,
    result: &'a str,
) -> AgentSessionInteractiveMenu<'a> {
    menu.state = AgentSessionLoopState::RebindExistingChildTarget;
    if let Some(session) = menu.session.as_mut() {
        session.message_target_status = "unbound";
    }
    menu.trace.push(AgentSessionLoopTraceStep {
        state: AgentSessionLoopState::RebindExistingChildTarget,
        result,
    });
    menu.choices = vec![AgentSessionInteractiveChoice {
        id: "verify-live-resident-transport-before-dispatch",
        label: "Verify the current native resident transport before dispatch.",
        platform_action: std::borrow::Cow::Owned(verify_live_transport_action(menu.name)),
        next_state: AgentSessionLoopState::Validate,
        required_inputs: &["freshVerifiedResidentTransportBinding"],
    }];
    menu
}

fn ready_dispatch_action(name: &str) -> &'static str {
    if name == "asp-testing" {
        "Derive one execution identity from the root session, registered agentMessageTargetId, canonical /root/asp_testing target, and exact denied command. Use host-native follow-up exactly once, then wait for a digest-bound [asp-testing-execution-v1] receipt. Never execute the command in the main agent."
    } else {
        "Derive one dispatch identity from the root session, registered agentMessageTargetId, and exact denied ASP command. Use host-native message-agent send exactly once for that identity, then wait for a compact [asp-search-subagent] receipt. A timeout or repeated bootstrap may only poll/wait for the same receipt; it must never resend the command or concatenate a second search output block."
    }
}

fn verify_live_transport_action(name: &str) -> String {
    let canonical_target = format!(
        "/root/{}",
        super::interactive_loop_actions::managed_agent_kind(name)
    );
    format!(
        "Audit the native tree for {canonical_target}, then use the main agent's native follow-up surface for that exact target once. If the follow-up succeeds, immediately record `direnv exec . asp agent session observe-host-ack --name {name} --canonical-target {canonical_target} --evidence-ref <native-followup-receipt>`, then re-enter bootstrap; this is the same-child live binding writeback. If the host returns target/path/id not found, record `direnv exec . asp agent session observe-host-tree --name {name} --resident-target-status absent`, then re-enter bootstrap. A historical rollout, registry row, or path-only observation cannot authorize dispatch or execution."
    )
}

fn registry_rehydration_menu(
    mut menu: AgentSessionInteractiveMenu<'_>,
) -> AgentSessionInteractiveMenu<'_> {
    menu.state = AgentSessionLoopState::Register;
    if let Some(session) = menu.session.as_mut() {
        session.message_target_status = "unbound";
    }
    menu.trace.push(AgentSessionLoopTraceStep {
        state: AgentSessionLoopState::Register,
        result: "verified-runtime-registry-rehydration-required",
    });
    menu.choices = vec![AgentSessionInteractiveChoice {
        id: "rehydrate-verified-existing-child-registry",
        label: "Rehydrate the verified same-child identity into the resident registry.",
        platform_action: std::borrow::Cow::Borrowed(
            "Use the ASP-owned verified lifecycle receipt to rehydrate this exact child and message target. Do not ask the user to enter ids, create another child, or infer success from task text. Re-enter bootstrap after ASP records the verified observation.",
        ),
        next_state: AgentSessionLoopState::Validate,
        required_inputs: &["aspOwnedRegistryRehydrationReceipt"],
    }];
    menu
}
