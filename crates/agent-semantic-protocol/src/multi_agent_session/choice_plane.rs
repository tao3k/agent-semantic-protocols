//! Runtime- and Org-backed Multi-Agent Lifecycle ChoicePlane.

use super::registration::HostChildSessionTopology;
use std::path::{Path, PathBuf};

use agent_semantic_client_db::{AgentSessionControlPlaneState, AgentSessionRegistry};
use agent_semantic_config::agent_route_registry::{
    AgentsRegistry, CompiledAgentRoute, compile_agent_route, load_agent_route_registry,
};
use agent_semantic_config::load_hook_client_config_file;
use agent_semantic_hook::{HookSessionAgentRoute, latest_hook_session_agent_route};

use crate::command::org_capture_interactive::AgentInteractiveChoice;

pub(crate) const PANE_SCHEMA_ID: &str =
    "agent.semantic-protocols.multi-agent-session-control-plane-pane";
pub(crate) const CONTROL_PLANE_CONTRACT_ID: &str = "agent.multi-agent-session-control-plane.v1";
pub(crate) const CONTROL_PLANE_CONTRACT_FILE: &str =
    "agent.multi-agent-session-control-plane.v1.org";
pub(crate) const CONTROL_PLANE_CONTRACT_SOURCE: &str =
    include_str!("../../../../org/contracts/agent.multi-agent-session-control-plane.v1.org");

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
    let project_id = AgentSessionRegistry::workspace_id(request.project_root)?;
    let current_root_session_id = if request.platform == "codex" {
        std::env::var("CODEX_THREAD_ID")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| {
                std::env::var("CODEX_SESSION_ID")
                    .ok()
                    .filter(|value| !value.trim().is_empty())
            })
            .unwrap_or_else(|| hook_route.root_session_id.clone())
    } else {
        hook_route.root_session_id.clone()
    };
    let local_registration = read_current_child_registration(
        request.project_root,
        &project_id,
        &current_root_session_id,
        route.route_key.as_str(),
    )?;
    let mut local_registration_authority = false;
    let mut context = SessionRegistryContext::Ready(AgentSessionControlPlaneState {
        project_id: Some(project_id.clone()),
        root_session_id: Some(current_root_session_id.clone()),
        name: route.platform_host_agent_name.as_str().to_owned(),
        state: if local_registration.is_some() {
            "registered".to_owned()
        } else {
            "registration-required".to_owned()
        },
        generation: local_registration
            .as_ref()
            .map(|receipt| receipt.generation)
            .unwrap_or(0),
        reason_kind: local_registration
            .is_none()
            .then(|| "host-agent-registration-required".to_owned()),
        host_binding: local_registration
            .map(serde_json::to_value)
            .transpose()
            .map_err(|error| format!("failed to encode child registration binding: {error}"))?,
    });
    if let Some(receipt) = read_current_child_registration(
        request.project_root,
        &project_id,
        &hook_route.root_session_id,
        route.route_key.as_str(),
    )? {
        local_registration_authority = true;
        let generation = receipt.generation;
        let host_binding = serde_json::to_value(&receipt)
            .map_err(|error| format!("failed to project child registration receipt: {error}"))?;
        context = SessionRegistryContext::Ready(AgentSessionControlPlaneState {
            project_id: Some(project_id.clone()),
            root_session_id: Some(hook_route.root_session_id.clone()),
            name: route.platform_host_agent_name.as_str().to_owned(),
            state: "registered".to_owned(),
            generation,
            reason_kind: None,
            host_binding: Some(host_binding),
        });
    }
    let inbox_reconciliation_failure = None::<String>;
    context.apply_host_lifecycle_surface(true);
    let reconcile_project_root = request.project_root.to_path_buf();
    let reconcile_state_home = state.state_home.clone();
    let reconcile_root_session_id = hook_route.root_session_id.clone();
    let reconcile_agent_name = route.platform_host_agent_name.as_str().to_owned();
    tokio::spawn(async move {
        let _ = tokio::time::timeout(
            crate::server::runtime_server::RUNTIME_SERVER_SUPERVISOR_EXECUTION_BUDGET,
            async {
                crate::server::runtime_server::observe_agent_facing_runtime_server(
                    &reconcile_state_home,
                )
                .await?;
                let _ = SessionRegistryContext::resolve(
                    &reconcile_project_root,
                    None,
                    Some(reconcile_root_session_id),
                    &reconcile_agent_name,
                )
                .await;
                Ok::<(), String>(())
            },
        )
        .await;
    });
    context.enforce_exact_binding(&route, sandbox_mode, &current_root_session_id)?;
    if local_registration_authority && let SessionRegistryContext::Ready(state) = &mut context {
        state.state = "registered".to_owned();
        state.reason_kind = None;
    }
    let choices = interactive_contract.admit_matching(&[
        ("SESSION_STATE", context.state()),
        (
            "REGISTERED_AGENT_NAME",
            route.platform_host_agent_name.as_str(),
        ),
        ("ROLE_DESCRIPTION", route.description.as_str()),
        ("SANDBOX_MODE", sandbox_mode),
    ])?;
    if choices.len() != 1 {
        return Err(format!(
            "agent-session-choice-state-not-unique: SESSION_STATE={} admitted {} contract rows",
            context.state(),
            choices.len(),
        ));
    }
    render_session_pane(
        request.json,
        &interactive_contract,
        &resolved_hook_route,
        &route,
        sandbox_mode,
        &context,
        inbox_reconciliation_failure.as_deref(),
        &choices,
    )
}

