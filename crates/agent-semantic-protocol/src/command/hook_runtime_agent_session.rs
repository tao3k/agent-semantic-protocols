//! Agent session routing owns resident child lifecycle decisions for hook-time `asp` commands.

use crate::command::{has_current_agent_session, record_current_session_tool_event};
use agent_semantic_client_db::AgentSessionRecord;
use agent_semantic_config::{
    HookClientAgentSessionMessagesConfig, HookClientAgentsConfig,
    HookClientAspCommandIntentPolicyConfig, HookClientConfigFile, HookClientExecutionLanesConfig,
    HookClientExecutionTransport, default_hook_client_config_template_for_source_extensions,
    load_hook_client_config_file,
};
use agent_semantic_hook::{
    DecisionKind, DecisionSubject, HOOK_DECISION_SCHEMA_ID, HOOK_DECISION_SCHEMA_VERSION,
    HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, HookDecision, HookRuntime, ReasonKind,
    classify_asp_language_command_tokens_with_policy,
};
#[path = "hook_runtime_agent_session_activation_failure.rs"]
mod hook_runtime_agent_session_activation_failure;
#[path = "hook_runtime_agent_session_command.rs"]
mod hook_runtime_agent_session_command;
#[path = "hook_runtime_agent_session_execution_lane.rs"]
mod hook_runtime_agent_session_execution_lane;
#[path = "hook_runtime_agent_session_identity.rs"]
mod hook_runtime_agent_session_identity;
#[path = "hook_runtime_agent_session_inline_fallback.rs"]
mod hook_runtime_agent_session_inline_fallback;
#[path = "hook_runtime_agent_session_pane.rs"]
mod hook_runtime_agent_session_pane;
#[path = "hook_runtime_agent_session_payload.rs"]
mod hook_runtime_agent_session_payload;
#[path = "hook_runtime_agent_session_presence.rs"]
mod hook_runtime_agent_session_presence;
#[path = "hook_runtime_agent_session_profile.rs"]
mod hook_runtime_agent_session_profile;
#[path = "hook_runtime_agent_session_rollout_topology.rs"]
mod hook_runtime_agent_session_rollout_topology;
#[path = "hook_runtime_agent_session_session_start.rs"]
mod hook_runtime_agent_session_session_start;
#[path = "hook_runtime_agent_session_spawn.rs"]
mod hook_runtime_agent_session_spawn;
#[path = "hook_runtime_agent_session_terminal.rs"]
mod hook_runtime_agent_session_terminal;
#[path = "hook_runtime_agent_session_testing.rs"]
mod hook_runtime_agent_session_testing;
#[path = "hook_runtime_agent_session_typed_replacement.rs"]
mod hook_runtime_agent_session_typed_replacement;
pub(super) use hook_runtime_agent_session_activation_failure::classify_activation_failure_main_session_asp;
use hook_runtime_agent_session_command::{
    command_contains_asp_binary, command_prefix_matches, command_prefix_matches_wrapped,
    command_prefix_tokens, command_requires_resident_child, shell_like_tokens,
};
use hook_runtime_agent_session_execution_lane::ResidentExecutionLane;
use hook_runtime_agent_session_inline_fallback::missing_resident_decision;
use hook_runtime_agent_session_pane::{
    agent_session_allow_decision, agent_session_route_fields,
    append_agent_session_recovery_action_fields, render_agent_session_template,
};
use hook_runtime_agent_session_payload::{
    payload_command_strings, payload_evidence_ref, string_field,
};
use hook_runtime_agent_session_profile::{
    append_resident_agent_fields, resident_agent_host_action, resident_child_create_action,
};
use hook_runtime_agent_session_rollout_topology::{
    nested_resident_child_decision, register_required_resident_child_decision,
};
use hook_runtime_agent_session_session_start::{
    classify_session_start_bootstrap, current_session_configured_resident_identity_proof,
    main_session_route_context,
};
pub(super) use hook_runtime_agent_session_session_start::{
    current_session_resident_child_identity_proof, session_matches_resident_agent,
};
use hook_runtime_agent_session_spawn::resident_spawn_context_decision;
use hook_runtime_agent_session_terminal::{
    append_terminal_execution_fields, proven_resident_parser_command_is_terminal,
    resident_dispatch_wrapper_is_terminal,
};
use hook_runtime_agent_session_testing::resident_execution_decision;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

