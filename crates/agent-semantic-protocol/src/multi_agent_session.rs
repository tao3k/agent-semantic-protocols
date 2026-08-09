//! Runtime- and Org-backed Multi-Agent Lifecycle ChoicePlane.

use std::path::{Path, PathBuf};

use agent_semantic_client_db::workspace_db_ipc::{
    AgentHostExecutionObservationIpc, AgentHostLifecycleEventIpc, AgentHostLifecycleEventKind,
    AgentSessionRegistryIpcResult,
};
use agent_semantic_client_db::{
    AgentSessionControlPlaneState, AgentSessionRegistry, SessionControlPlaneAgentRegistration,
    SessionControlPlaneDelegationProposal,
};
use agent_semantic_config::agent_route_registry::{
    AgentsRegistry, CompiledAgentRoute, compile_agent_route, load_agent_route_registry,
};
use agent_semantic_config::load_hook_client_config_file;
use agent_semantic_context_product::agent_session_delegation_admission::{
    AgentSessionDelegationCapability, AgentSessionDelegationDecision,
};
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

pub(crate) async fn open_choice_plane(request: ChoicePlaneRequest<'_>) -> Result<String, String> {
    let hook_route = latest_hook_session_agent_route(request.project_root)?.ok_or_else(|| {
                "hook-deny-session-route-required: asp session --agents choice-plane requires a current config-selected denied Hook event"
                    .to_owned()
            })?;
    let state = agent_semantic_runtime::state_core::ResolvedState::resolve(request.project_root)
        .map_err(|error| format!("failed to resolve session ChoicePlane state: {error}"))?;
    let loaded = load_agent_route_registry(&agents_root(&state.state_home).join("config.toml"))?;
    let (route, resolved_hook_route) =
        compile_hook_selected_route(&loaded, &hook_route, request.platform, &state.state_home)?;
    let sandbox_mode = route.sandbox_mode.as_deref().ok_or_else(|| {
        format!(
            "agent-route-sandbox-mode-required: registry route `{}` does not declare sandbox_mode",
            route.route_key.as_str(),
        )
    })?;
    let interactive_contract = AgentInteractiveChoice::from_source(
        CONTROL_PLANE_CONTRACT_SOURCE,
        CONTROL_PLANE_CONTRACT_FILE,
        "presentation",
    )?;
    let runtime_observation_started = tokio::time::Instant::now();
    let mut context = match crate::server::runtime_server::await_agent_facing_runtime_server_client(
        runtime_observation_started,
        "session-choice-plane",
        "runtime-session-state",
        request.project_root,
        async {
            crate::server::runtime_server::observe_agent_facing_runtime_server(&state.state_home)
                .await?;
            reconcile_pending_hook_memory_events(request.project_root).await?;
            Ok(SessionRegistryContext::resolve(
                request.project_root,
                None,
                Some(hook_route.root_session_id.clone()),
                route.platform_host_agent_name.as_str(),
            )
            .await)
        },
    )
    .await
    {
        Ok(context) => context,
        Err(error) => SessionRegistryContext::blocked(error),
    };
    context.enforce_exact_binding(&route, sandbox_mode, &hook_route.root_session_id)?;
    let choices = interactive_contract.admit_matching(&[
        ("SESSION_STATE", context.state()),
        (
            "REGISTERED_AGENT_NAME",
            route.platform_host_agent_name.as_str(),
        ),
        ("ROLE_DESCRIPTION", route.description.as_str()),
        ("SANDBOX_MODE", sandbox_mode),
    ])?;
    render_session_pane(
        request.json,
        &interactive_contract,
        &resolved_hook_route,
        &route,
        sandbox_mode,
        &context,
        &choices,
    )
}

