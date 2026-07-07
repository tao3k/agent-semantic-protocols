use super::hook_runtime_agent_session_rollout_topology::{
    nested_resident_child_decision, register_required_resident_child_decision,
};
use super::{
    AspSessionPolicy, HookDecision, agent_session_allow_decision, command_contains_asp_binary,
    command_requires_resident_child, first_restricted_main_session_asp_command,
    main_session_restricted_asp_command_decision, main_session_route_context,
    missing_resident_asp_explore_decision, payload_command_strings, unix_timestamp,
};
use std::path::Path;

pub(in crate::command) fn classify_activation_failure_main_session_asp(
    project_root: &Path,
    platform: &str,
    event: &str,
    payload: &serde_json::Value,
    asp_session_policy: &AspSessionPolicy,
) -> Result<Option<HookDecision>, String> {
    if !asp_session_policy.enabled() || event != "pre-tool" {
        return Ok(None);
    }
    let commands = payload_command_strings(payload);
    if commands.is_empty() {
        return Ok(None);
    }
    let mut context = main_session_route_context(project_root, asp_session_policy)?;
    let unusable_explore_session = matches!(
        context
            .active_explore_session
            .as_ref()
            .map(|session| session.status.as_str()),
        Some("archived" | "closed" | "deleted" | "expired" | "invalid" | "missing" | "orphan-risk")
    );
    if unusable_explore_session {
        context.active_explore_session = None;
    }
    let now = unix_timestamp()?;
    if context.current_is_active_resident_child(now, asp_session_policy) {
        if commands
            .iter()
            .any(|command| command_contains_asp_binary(command))
        {
            return Ok(Some(agent_session_allow_decision(
                platform,
                event,
                payload,
                "active-resident-child",
                "ASP allowed resident asp-explore child session command.",
            )));
        }
        return Ok(None);
    }
    if context.outside_agent_session() {
        return Ok(None);
    }
    let requires_resident_child = commands.iter().any(|command| {
        command_requires_resident_child(command, |tokens, index| {
            asp_session_policy.main_asp_command_allowed(tokens, index)
        })
    });
    if context.active_explore_session.is_none() && requires_resident_child {
        if let Some(topology) = context.current_nested_resident_child(asp_session_policy) {
            return Ok(Some(nested_resident_child_decision(
                platform,
                event,
                payload,
                topology,
                asp_session_policy,
            )));
        }
        if !unusable_explore_session
            && let Some(topology) =
                context.current_register_required_resident_child(asp_session_policy)
        {
            return Ok(Some(register_required_resident_child_decision(
                platform,
                event,
                payload,
                topology,
                asp_session_policy,
            )));
        }
        return Ok(Some(missing_resident_asp_explore_decision(
            platform,
            event,
            payload,
            commands.first().map(String::as_str),
            context.root_session_id,
            asp_session_policy,
        )));
    }
    if let Some((command, invocation)) =
        first_restricted_main_session_asp_command(&commands, asp_session_policy)
    {
        return Ok(Some(main_session_restricted_asp_command_decision(
            platform,
            event,
            payload,
            command,
            invocation,
            context.active_explore_session.as_ref(),
            asp_session_policy,
        )));
    }
    Ok(None)
}