struct AspExplorationCommand {
    facade: String,
    stage: Option<String>,
    language_id: Option<String>,
}

pub(super) struct AspSessionPolicy {
    command_intent_policy: HookClientAspCommandIntentPolicyConfig,
    enabled: bool,
    resident_child_name: String,
    resident_agent_role: String,
    resident_codex_agent_name: String,
    resident_route_source: &'static str,
    main_allowed_asp_command_prefixes: Vec<Vec<String>>,
    execution_lanes: Vec<ResidentExecutionLane>,
    messages: HookClientAgentSessionMessagesConfig,
}

impl AspSessionPolicy {
    fn command_intent_policy(&self) -> &HookClientAspCommandIntentPolicyConfig {
        &self.command_intent_policy
    }

    fn enabled(&self) -> bool {
        self.enabled
    }

    pub(super) fn resident_child_name(&self) -> &str {
        &self.resident_child_name
    }

    pub(super) fn resident_agent_role(&self) -> &str {
        &self.resident_agent_role
    }

    pub(super) fn resident_codex_agent_name(&self) -> &str {
        &self.resident_codex_agent_name
    }

    pub(super) fn uses_builtin_resident_fallback(&self) -> bool {
        self.resident_route_source == "built-in-fallback"
    }

    fn main_asp_command_allowed(&self, tokens: &[String], asp_index: usize) -> bool {
        match hook_runtime_agent_session_command::classify_main_session_asp_command(
            tokens,
            asp_index,
            &self.command_intent_policy,
        ) {
            hook_runtime_agent_session_command::MainSessionAspCommandClass::ControlPlane
            | hook_runtime_agent_session_command::MainSessionAspCommandClass::ExactEvidenceRead
            | hook_runtime_agent_session_command::MainSessionAspCommandClass::DirectReadFallback
            | hook_runtime_agent_session_command::MainSessionAspCommandClass::InvalidEvidence => {
                true
            }
            hook_runtime_agent_session_command::MainSessionAspCommandClass::ReasoningFlow => false,
            hook_runtime_agent_session_command::MainSessionAspCommandClass::Unknown => self
                .main_allowed_asp_command_prefixes
                .iter()
                .any(|prefix| command_prefix_matches(tokens, asp_index, prefix)),
        }
    }
}

pub(super) fn load_asp_session_policy(config_path: &Path) -> Result<AspSessionPolicy, String> {
    let default_messages = default_hook_client_config_file()?.agent_session_messages;
    let mut config = if config_path.is_file() {
        load_hook_client_config_file(config_path)?
    } else {
        default_hook_client_config_file()?
    };
    config.agent_session_messages =
        merge_agent_session_messages(config.agent_session_messages, default_messages);
    AspSessionPolicy::try_from(config)
}

pub(super) fn default_asp_session_policy() -> Result<AspSessionPolicy, String> {
    let mut config = default_hook_client_config_file()?;
    let default_messages = default_hook_client_config_file()?.agent_session_messages;
    config.agent_session_messages =
        merge_agent_session_messages(config.agent_session_messages, default_messages);
    AspSessionPolicy::try_from(config)
}

fn default_hook_client_config_file() -> Result<HookClientConfigFile, String> {
    toml::from_str(&default_hook_client_config_template_for_source_extensions(
        [".rs"],
    ))
    .map_err(|error| format!("failed to parse default hook client config template: {error}"))
}