const CHILD_REGISTRATION_SCHEMA_ID: &str = "agent.child-session-registration";

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ChildSessionRegistrationReceipt {
    pub(super) schema_id: String,
    pub(super) schema_version: u64,
    pub(super) project_id: String,
    pub(super) root_session_id: String,
    pub(super) parent_session_id: String,
    pub(super) child_session_id: String,
    pub(super) resident_id: String,
    pub(super) route_key: String,
    pub(super) profile_id: String,
    pub(super) profile_digest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) configured_model_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) configured_model_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) observed_model_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) observed_model_digest: Option<String>,
    pub(super) sandbox_mode: String,
    pub(super) session_lifetime: String,
    pub(super) generation: u64,
    pub(super) lifecycle_state: String,
    pub(super) routable: bool,
    pub(super) host_call_receipt_digest: String,
    pub(super) registration_authority: String,
}

fn child_registration_path(
    project_root: &std::path::Path,
    project_id: &str,
    root_session_id: &str,
    route_key: &str,
) -> Result<std::path::PathBuf, String> {
    let identity = format!("{project_id}\0{root_session_id}\0{route_key}");
    let namespace = blake3::hash(identity.as_bytes()).to_hex().to_string();
    Ok(agent_semantic_runtime::project_state_paths(project_root)?
        .hook_state_dir
        .join("host-sessions")
        .join(namespace)
        .join("child-registration.json"))
}

fn read_current_child_registration(
    project_root: &std::path::Path,
    project_id: &str,
    root_session_id: &str,
    route_key: &str,
) -> Result<Option<ChildSessionRegistrationReceipt>, String> {
    let path = child_registration_path(project_root, project_id, root_session_id, route_key)?;
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "failed to read child registration {}: {error}",
                path.display()
            ));
        }
    };
    let receipt: ChildSessionRegistrationReceipt =
        serde_json::from_slice(&bytes).map_err(|error| {
            format!(
                "failed to decode child registration {}: {error}",
                path.display()
            )
        })?;
    if receipt.schema_id != CHILD_REGISTRATION_SCHEMA_ID
        || receipt.schema_version != 1
        || receipt.project_id != project_id
        || receipt.root_session_id != root_session_id
        || receipt.route_key != route_key
        || receipt.parent_session_id.is_empty()
        || receipt.child_session_id.is_empty()
        || receipt.child_session_id == receipt.root_session_id
        || receipt.child_session_id == receipt.parent_session_id
        || receipt.generation == 0
        || receipt.lifecycle_state != "live"
        || !receipt.routable
        || !receipt.host_call_receipt_digest.starts_with("blake3-256:")
        || receipt.registration_authority != "project-child-registration-authority"
    {
        return Err(format!(
            "child-session-registration-identity-mismatch: {}",
            path.display()
        ));
    }
    Ok(Some(receipt))
}

