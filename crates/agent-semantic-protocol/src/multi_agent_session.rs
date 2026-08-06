//! Runtime- and Org-backed Multi-Agent Lifecycle ChoicePlane.

use std::path::{Path, PathBuf};

use agent_semantic_client_db::{AgentSessionControlPlaneState, AgentSessionRegistry};
use agent_semantic_config::agent_route_registry::{
    AgentsRegistry, CompiledAgentRoute, compile_agent_route, load_agent_route_registry,
};
use agent_semantic_config::load_hook_client_config_file;
use agent_semantic_hook::{HookSessionAgentRoute, latest_hook_session_agent_route};

use crate::command::org_capture_interactive::{
    AdmittedAgentInteractiveChoice, AgentInteractiveChoice,
};

pub(crate) const PANE_SCHEMA_ID: &str =
    "agent.semantic-protocols.multi-agent-session-control-plane-pane";
pub(crate) const CONTROL_PLANE_CONTRACT_ID: &str = "agent.multi-agent-session-control-plane.v1";
pub(crate) const CONTROL_PLANE_CONTRACT_FILE: &str =
    "agent.multi-agent-session-control-plane.v1.org";
pub(crate) const CONTROL_PLANE_CONTRACT_SOURCE: &str =
    include_str!("../../../org/contracts/agent.multi-agent-session-control-plane.v1.org");

pub(crate) struct ChoicePlaneRequest<'a> {
    pub project_root: &'a Path,
    pub platform: &'a str,
    pub json: bool,
}

pub(crate) fn open_choice_plane(request: ChoicePlaneRequest<'_>) -> Result<String, String> {
    let hook_route = latest_hook_session_agent_route(request.project_root)?.ok_or_else(|| {
                "hook-deny-session-route-required: asp session --agents choice-plane requires a current config-selected denied Hook event"
                    .to_owned()
            })?;
    let state = agent_semantic_runtime::state_core::ResolvedState::resolve(request.project_root)
        .map_err(|error| format!("failed to resolve session ChoicePlane state: {error}"))?;
    let loaded = load_agent_route_registry(&agents_root(&state.state_home).join("config.toml"))?;
    let (route, resolved_hook_route) =
        compile_hook_selected_route(&loaded, &hook_route, request.platform, &state.state_home)?;
    let interactive_contract = AgentInteractiveChoice::from_source(
        CONTROL_PLANE_CONTRACT_SOURCE,
        CONTROL_PLANE_CONTRACT_FILE,
        "presentation",
    )?;
    let context = SessionRegistryContext::resolve(
        request.project_root,
        None,
        Some(hook_route.root_session_id.clone()),
        route.platform_host_agent_name.as_str(),
    );
    let choices = interactive_contract.admit_matching(&[
        ("SESSION_STATE", context.state()),
        (
            "REGISTERED_AGENT_NAME",
            route.platform_host_agent_name.as_str(),
        ),
        ("ROLE_DESCRIPTION", route.description.as_str()),
    ])?;
    render_session_pane(
        request.json,
        &interactive_contract,
        &resolved_hook_route,
        &route,
        &context,
        &choices,
    )
}

struct ResolvedHookSessionRoute {
    config_rule_id: String,
    target_agent_name: String,
    target_agent_role: String,
    receipt_kind: String,
    command_digest: Option<String>,
    reason_kind: String,
}