fn merge_agent_session_messages(
    mut config: HookClientAgentSessionMessagesConfig,
    defaults: HookClientAgentSessionMessagesConfig,
) -> HookClientAgentSessionMessagesConfig {
    if config.session_start_reuse.is_none() {
        config.session_start_reuse = defaults.session_start_reuse;
    }
    if config.session_start_bootstrap.is_none() {
        config.session_start_bootstrap = defaults.session_start_bootstrap;
    }
    if config.missing_resident_explore.is_none() {
        config.missing_resident_explore = defaults.missing_resident_explore;
    }
    if config.main_restricted_with_child.is_none() {
        config.main_restricted_with_child = defaults.main_restricted_with_child;
    }
    if config.main_restricted_without_child.is_none() {
        config.main_restricted_without_child = defaults.main_restricted_without_child;
    }
    if config.binary_gate_with_child.is_none() {
        config.binary_gate_with_child = defaults.binary_gate_with_child;
    }
    if config.binary_gate_without_child.is_none() {
        config.binary_gate_without_child = defaults.binary_gate_without_child;
    }
    if config.binary_gate_invalid_child.is_none() {
        config.binary_gate_invalid_child = defaults.binary_gate_invalid_child;
    }
    if config.binary_gate_registry_blocked.is_none() {
        config.binary_gate_registry_blocked = defaults.binary_gate_registry_blocked;
    }
    if config.source_access_compact.is_none() {
        config.source_access_compact = defaults.source_access_compact;
    }
    if config.source_access_compact_repeated.is_none() {
        config.source_access_compact_repeated = defaults.source_access_compact_repeated;
    }
    if config.source_access_compact_subagent.is_none() {
        config.source_access_compact_subagent = defaults.source_access_compact_subagent;
    }
    config
}

impl AspSessionPolicy {
    fn try_from_parts(
        config: HookClientAgentsConfig,
        messages: HookClientAgentSessionMessagesConfig,
        command_intent_policy: HookClientAspCommandIntentPolicyConfig,
        execution_lanes: HookClientExecutionLanesConfig,
    ) -> Result<Self, String> {
        let default_agents = HookClientAgentsConfig::default();
        let configured_explore_agent = configured_resident_agent(&config, "asp-command");
        let explore_agent = configured_explore_agent
            .or_else(|| configured_resident_agent(&default_agents, "asp-command"));
        let main_allowed_asp_command_prefixes = explore_agent
            .into_iter()
            .flat_map(|agent| agent.main_allowed_asp_command_prefixes.iter())
            .map(|prefix| command_prefix_tokens(prefix))
            .collect::<Result<Vec<_>, _>>()?;
        let resident_agents = config
            .resident_agents
            .iter()
            .filter(|agent| agent.enabled)
            .map(|agent| (agent.name.as_str(), agent))
            .collect::<std::collections::HashMap<_, _>>();
        let resident_execution_lanes = execution_lanes
            .lanes
            .into_iter()
            .filter(|(_, lane)| lane.enabled)
            .map(|(lane_name, lane)| {
                let resident_agent = resident_agents.get(lane.resident_name.as_str()).copied();
                let command_prefixes = lane
                    .command_prefixes
                    .iter()
                    .map(|prefix| command_prefix_tokens(prefix))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(ResidentExecutionLane {
                    name: lane_name,
                    transport: lane.transport,
                    resident_child_name: lane.resident_name,
                    resident_agent_role: resident_agent
                        .map(|agent| agent.role.clone())
                        .unwrap_or_default(),
                    resident_codex_agent_name: resident_agent
                        .map(configured_codex_agent_name)
                        .unwrap_or_default(),
                    command_prefixes,
                    receipt_kind: lane.receipt_kind,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Self {
            command_intent_policy,
            enabled: explore_agent.is_some_and(|agent| agent.enabled),
            resident_child_name: explore_agent
                .map(|agent| agent.name.clone())
                .unwrap_or_default(),
            resident_agent_role: explore_agent
                .map(|agent| agent.role.clone())
                .unwrap_or_default(),
            resident_codex_agent_name: explore_agent
                .map(configured_codex_agent_name)
                .unwrap_or_default(),
            resident_route_source: if configured_explore_agent.is_some() {
                "hook-config"
            } else {
                "built-in-fallback"
            },
            main_allowed_asp_command_prefixes,
            execution_lanes: resident_execution_lanes,
            messages,
        })
    }
}

fn configured_resident_agent<'a>(
    config: &'a HookClientAgentsConfig,
    lifecycle: &str,
) -> Option<&'a agent_semantic_config::HookClientResidentAgentConfig> {
    config
        .resident_agents
        .iter()
        .find(|agent| agent.lifecycle == lifecycle)
}

fn configured_codex_agent_name(
    agent: &agent_semantic_config::HookClientResidentAgentConfig,
) -> String {
    if agent.codex_agent_name.is_empty() {
        agent.role.clone()
    } else {
        agent.codex_agent_name.clone()
    }
}

impl TryFrom<HookClientConfigFile> for AspSessionPolicy {
    type Error = String;