pub(super) async fn publish_child_session_registration(
    project_root: &std::path::Path,
    platform: &str,
    topology: HostChildSessionTopology,
    metadata: agent_semantic_runtime::CodexRolloutSessionMetadata,
) -> Result<String, String> {
    if platform != "codex" {
        return Err(format!(
            "child-self-registration-host-unsupported: platform={platform}"
        ));
    }
    let HostChildSessionTopology {
        child_session_id,
        parent_session_id,
        root_session_id,
    } = topology;
    let agent_role = metadata.agent_role().ok_or_else(|| {
        "child-self-registration-host-role-required: Host call receipt has no agent role".to_owned()
    })?;
    let state = agent_semantic_runtime::state_core::ResolvedState::resolve(project_root)
        .map_err(|error| format!("failed to resolve child registration state: {error}"))?;
    let loaded = load_agent_route_registry(&agents_root(&state.state_home).join("config.toml"))?;
    let route = loaded
        .compile_route_for_platform_host_agent_name(platform, agent_role)?
        .ok_or_else(|| {
            format!(
                "child-self-registration-host-role-unregistered: platform={platform} role={agent_role}"
            )
        })?;
    let sandbox_mode = route.sandbox_mode.as_deref().ok_or_else(|| {
        "child-self-registration-sandbox-required: selected route has no sandbox".to_owned()
    })?;
    let model = route.model.as_deref().ok_or_else(|| {
        "child-self-registration-model-required: selected route has no model".to_owned()
    })?;
    let observed_model = metadata.model().map(str::to_owned);
    let profile = tokio::fs::read(&route.profile_path)
        .await
        .map_err(|error| {
            format!(
                "child-self-registration-profile-unavailable: failed to read {}: {error}",
                route.profile_path
            )
        })?;
    let project_id = AgentSessionRegistry::workspace_id(project_root)?;
    let path = child_registration_path(
        project_root,
        &project_id,
        &root_session_id,
        route.route_key.as_str(),
    )?;
    let authority_dir = path.parent().ok_or_else(|| {
        "child-self-registration-path-invalid: receipt path has no parent".to_owned()
    })?;
    if let Some(receipt) = read_current_child_registration(
        project_root,
        &project_id,
        &root_session_id,
        route.route_key.as_str(),
    )? && receipt.child_session_id == child_session_id
        && receipt.parent_session_id == parent_session_id
        && receipt.observed_model_id == observed_model
    {
        return serde_json::to_string(&receipt)
            .map_err(|error| format!("failed to encode child registration receipt: {error}"));
    }
    tokio::fs::create_dir_all(authority_dir)
        .await
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::PermissionDenied {
                format!(
                    "child-registration-authority-permission-denied: Host lifecycle publication cannot create {}",
                    authority_dir.display()
                )
            } else {
                format!("failed to create child registration authority: {error}")
            }
        })?;
    let lock_path = authority_dir.join(".registration-v2-lock");
    let mut lock_acquired = false;
    for _ in 0..100 {
        match tokio::fs::create_dir(&lock_path).await {
            Ok(()) => {
                lock_acquired = true;
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                if let Ok(metadata) = tokio::fs::metadata(&lock_path).await
                    && let Ok(modified) = metadata.modified()
                    && modified
                        .elapsed()
                        .is_ok_and(|elapsed| elapsed > std::time::Duration::from_secs(30))
                {
                    let _ = tokio::fs::remove_dir(&lock_path).await;
                    continue;
                }
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
            Err(error) => {
                return Err(format!(
                    "child-self-registration-authority-lock-failed: {}: {error}",
                    lock_path.display()
                ));
            }
        }
    }
    if !lock_acquired {
        return Err(format!(
            "child-self-registration-authority-lock-timeout: {}",
            lock_path.display()
        ));
    }
    let host_call_identity = format!(
        "{}\0{}\0{}\0{}",
        root_session_id,
        parent_session_id,
        child_session_id,
        metadata.rollout_path().display()
    );
    let publication = async {
        let existing = read_current_child_registration(
            project_root,
            &project_id,
            &root_session_id,
            route.route_key.as_str(),
        )?;
        if let Some(ref receipt) = existing
            && receipt.child_session_id == child_session_id
            && receipt.parent_session_id == parent_session_id
            && receipt.host_call_receipt_digest
                == format!(
                    "blake3-256:{}",
                    blake3::hash(host_call_identity.as_bytes()).to_hex()
                )
        {
            return serde_json::to_string(receipt)
                .map_err(|error| format!("failed to encode child registration receipt: {error}"));
        }
        let generation = existing
            .as_ref()
            .map(|receipt| receipt.generation)
            .unwrap_or(0)
            .checked_add(1)
            .ok_or_else(|| "child-self-registration-generation-exhausted".to_owned())?;
        let receipt = ChildSessionRegistrationReceipt {
            schema_id: CHILD_REGISTRATION_SCHEMA_ID.to_owned(),
            schema_version: 1,
            project_id,
            root_session_id,
            parent_session_id,
            child_session_id,
            resident_id: route.platform_host_agent_name.as_str().to_owned(),
            route_key: route.route_key.as_str().to_owned(),
            profile_id: route.profile_path.as_str().to_owned(),
            profile_digest: format!("blake3-256:{}", blake3::hash(&profile).to_hex()),
            configured_model_id: Some(model.to_owned()),
            configured_model_digest: Some(format!(
                "blake3-256:{}",
                blake3::hash(model.as_bytes()).to_hex()
            )),
            observed_model_digest: observed_model.as_deref().map(|observed| {
                format!("blake3-256:{}", blake3::hash(observed.as_bytes()).to_hex())
            }),
            observed_model_id: observed_model,
            sandbox_mode: sandbox_mode.to_owned(),
            session_lifetime: route.session_lifetime.as_str().to_owned(),
            generation,
            lifecycle_state: "live".to_owned(),
            routable: true,
            host_call_receipt_digest: format!(
                "blake3-256:{}",
                blake3::hash(host_call_identity.as_bytes()).to_hex()
            ),
            registration_authority: "project-child-registration-authority".to_owned(),
        };
        let encoded = serde_json::to_vec_pretty(&receipt)
            .map_err(|error| format!("failed to encode child registration receipt: {error}"))?;
        let temporary = authority_dir.join(format!(
            ".child-registration.{}.{}.tmp",
            std::process::id(),
            blake3::hash(host_call_identity.as_bytes()).to_hex()
        ));
        tokio::fs::write(&temporary, encoded)
            .await
            .map_err(|error| format!("failed to write child registration receipt: {error}"))?;
        if let Err(error) = tokio::fs::rename(&temporary, &path).await {
            let _ = tokio::fs::remove_file(&temporary).await;
            return Err(format!(
                "failed to publish child registration receipt: {error}"
            ));
        }
        serde_json::to_string(&receipt)
            .map_err(|error| format!("failed to encode child registration receipt: {error}"))
    }
    .await;
    let unlock = tokio::fs::remove_dir(&lock_path).await;
    match (publication, unlock) {
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(format!(
            "child-self-registration-authority-unlock-failed: {}: {error}",
            lock_path.display()
        )),
        (Ok(receipt), Ok(())) => Ok(receipt),
    }
}