fn compile_hook_selected_route(
    loaded: &AgentsRegistry,
    hook_route: &HookSessionAgentRoute,
    platform: &str,
    state_home: &Path,
) -> Result<(CompiledAgentRoute, ResolvedHookSessionRoute), String> {
    let hook_config = load_hook_client_config_file(&state_home.join("hooks/config.toml"))?;
    let rule = hook_config
        .rules
        .iter()
        .find(|rule| rule.id == hook_route.config_rule_id)
        .ok_or_else(|| {
            format!(
                "hook-session-rule-missing: managed Hook config no longer contains `{}`",
                hook_route.config_rule_id
            )
        })?;
    let dispatch = rule.dispatch.as_ref().ok_or_else(|| {
        format!(
            "hook-session-dispatch-missing: Hook rule `{}` does not declare a semantic role",
            hook_route.config_rule_id
        )
    })?;
    let route_key = hook_config
        .agents
        .placeholders
        .get(dispatch.role.as_str())
        .ok_or_else(|| {
            format!(
                "hook-session-role-placeholder-missing: rule `{}` role `{}` has no agents.placeholders binding",
                hook_route.config_rule_id,
                dispatch.role.as_str()
            )
        })?;
    let route = compile_agent_route(loaded, route_key, platform)?;
    let identity_matches = route_key == route.route_key.as_str()
        || route_key == route.platform_host_agent_name.as_str();
    if !identity_matches {
        return Err(format!(
            "hook-agent-route-registry-mismatch: rule `{}` role `{}` resolved `{route_key}`, but registry route `{}` owns session `{}`",
            hook_route.config_rule_id,
            dispatch.role.as_str(),
            route.route_key.as_str(),
            route.platform_host_agent_name.as_str(),
        ));
    }
    let resolved = ResolvedHookSessionRoute {
        config_rule_id: hook_route.config_rule_id.clone(),
        target_agent_name: route.platform_host_agent_name.as_str().to_owned(),
        target_agent_role: dispatch.role.as_str().to_owned(),
        receipt_kind: dispatch.receipt_kind.as_str().to_owned(),
        command_digest: hook_route.command_digest.clone(),
        reason_kind: hook_route.reason_kind.clone(),
    };
    Ok((route, resolved))
}

enum SessionRegistryContext {
    Ready(AgentSessionControlPlaneState),
    Blocked {
        reason_kind: String,
        failure: String,
    },
}

impl SessionRegistryContext {
    fn resolve(
        project_root: &Path,
        session_id: Option<String>,
        root_session_id: Option<String>,
        name: &str,
    ) -> Self {
        match AgentSessionRegistry::resolve_project_session_control_plane(
            project_root,
            session_id.as_deref(),
            root_session_id.as_deref(),
            name,
        ) {
            Ok(state) => Self::Ready(state),
            Err(error) => Self::Blocked {
                reason_kind: typed_runtime_failure_reason(
                    &error,
                    "runtime-server-session-control-plane-unavailable",
                ),
                failure: error,
            },
        }
    }

    fn state(&self) -> &str {
        match self {
            Self::Ready(state) => state.state.as_str(),
            Self::Blocked { .. } => "blocked",
        }
    }

    fn generation(&self) -> u64 {
        match self {
            Self::Ready(state) => state.generation,
            Self::Blocked { .. } => 0,
        }
    }

    fn reason_kind(&self) -> Option<&str> {
        match self {
            Self::Ready(state) => state.reason_kind.as_deref(),
            Self::Blocked { reason_kind, .. } => Some(reason_kind),
        }
    }

    fn failure(&self) -> Option<&str> {
        match self {
            Self::Ready(_) => None,
            Self::Blocked { failure, .. } => Some(failure),
        }
    }
}

pub(crate) fn typed_runtime_failure_reason(error: &str, fallback: &str) -> String {
    error
        .split_once(':')
        .map(|(code, _)| code.trim())
        .filter(|code| {
            !code.is_empty()
                && code
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
        .unwrap_or(fallback)
        .to_owned()
}

fn render_session_pane(
    json: bool,
    interactive_contract: &AgentInteractiveChoice,
    hook_route: &ResolvedHookSessionRoute,
    route: &CompiledAgentRoute,
    context: &SessionRegistryContext,
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
            "targetAgentName": hook_route.target_agent_name,
            "targetAgentRole": hook_route.target_agent_role,
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
        "agent": {
            "routeKey": route.route_key.as_str(),
            "sessionName": route.platform_host_agent_name.as_str(),
            "sessionLifetime": route.session_lifetime.as_str(),
            "platform": route.platform.as_str(),
            "hostAgentName": route.platform_host_agent_name.as_str(),
            "roles": &route.roles,
            "roleDescription": route.description.as_str(),
        },
        "choices": choice_receipts,
    });
    if json {
        return Ok(receipt.to_string());
    }
    let pane_context = format!(
        "pane=session node={} generation={} historyCursor={} hookRule={} receiptKind={} commandDigest={} reasonKind={} failure={}",
        node_id,
        generation,
        history_cursor,
        hook_route.config_rule_id,
        hook_route.receipt_kind,
        hook_route.command_digest.as_deref().unwrap_or("none"),
        reason_kind.unwrap_or("none"),
        failure.unwrap_or("none"),
    );
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

fn agents_root(state_home: &Path) -> PathBuf {
    if let Some(root) = std::env::var_os("ASP_AGENTS_HOME") {
        return root.into();
    }
    state_home.join("agents")
}