    fn try_from(config: HookClientConfigFile) -> Result<Self, Self::Error> {
        Self::try_from_parts(
            config.agents,
            config.agent_session_messages,
            config.asp_command_intent_policy,
            config.execution_lanes,
        )
    }
}

pub(super) fn classify_main_session_asp_exploration(
    project_root: &Path,
    platform: &str,
    event: &str,
    runtime: &HookRuntime,
    asp_session_policy: &AspSessionPolicy,
    payload: &serde_json::Value,
) -> Result<Option<HookDecision>, String> {
    if !asp_session_policy.enabled() {
        return Ok(None);
    }
    match event {
        "session-start" | "subagent-start" => classify_session_start_bootstrap(
            project_root,
            platform,
            event,
            payload,
            asp_session_policy,
        ),
        "post-tool" => {
            record_post_tool_session_event(project_root, event, payload)?;
            if has_current_agent_session() {
                Ok(Some(agent_session_allow_decision(
                    platform,
                    event,
                    payload,
                    "post-tool-session-event-recorded",
                    "ASP recorded post-tool session activity.",
                )))
            } else {
                Ok(None)
            }
        }
        "pre-tool" => classify_pre_tool_main_session_asp(
            project_root,
            platform,
            event,
            runtime,
            payload,
            asp_session_policy,
        ),
        _ => Ok(None),
    }
}

fn classify_pre_tool_main_session_asp(
    project_root: &Path,
    platform: &str,
    event: &str,
    runtime: &HookRuntime,
    payload: &serde_json::Value,
    asp_session_policy: &AspSessionPolicy,
) -> Result<Option<HookDecision>, String> {
    if let Some(decision) =
        resident_spawn_context_decision(platform, event, payload, asp_session_policy)
    {
        return Ok(Some(decision));
    }
    let commands = payload_command_strings(payload);
    if commands.is_empty() {
        return Ok(None);
    }

    let resident_identity_proven =
        current_session_resident_child_identity_proof(project_root, asp_session_policy, payload)?
            .is_some();
    if resident_dispatch_wrapper_is_terminal(&commands) {
        return Ok(Some(agent_session_allow_decision(
            platform,
            event,
            payload,
            "resident-command-bridge",
            "ASP allowed the hook-selected resident command wrapper to validate and consume one exact dispatch lease.",
        )));
    }
    if proven_resident_parser_command_is_terminal(&commands, resident_identity_proven) {
        let mut decision = agent_session_allow_decision(
            platform,
            event,
            payload,
            "active-resident-child",
            "ASP allowed the proven resident child to execute the parser command as a terminal dispatch; self-routing is forbidden.",
        );
        decision.fields.insert(
            "executionLane".to_string(),
            serde_json::Value::String(asp_session_policy.resident_child_name().to_string()),
        );
        return Ok(Some(decision));
    }
    if let Some((_, lane)) = first_resident_execution_command(&commands, asp_session_policy)
        && current_session_configured_resident_identity_proof(
            project_root,
            payload,
            lane.resident_child_name(),
            lane.resident_agent_role(),
            lane.resident_codex_agent_name(),
        )?
        .is_some()
    {
        let mut decision = agent_session_allow_decision(
            platform,
            event,
            payload,
            "active-hook-selected-resident",
            "ASP allowed the host-proven hook-selected resident to execute its configured lane as a routing terminal.",
        );
        decision.fields.extend([
            (
                "executionLane".to_string(),
                serde_json::Value::String(lane.name().to_string()),
            ),
            (
                "executionTransport".to_string(),
                serde_json::Value::String("resident-child-terminal".to_string()),
            ),
            (
                "residentChildName".to_string(),
                serde_json::Value::String(lane.resident_child_name().to_string()),
            ),
            (
                "targetAgentName".to_string(),
                serde_json::Value::String(lane.resident_codex_agent_name().to_string()),
            ),
            (
                "canonicalTarget".to_string(),
                serde_json::Value::String(format!("/root/{}", lane.resident_codex_agent_name())),
            ),
        ]);
        return Ok(Some(decision));
    }

    let context = main_session_route_context(project_root, asp_session_policy, payload)?;
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
        if let Some((command, lane)) =
            first_resident_execution_command(&commands, asp_session_policy)
        {
            return Ok(Some(main_session_resident_execution_decision(
                platform, event, payload, command, lane,
            )));
        }
        return Ok(None);
    }
    if context.outside_agent_session() {
        return Ok(None);
    }
    if let Some((command, lane)) = first_resident_execution_command(&commands, asp_session_policy) {
        return Ok(Some(main_session_resident_execution_decision(
            platform, event, payload, command, lane,
        )));
    }
    if context.active_explore_session.is_none()
        && first_asp_exploration_command(&commands, runtime, asp_session_policy).is_some()
    {
        if let Some(topology) = context.current_nested_resident_child(asp_session_policy) {
            return Ok(Some(nested_resident_child_decision(
                platform,
                event,
                payload,
                topology,
                asp_session_policy,
            )));
        }
        if let Some(topology) = context.current_register_required_resident_child(asp_session_policy)
        {
            return Ok(Some(register_required_resident_child_decision(
                platform,
                event,
                payload,
                topology,
                asp_session_policy,
            )));
        }
        return Ok(Some(missing_resident_decision(
            platform,
            event,
            payload,
            commands.first().map(String::as_str),
            context.root_session_id,
            asp_session_policy,
        )));
    }