pub(super) struct ResolvedHookSessionRoute {
    pub(super) config_rule_id: String,
    pub(super) target_agent_name: String,
    pub(super) target_agent_role: String,
    pub(super) receipt_kind: String,
    pub(super) command_digest: Option<String>,
    pub(super) reason_kind: String,
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
    let (route_key, _) = loaded
        .registry
        .unique_route_for_role(dispatch.role.as_str())?;
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

pub(super) enum SessionRegistryContext {
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

    pub(super) fn state(&self) -> &str {
        match self {
            Self::Ready(state) => state.state.as_str(),
            Self::Blocked { .. } => "blocked",
        }
    }

    fn apply_host_lifecycle_surface(&mut self, available: bool) {
        let Self::Ready(state) = self else {
            return;
        };
        let resolved = crate::agent_session_choice_state::resolve_agent_session_choice_state(
            state.state.as_str(),
            state.reason_kind.as_deref(),
            if available {
                crate::agent_session_choice_state::HostLifecycleSurface::Available
            } else {
                crate::agent_session_choice_state::HostLifecycleSurface::Unavailable
            },
        );
        let resolved_state = resolved.state.to_owned();
        let resolved_reason_kind = resolved.reason_kind.map(str::to_owned);
        if resolved_state != state.state {
            state.state = resolved_state;
            state.reason_kind = resolved_reason_kind;
        }
    }