async fn reconcile_pending_hook_memory_events(project_root: &Path) -> Result<(), String> {
    let pending =
        crate::command::hook_runtime_memory_inbox::pending_hook_events(project_root).await?;
    if pending.is_empty() {
        return Ok(());
    }
    let runtime_registry = AgentSessionRegistry::open_runtime_project_proxy(project_root)?
        .ok_or_else(|| "Runtime Server agent-session registry proxy is unavailable".to_owned())?;
    let state = agent_semantic_runtime::state_core::ResolvedState::resolve(project_root)?;
    let routes = load_agent_route_registry(&agents_root(&state.state_home).join("config.toml"))?;
    for record in pending {
        match record.entry_kind.as_str() {
            "host-lifecycle" => {
                let event = serde_json::from_value::<AgentHostLifecycleEventIpc>(record.event)
                    .map_err(|error| {
                        format!("failed to decode pending Host lifecycle event: {error}")
                    })?;
                let route = compile_agent_route(&routes, &event.route_key, "codex")?;
                if event.kind == AgentHostLifecycleEventKind::Started {
                    if event.parent_session_id == event.root_session_id {
                        runtime_registry
                            .register_control_plane_agent(SessionControlPlaneAgentRegistration {
                                project_id: event.project_id.clone(),
                                root_session_id: event.root_session_id.clone(),
                                session_id: event.root_session_id.clone(),
                                parent_session_id: None,
                                resident_name: "codex-root".to_owned(),
                                capability: AgentSessionDelegationCapability::Standard,
                            })
                            .await?;
                    }
                    let snapshot = runtime_registry
                        .read_control_plane_snapshot(
                            event.project_id.clone(),
                            event.root_session_id.clone(),
                        )
                        .await?;
                    let proposed_child_capability = match route.focus_mode {
                        agent_semantic_config::agent_route_registry::AgentFocusMode::Standard => {
                            AgentSessionDelegationCapability::Standard
                        }
                        agent_semantic_config::agent_route_registry::AgentFocusMode::Leaf => {
                            AgentSessionDelegationCapability::FocusedLeaf
                        }
                    };
                    let transaction = runtime_registry
                        .admit_control_plane_delegation(SessionControlPlaneDelegationProposal {
                            event_id: event.host_event_id.clone(),
                            project_id: event.project_id.clone(),
                            root_session_id: event.root_session_id.clone(),
                            current_session_id: event.parent_session_id.clone(),
                            proposed_child_session_id: event.child_session_id.clone(),
                            proposed_child_resident_name: event.route_key.clone(),
                            proposed_child_capability,
                            expected_generation: snapshot.generation,
                            evidence_refs: vec![
                                event.payload_digest.clone(),
                                event.profile_digest.clone(),
                                format!("route:{}", event.route_key),
                            ],
                            observed_at_ms: event.observed_at.saturating_mul(1_000),
                        })
                        .await?;
                    if transaction.admission.decision == AgentSessionDelegationDecision::Denied {
                        return Err(transaction
                            .admission
                            .reason_kind
                            .unwrap_or_else(|| "focused-agent-delegation-denied".to_owned()));
                    }
                }
                let kind = event.kind;
                let result = runtime_registry
                    .record_host_lifecycle_event_async(event)
                    .await?;
                match (kind, result) {
                    (
                        AgentHostLifecycleEventKind::Started,
                        AgentSessionRegistryIpcResult::Registered { .. },
                    )
                    | (
                        AgentHostLifecycleEventKind::Resumed
                        | AgentHostLifecycleEventKind::Stopped
                        | AgentHostLifecycleEventKind::Achieved,
                        AgentSessionRegistryIpcResult::Changed { .. },
                    ) => {}
                    _ => {
                        return Err(
                            "Runtime Server returned an unexpected Host lifecycle result"
                                .to_owned(),
                        );
                    }
                }
            }
            "host-execution-observation" => {
                let observation =
                    serde_json::from_value::<AgentHostExecutionObservationIpc>(record.event)
                        .map_err(|error| {
                            format!("failed to decode pending Host execution observation: {error}")
                        })?;
                match runtime_registry
                    .record_host_execution_observation_async(observation)
                    .await?
                {
                    AgentSessionRegistryIpcResult::Changed { .. } => {}
                    _ => {
                        return Err(
                            "Runtime Server returned an unexpected Host execution result"
                                .to_owned(),
                        );
                    }
                }
            }
            "workspace-mutation" => {
                let mutation_id = record
                    .event
                    .get("mutationId")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| "pending workspace mutation omitted mutationId".to_owned())?
                    .to_owned();
                let changed_paths = record
                    .event
                    .get("changedPaths")
                    .and_then(serde_json::Value::as_array)
                    .ok_or_else(|| "pending workspace mutation omitted changedPaths".to_owned())?
                    .iter()
                    .map(|value| {
                        value.as_str().map(str::to_owned).ok_or_else(|| {
                            "pending workspace mutation contains a non-string path".to_owned()
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                crate::server::runtime_server_hook_mutation::submit(
                    project_root,
                    mutation_id,
                    changed_paths,
                )
                .await?
                .validate()?;
            }
            "runtime-performance-observation" => {
                crate::server::runtime_server_hook_mutation::submit_pending_wall_failure(
                    record.event,
                    project_root,
                )
                .await?;
            }
            other => {
                return Err(format!("unsupported Hook memory inbox entry kind {other}"));
            }
        }
        crate::command::hook_runtime_memory_inbox::acknowledge_hook_event(
            project_root,
            record.inbox_sequence,
        )
        .await?;
    }
    Ok(())
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
    fn blocked(error: String) -> Self {
        Self::Blocked {
            reason_kind: typed_runtime_failure_reason(
                &error,
                "runtime-server-session-control-plane-unavailable",
            ),
            failure: error,
        }
    }

    async fn resolve(
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
        )
        .await
        {
            Ok(state) => Self::Ready(state),
            Err(error) => Self::blocked(error),
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

    fn binding(&self) -> Option<&serde_json::Value> {
        match self {
            Self::Ready(state) if state.state == "registered" => state.host_binding.as_ref(),
            Self::Ready(_) | Self::Blocked { .. } => None,
        }
    }

    fn enforce_exact_binding(
        &mut self,
        route: &CompiledAgentRoute,
        sandbox_mode: &str,
        expected_root_session_id: &str,
    ) -> Result<(), String> {
        let Self::Ready(state) = self else {
            return Ok(());
        };
        if state.state != "registered" {
            return Ok(());
        }
        let binding = state.host_binding.as_ref().ok_or_else(|| {
            "unbound-matched-child: registered pane state has no Host binding receipt".to_owned()
        })?;
        let profile = std::fs::read(&route.profile_path).map_err(|error| {
            format!(
                "host-binding-profile-unavailable: failed to read {}: {error}",
                route.profile_path
            )
        })?;
        let profile_digest = format!("blake3-256:{}", blake3::hash(&profile).to_hex());
        let model = route.model.as_deref().ok_or_else(|| {
            "host-binding-profile-model-required: selected route has no model".to_owned()
        })?;
        let model_digest = format!("blake3-256:{}", blake3::hash(model.as_bytes()).to_hex());
        let exact = [
            ("rootSessionId", expected_root_session_id),
            ("residentId", route.platform_host_agent_name.as_str()),
            ("routeKey", route.route_key.as_str()),
            ("profileId", route.profile_path.as_str()),
            ("profileDigest", profile_digest.as_str()),
            ("modelId", model),
            ("modelDigest", model_digest.as_str()),
            ("sandboxMode", sandbox_mode),
            ("sessionLifetime", route.session_lifetime.as_str()),
        ]
        .into_iter()
        .all(|(field, expected)| {
            binding.get(field).and_then(serde_json::Value::as_str) == Some(expected)
        }) && binding
            .get("generation")
            .and_then(serde_json::Value::as_u64)
            == Some(state.generation)
            && binding
                .get("lifecycleState")
                .and_then(serde_json::Value::as_str)
                == Some("live")
            && binding.get("routable").and_then(serde_json::Value::as_bool) == Some(true);
        if !exact {
            state.state = "registration-required".to_owned();
            state.reason_kind = Some("host-binding-route-profile-drift".to_owned());
            state.host_binding = None;
        }
        Ok(())
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
    sandbox_mode: &str,
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
            "residentId": route.platform_host_agent_name.as_str(),
            "matchDecision": "matched",
            "platform": route.platform.as_str(),
            "hostAgentName": route.platform_host_agent_name.as_str(),
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