    if let Some((command, invocation)) =
        first_asp_exploration_command(&commands, runtime, asp_session_policy)
    {
        return Ok(Some(main_session_asp_exploration_decision(
            platform,
            event,
            payload,
            command,
            invocation,
            context.active_explore_session.as_ref(),
            asp_session_policy,
        )));
    }
    if let Some((command, invocation)) =
        first_restricted_main_session_asp_command(&commands, Some(runtime), asp_session_policy)
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

fn first_asp_exploration_command<'a>(
    commands: &'a [String],
    runtime: &HookRuntime,
    asp_session_policy: &AspSessionPolicy,
) -> Option<(&'a str, AspExplorationCommand)> {
    commands.iter().find_map(|command| {
        asp_exploration_command(command, runtime, asp_session_policy)
            .map(|invocation| (command.as_str(), invocation))
    })
}

fn first_restricted_main_session_asp_command<'a>(
    commands: &'a [String],
    runtime: Option<&HookRuntime>,
    asp_session_policy: &AspSessionPolicy,
) -> Option<(&'a str, AspExplorationCommand)> {
    commands.iter().find_map(|command| {
        restricted_main_session_asp_command(command, runtime, asp_session_policy)
            .map(|invocation| (command.as_str(), invocation))
    })
}

fn first_resident_execution_command<'a>(
    commands: &'a [String],
    asp_session_policy: &'a AspSessionPolicy,
) -> Option<(&'a str, &'a ResidentExecutionLane)> {
    commands.iter().find_map(|command| {
        let tokens = shell_like_tokens(command);
        asp_session_policy
            .execution_lanes
            .iter()
            .filter_map(|lane| {
                lane.matching_prefix_len(&tokens)
                    .map(|length| (length, lane))
            })
            .max_by_key(|(length, _)| *length)
            .map(|(_, lane)| (command.as_str(), lane))
    })
}