    pub(super) fn generation(&self) -> u64 {
        match self {
            Self::Ready(state) => state.generation,
            Self::Blocked { .. } => 0,
        }
    }

    pub(super) fn reason_kind(&self) -> Option<&str> {
        match self {
            Self::Ready(state) => state.reason_kind.as_deref(),
            Self::Blocked { reason_kind, .. } => Some(reason_kind),
        }
    }

    pub(super) fn failure(&self) -> Option<&str> {
        match self {
            Self::Ready(_) => None,
            Self::Blocked { failure, .. } => Some(failure),
        }
    }

    pub(super) fn binding(&self) -> Option<&serde_json::Value> {
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
        let expected_binding = [
            ("rootSessionId", expected_root_session_id),
            ("residentId", route.platform_host_agent_name.as_str()),
            ("routeKey", route.route_key.as_str()),
            ("profileId", route.profile_path.as_str()),
            ("profileDigest", profile_digest.as_str()),
            ("configuredModelId", model),
            ("configuredModelDigest", model_digest.as_str()),
            ("sandboxMode", sandbox_mode),
            ("sessionLifetime", route.session_lifetime.as_str()),
        ];
        let mismatched_field = expected_binding.into_iter().find_map(|(field, expected)| {
            (binding.get(field).and_then(serde_json::Value::as_str) != Some(expected))
                .then_some(field)
        });
        let exact = mismatched_field.is_none()
            && binding
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
            state.reason_kind = Some(
                mismatched_field
                    .map(|field| format!("host-binding-{field}-drift"))
                    .unwrap_or_else(|| "host-binding-lifecycle-drift".to_owned()),
            );
            state.host_binding = None;
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

/*
fn render_session_pane(
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
        "hookInbox": hook_inbox,
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
    if let Some(inbox_failure) = inbox_reconciliation_failure {
        pane_context.push_str(&format!(
            " hookInboxState=degraded hookInboxBlocksChoicePlane=false hookInboxFailure={inbox_failure}"
        ));
    }
    if choices.len() == 1 && choices[0].presentation == "action" {
        let rendered = interactive_contract.render_admitted_action(
            CONTROL_PLANE_CONTRACT_ID,
            &choices[0],
            &pane_context,
        );
        if context.state() == "registration-required" {
            return Ok(format!(
                "{rendered}\n registration: After the Host creates the subagent, Call @{} with this instruction: Run `asp session register-current-child` from your own child session. The command reads your current childSessionId and the Host calling receipt, verifies the bound rootSessionId and parentSessionId, registers this child with ASP, and returns a nonzero generation receipt. Then continue the preserved denied command in this same child. Do not ask the parent to supply session IDs and do not create another subagent.",
                route.platform_host_agent_name.as_str(),
            ));
        }
        return Ok(rendered);
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
    let rendered = interactive_contract.render_admitted_pane(
        CONTROL_PLANE_CONTRACT_ID,
        &rendered_choices,
        &pane_context,
    );
    if context.state() == "registration-required" {
        return Ok(format!(
            "{rendered}\n register-with-asp: Call @{} with this instruction: You are now inside the Host-created child session. Run `asp session register-current-child` here. The command reads your current childSessionId and verifies the Host calling receipt that binds rootSessionId and parentSessionId; ASP returns the nonzero generation receipt. After it succeeds, continue the preserved denied command in this same child. Do not ask the parent to supply session IDs and do not create another subagent.",
            route.platform_host_agent_name.as_str(),
        ));
    }
    Ok(rendered)
}

*/
fn agents_root(state_home: &Path) -> PathBuf {
    if let Some(root) = std::env::var_os("ASP_AGENTS_HOME") {
        return root.into();
    }
    state_home.join("agents")
}
use super::pane::render_session_pane;
