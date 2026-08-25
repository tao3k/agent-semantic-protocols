use super::choice_plane::{
    CONTROL_PLANE_CONTRACT_ID, PANE_SCHEMA_ID, ResolvedHookSessionRoute, SessionRegistryContext,
};
use crate::command::org_capture_interactive::{
    AdmittedAgentInteractiveChoice, AgentInteractiveChoice,
};
use agent_semantic_config::agent_route_registry::CompiledAgentRoute;

pub(super) fn render_session_pane(
    json: bool,
    interactive_contract: &AgentInteractiveChoice,
    hook_route: &ResolvedHookSessionRoute,
    route: &CompiledAgentRoute,
    sandbox_mode: &str,
    context: &SessionRegistryContext,
    inbox_reconciliation_failure: Option<&str>,
    choices: &[AdmittedAgentInteractiveChoice],
) -> Result<String, String> {
    if let SessionRegistryContext::Ready(resolved) = context
        && resolved.name != route.platform_host_agent_name.as_str()
    {
        return Err("runtime-server-session-route-mismatch".to_owned());
    }
    let generation = context.generation();
    let node_id = context.state();
    let reason_kind = context.reason_kind();
    let failure = context.failure();
    let binding = context.binding();
    let history_cursor = pane_history_cursor(context, generation);
    let choice_receipts = choices
        .iter()
        .map(|choice| {
            serde_json::json!({
                "id": choice.id,
                "instruction": choice.instruction,
                "presentation": choice.presentation,
                "why": choice.use_if,
            })
        })
        .collect::<Vec<_>>();
    let hook_inbox = hook_inbox_reconciliation_receipt(inbox_reconciliation_failure);
    let host_invocation = route.host_invocation(&hook_route.target_agent_symbol);
    let receipt = serde_json::json!({
        "schemaId": PANE_SCHEMA_ID,
        "schemaVersion": "1",
        "orgInteractive": {
            "contractId": CONTROL_PLANE_CONTRACT_ID,
            "panelId": "multi-agent-session-control-plane",
            "method": "choice",
            "stage": "presentation",
        },
        "hookRoute": {
            "configRuleId": hook_route.config_rule_id,
            "targetAgent": hook_route.target_agent,
            "targetAgentSymbol": &host_invocation.symbol,
            "receiptKind": hook_route.receipt_kind,
            "commandDigest": hook_route.command_digest,
            "reasonKind": hook_route.reason_kind,
        },
        "pane": {
            "paneId": "session",
            "nodeId": node_id,
            "generation": generation,
            "historyCursor": history_cursor,
            "reasonKind": reason_kind,
            "failure": failure,
        },
        "hookInbox": hook_inbox,
        "agent": {
            "routeKey": route.route_key.as_str(),
            "sessionName": route.platform_host_agent_name.as_str(),
            "sessionLifetime": route.session_lifetime.as_str(),
            "residentId": route.platform_host_agent_name.as_str(),
            "matchDecision": "matched",
            "platform": route.platform.as_str(),
            "hostAgentName": route.platform_host_agent_name.as_str(),
            "hostInvocation": host_invocation,
            "roles": &route.roles,
            "roleDescription": route.description.as_str(),
            "sandboxMode": sandbox_mode,
            "binding": binding,
        },
        "choices": choice_receipts,
    });
    if json {
        return Ok(receipt.to_string());
    }
    let mut pane_context = format!(
        "node={} generation={} agent=@{} sandboxMode={}",
        node_id,
        generation,
        route.platform_host_agent_name.as_str(),
        sandbox_mode,
    );
    if let Some(reason_kind) = reason_kind {
        pane_context.push_str(&format!(" reasonKind={reason_kind}"));
    }
    if let Some(failure) = failure {
        pane_context.push_str(&format!(" failure={failure}"));
    }
    if let Some(inbox_failure) = inbox_reconciliation_failure {
        pane_context.push_str(&format!(
            " hookInboxState=degraded hookInboxBlocksChoicePlane=false hookInboxFailure={inbox_failure}"
        ));
    }
    if choices.len() == 1 && choices[0].presentation == "action" {
        return Ok(interactive_contract.render_admitted_action(
            CONTROL_PLANE_CONTRACT_ID,
            &choices[0],
            &pane_context,
        ));
    }
    let rendered_choices = choices
        .iter()
        .map(|choice| {
            (
                choice.id.as_str(),
                choice.instruction.as_str(),
                choice.use_if.as_str(),
            )
        })
        .collect::<Vec<_>>();
    Ok(interactive_contract.render_admitted_pane(
        CONTROL_PLANE_CONTRACT_ID,
        &rendered_choices,
        &pane_context,
    ))
}

pub(crate) fn hook_inbox_reconciliation_receipt(failure: Option<&str>) -> serde_json::Value {
    serde_json::json!({
        "authority": "recovery-log-only",
        "state": if failure.is_some() { "degraded" } else { "reconciled" },
        "failure": failure,
        "blocksChoicePlane": false,
    })
}

fn pane_history_cursor(context: &SessionRegistryContext, generation: u64) -> String {
    match context {
        SessionRegistryContext::Ready(state) => format!(
            "{}:{}:session:{generation}",
            state.project_id.as_deref().unwrap_or("unavailable"),
            state.root_session_id.as_deref().unwrap_or("unavailable"),
        ),
        SessionRegistryContext::Blocked { .. } => format!("unavailable:session:{generation}"),
    }
}