fn record_post_tool_session_event(
    project_root: &Path,
    event: &str,
    payload: &serde_json::Value,
) -> Result<(), String> {
    let command = payload_command_strings(payload).into_iter().next();
    let evidence_ref = payload_evidence_ref(payload);
    record_current_session_tool_event(
        project_root,
        event,
        command.as_deref(),
        evidence_ref.as_deref(),
    )?;
    Ok(())
}

fn main_session_asp_exploration_decision(
    platform: &str,
    event: &str,
    payload: &serde_json::Value,
    command: &str,
    invocation: AspExplorationCommand,
    explore_session: Option<&AgentSessionRecord>,
    asp_session_policy: &AspSessionPolicy,
) -> HookDecision {
    let resident_child_name = asp_session_policy.resident_child_name();
    let command_label = match invocation.stage.as_deref() {
        Some(stage) => format!("asp {} {stage}", invocation.facade),
        None => format!("asp {}", invocation.facade),
    };
    let host_action =
        resident_agent_host_action(platform, asp_session_policy, explore_session.is_some());
    let message = if let Some(session) = explore_session {
        format!(
            "ASP denied main-session ASP exploration (`{command_label}`). Use the resident-child interactive pane for registered resident {resident_child_name} child session `{}`. {host_action} Run `asp agent session bootstrap --name {resident_child_name}`, choose one number, perform that native platform action, and re-enter the pane until state=Ready.\nCommand: {command}",
            session.session_id
        )
    } else {
        format!(
            "ASP denied main-session ASP exploration (`{command_label}`). No registered and profile-valid {resident_child_name} child is available. Run the resident-child interactive pane: `asp agent session bootstrap --name {resident_child_name}`. {host_action} Choose one number, perform that native platform action, and re-enter the same pane until state=Ready. The pane owns audit, recovery, cleanup, creation, model alignment, and registration for configured agent `{}`.\nCommand: {command}",
            asp_session_policy.resident_codex_agent_name()
        )
    };
    let mut fields = agent_session_route_fields(
        if explore_session.is_some() {
            "reuse-resident-child"
        } else {
            "start-resident-child"
        },
        resident_child_name,
    );
    append_resident_agent_fields(&mut fields, platform, asp_session_policy);
    append_asp_command_intent_fields(&mut fields, command, asp_session_policy);
    fields.insert(
        "blockedAspFacade".to_string(),
        serde_json::Value::String(invocation.facade.clone()),
    );
    if let Some(stage) = invocation.stage.as_ref() {
        fields.insert(
            "blockedAspStage".to_string(),
            serde_json::Value::String(stage.clone()),
        );
    }
    if let Some(session) = explore_session {
        fields.insert(
            "rootSessionId".to_string(),
            serde_json::Value::String(session.root_session_id.clone()),
        );
        fields.insert(
            "childSessionId".to_string(),
            serde_json::Value::String(session.session_id.clone()),
        );
        fields.insert(
            "agentSessionResumeId".to_string(),
            serde_json::Value::String(session.session_id.clone()),
        );
        fields.insert(
            "childSessionName".to_string(),
            serde_json::Value::String(session.name.clone()),
        );
    }
    HookDecision {
        schema_id: HOOK_DECISION_SCHEMA_ID,
        schema_version: HOOK_DECISION_SCHEMA_VERSION,
        protocol_id: HOOK_PROTOCOL_ID,
        protocol_version: HOOK_PROTOCOL_VERSION,
        platform: platform.to_string(),
        event: event.to_string(),
        decision: DecisionKind::Deny,
        reason_kind: ReasonKind::AspReasoningRouted,
        language_ids: invocation.language_id.into_iter().collect(),
        subject: DecisionSubject {
            tool_name: string_field(payload, &["tool_name", "toolName"]),
            command: Some(command.to_string()),
            paths: Vec::new(),
        },
        routes: Vec::new(),
        message,
        fields,
    }
}

fn main_session_restricted_asp_command_decision(
    platform: &str,
    event: &str,
    payload: &serde_json::Value,
    command: &str,
    invocation: AspExplorationCommand,
    explore_session: Option<&AgentSessionRecord>,
    asp_session_policy: &AspSessionPolicy,
) -> HookDecision {
    let resident_child_name = asp_session_policy.resident_child_name();
    let command_label = match invocation.stage.as_deref() {
        Some(stage) => format!("asp {} {stage}", invocation.facade),
        None => format!("asp {}", invocation.facade),
    };
    let mut fields = agent_session_route_fields("resume-resident-child", resident_child_name);
    append_resident_agent_fields(&mut fields, platform, asp_session_policy);
    append_asp_command_intent_fields(&mut fields, command, asp_session_policy);
    fields.insert(
        "mainSessionAspPolicy".to_string(),
        serde_json::Value::String("session-checkpoint-recovery-only".to_string()),
    );
    fields.insert(
        "blockedAspFacade".to_string(),
        serde_json::Value::String(invocation.facade.clone()),
    );
    if let Some(stage) = invocation.stage.as_ref() {
        fields.insert(
            "blockedAspStage".to_string(),
            serde_json::Value::String(stage.clone()),
        );
    }
    let message = if let Some(session) = explore_session {
        fields.insert(
            "rootSessionId".to_string(),
            serde_json::Value::String(session.root_session_id.clone()),
        );
        fields.insert(
            "childSessionId".to_string(),
            serde_json::Value::String(session.session_id.clone()),
        );
        fields.insert(
            "agentSessionResumeId".to_string(),
            serde_json::Value::String(session.session_id.clone()),
        );
        render_agent_session_template(
            asp_session_policy
                .messages
                .main_restricted_with_child
                .as_deref(),
            &[
                template_value("residentChildName", resident_child_name),
                template_value("childSessionId", &session.session_id),
                template_value("rootSessionId", &session.root_session_id),
                template_value("commandLabel", &command_label),
                template_value("command", command),
            ],
        )
    } else {
        let create_action = resident_child_create_action(platform, asp_session_policy);
        render_agent_session_template(
            asp_session_policy
                .messages
                .main_restricted_without_child
                .as_deref(),
            &[
                template_value("residentChildName", resident_child_name),
                template_value(
                    "residentCodexAgentName",
                    asp_session_policy.resident_codex_agent_name(),
                ),
                template_value("createAction", &create_action),
                template_value("commandLabel", &command_label),
                template_value("command", command),
            ],
        )
    };
    HookDecision {
        schema_id: HOOK_DECISION_SCHEMA_ID,
        schema_version: HOOK_DECISION_SCHEMA_VERSION,
        protocol_id: HOOK_PROTOCOL_ID,
        protocol_version: HOOK_PROTOCOL_VERSION,
        platform: platform.to_string(),
        event: event.to_string(),
        decision: DecisionKind::Deny,
        reason_kind: ReasonKind::AspReasoningRouted,
        language_ids: invocation.language_id.into_iter().collect(),
        subject: DecisionSubject {
            tool_name: string_field(payload, &["tool_name", "toolName"]),
            command: Some(command.to_string()),
            paths: Vec::new(),
        },
        routes: Vec::new(),
        message,
        fields,
    }
}

fn append_asp_command_intent_fields(
    fields: &mut BTreeMap<String, serde_json::Value>,
    command: &str,
    asp_session_policy: &AspSessionPolicy,
) {
    let tokens = agent_semantic_hook::semantic_shell_tokens(command);
    let Some(parsed) = classify_asp_language_command_tokens_with_policy(
        &tokens,
        asp_session_policy.command_intent_policy(),
    ) else {
        return;
    };
    fields.insert(
        "aspCommandIntent".to_string(),
        serde_json::Value::String(parsed.intent.as_str().to_string()),
    );
    fields.insert(
        "aspCommandRoute".to_string(),
        serde_json::Value::String(parsed.route.wire_value()),
    );
    fields.insert(
        "languageId".to_string(),
        serde_json::Value::String(parsed.language_id),
    );
    if let Some(selector) = parsed.selector {
        fields.insert("selector".to_string(), serde_json::Value::String(selector));
    }
}

fn main_session_resident_execution_decision(
    platform: &str,
    event: &str,
    payload: &serde_json::Value,
    command: &str,
    lane: &ResidentExecutionLane,
) -> HookDecision {
    resident_execution_decision(platform, event, payload, command, lane)
}

fn template_value(key: &'static str, value: impl Into<String>) -> (&'static str, String) {
    (key, value.into())
}

fn asp_exploration_command(
    command: &str,
    runtime: &HookRuntime,
    asp_session_policy: &AspSessionPolicy,
) -> Option<AspExplorationCommand> {
    let tokens = agent_semantic_hook::semantic_shell_tokens(command);
    let provider_language_ids = runtime
        .providers
        .iter()
        .map(|provider| provider.language_id.as_str())
        .collect::<BTreeSet<_>>();
    for index in agent_semantic_hook::asp_invocation_indices(&tokens) {
        if hook_runtime_agent_session_command::classify_main_session_asp_command(
            &tokens,
            index,
            asp_session_policy.command_intent_policy(),
        ) != hook_runtime_agent_session_command::MainSessionAspCommandClass::ReasoningFlow
        {
            continue;
        }
        let parsed = classify_asp_language_command_tokens_with_policy(
            &tokens[index..],
            asp_session_policy.command_intent_policy(),
        );
        if parsed
            .as_ref()
            .is_some_and(|command| !provider_language_ids.contains(command.language_id.as_str()))
        {
            continue;
        }
        let facade = tokens.get(index + 1)?;
        let stage = tokens.get(index + 2).cloned();
        if matches!(facade.as_str(), "fd" | "rg" | "query" | "search") {
            return Some(AspExplorationCommand {
                facade: facade.clone(),
                stage,
                language_id: parsed.map(|command| command.language_id),
            });
        }
        if facade == "org" {
            if stage
                .as_deref()
                .is_some_and(|stage| matches!(stage, "query" | "search"))
            {
                return Some(AspExplorationCommand {
                    facade: facade.clone(),
                    stage,
                    language_id: Some("org".to_string()),
                });
            }
            continue;
        }
        if provider_language_ids.contains(facade.as_str())
            && stage
                .as_deref()
                .is_some_and(|stage| matches!(stage, "guide" | "query" | "search"))
        {
            return Some(AspExplorationCommand {
                facade: facade.clone(),
                stage,
                language_id: Some(facade.clone()),
            });
        }
    }
    None
}

fn restricted_main_session_asp_command(
    command: &str,
    runtime: Option<&HookRuntime>,
    asp_session_policy: &AspSessionPolicy,
) -> Option<AspExplorationCommand> {
    let tokens = agent_semantic_hook::semantic_shell_tokens(command);
    let provider_language_ids = runtime.map(|runtime| {
        runtime
            .providers
            .iter()
            .map(|provider| provider.language_id.as_str())
            .collect::<std::collections::HashSet<_>>()
    });
    for index in agent_semantic_hook::asp_invocation_indices(&tokens) {
        if let (Some(provider_language_ids), Some(parsed)) = (
            provider_language_ids.as_ref(),
            classify_asp_language_command_tokens_with_policy(
                &tokens[index..],
                asp_session_policy.command_intent_policy(),
            ),
        ) && !provider_language_ids.contains(parsed.language_id.as_str())
        {
            continue;
        }
        if asp_session_policy.main_asp_command_allowed(&tokens, index) {
            continue;
        }
        let facade = tokens.get(index + 1)?.clone();
        return Some(AspExplorationCommand {
            facade,
            stage: tokens.get(index + 2).cloned(),
            language_id: None,
        });
    }
    None
}

fn unix_timestamp() -> Result<i64, String> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock is before unix epoch: {error}"))?;
    i64::try_from(duration.as_secs()).map_err(|error| format!("timestamp overflow: {error}"))
}
