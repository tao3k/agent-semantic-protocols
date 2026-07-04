//! Agent session routing owns resident child lifecycle decisions for hook-time `asp` commands.

use crate::command::{has_current_agent_session, record_current_session_tool_event};
use agent_semantic_client_db::AgentSessionRecord;
use agent_semantic_config::{
    HookClientAgentSessionMessagesConfig, HookClientAgentsConfig, HookClientConfigFile,
    default_hook_client_config_template_for_source_extensions, load_hook_client_config_file,
};
use agent_semantic_hook::{
    DecisionKind, DecisionSubject, HOOK_DECISION_SCHEMA_ID, HOOK_DECISION_SCHEMA_VERSION,
    HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, HookDecision, HookRuntime, ReasonKind,
};
#[path = "hook_runtime_agent_session_activation_failure.rs"]
mod hook_runtime_agent_session_activation_failure;
#[path = "hook_runtime_agent_session_command.rs"]
mod hook_runtime_agent_session_command;
#[path = "hook_runtime_agent_session_rollout_topology.rs"]
mod hook_runtime_agent_session_rollout_topology;
#[path = "hook_runtime_agent_session_session_start.rs"]
mod hook_runtime_agent_session_session_start;
pub(super) use hook_runtime_agent_session_activation_failure::classify_activation_failure_main_session_asp;
use hook_runtime_agent_session_command::{
    command_contains_asp_binary, command_prefix_matches, command_prefix_matches_wrapped,
    command_prefix_tokens, command_requires_resident_child, is_asp_binary_token, shell_like_tokens,
};
use hook_runtime_agent_session_rollout_topology::{
    nested_resident_child_decision, register_required_resident_child_decision,
};
use hook_runtime_agent_session_session_start::{
    classify_session_start_bootstrap, main_session_route_context,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

struct AspExplorationCommand {
    facade: String,
    stage: Option<String>,
    language_id: Option<String>,
}

pub(super) struct AspSessionPolicy {
    enabled: bool,
    resident_child_name: String,
    resident_agent_role: String,
    resident_codex_agent_name: String,
    main_allowed_asp_command_prefixes: Vec<Vec<String>>,
    testing_enabled: bool,
    testing_resident_child_name: String,
    testing_resident_agent_role: String,
    testing_resident_codex_agent_name: String,
    testing_command_prefixes: Vec<Vec<String>>,
    messages: HookClientAgentSessionMessagesConfig,
}

impl AspSessionPolicy {
    fn enabled(&self) -> bool {
        self.enabled
    }

    fn resident_child_name(&self) -> &str {
        &self.resident_child_name
    }

    fn resident_agent_role(&self) -> &str {
        &self.resident_agent_role
    }

    fn resident_codex_agent_name(&self) -> &str {
        &self.resident_codex_agent_name
    }

    fn main_asp_command_allowed(&self, tokens: &[String], asp_index: usize) -> bool {
        self.main_allowed_asp_command_prefixes
            .iter()
            .any(|prefix| command_prefix_matches(tokens, asp_index, prefix))
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
    if config.testing_with_child.is_none() {
        config.testing_with_child = defaults.testing_with_child;
    }
    if config.testing_without_child.is_none() {
        config.testing_without_child = defaults.testing_without_child;
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
    ) -> Result<Self, String> {
        let explore_agent = configured_resident_agent(&config, "asp-command");
        let testing_agent = configured_resident_agent(&config, "testing-command");
        let main_allowed_asp_command_prefixes = explore_agent
            .into_iter()
            .flat_map(|agent| agent.main_allowed_asp_command_prefixes.iter())
            .map(|prefix| command_prefix_tokens(prefix))
            .collect::<Result<Vec<_>, _>>()?;
        let testing_command_prefixes = testing_agent
            .into_iter()
            .flat_map(|agent| agent.command_prefixes.iter())
            .map(|prefix| command_prefix_tokens(prefix))
            .collect::<Result<Vec<_>, _>>()?;
        let explore_agent = configured_resident_agent(&config, "asp-command");
        let testing_agent = configured_resident_agent(&config, "testing-command");
        Ok(Self {
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
            main_allowed_asp_command_prefixes,
            testing_enabled: testing_agent.is_some_and(|agent| agent.enabled),
            testing_resident_child_name: testing_agent
                .map(|agent| agent.name.clone())
                .unwrap_or_default(),
            testing_resident_agent_role: testing_agent
                .map(|agent| agent.role.clone())
                .unwrap_or_default(),
            testing_resident_codex_agent_name: testing_agent
                .map(configured_codex_agent_name)
                .unwrap_or_default(),
            testing_command_prefixes,
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
        Self::try_from_parts(config.agents, config.agent_session_messages)
    }
}

impl AspSessionPolicy {
    fn testing_resident_child_name(&self) -> &str {
        &self.testing_resident_child_name
    }

    fn testing_resident_agent_role(&self) -> &str {
        &self.testing_resident_agent_role
    }

    fn testing_resident_codex_agent_name(&self) -> &str {
        &self.testing_resident_codex_agent_name
    }

    fn testing_command_matches(&self, command_tokens: &[String]) -> bool {
        self.testing_enabled
            && self
                .testing_command_prefixes
                .iter()
                .any(|prefix| command_prefix_matches_wrapped(command_tokens, prefix))
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
        "session-start" => classify_session_start_bootstrap(
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
    let commands = payload_command_strings(payload);
    if commands.is_empty() {
        return Ok(None);
    }

    let context = main_session_route_context(project_root, asp_session_policy)?;
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
        if let Some(command) = first_testing_command(&commands, asp_session_policy) {
            return Ok(Some(main_session_testing_command_decision(
                platform,
                event,
                payload,
                command,
                context.active_testing_session.as_ref(),
                asp_session_policy,
            )));
        }
        return Ok(None);
    }
    if context.current_is_active_testing_child(now, asp_session_policy) {
        if first_testing_command(&commands, asp_session_policy).is_some() {
            return Ok(Some(agent_session_allow_decision(
                platform,
                event,
                payload,
                "active-resident-testing-child",
                "ASP allowed resident asp-testing child session command.",
            )));
        }
        return Ok(None);
    }
    if context.outside_agent_session() {
        return Ok(None);
    }
    if let Some(command) = first_testing_command(&commands, asp_session_policy) {
        return Ok(Some(main_session_testing_command_decision(
            platform,
            event,
            payload,
            command,
            context.active_testing_session.as_ref(),
            asp_session_policy,
        )));
    }
    if context.needs_bootstrap_for(&commands, asp_session_policy) {
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
    asp_session_policy: &AspSessionPolicy,
) -> Option<(&'a str, AspExplorationCommand)> {
    commands.iter().find_map(|command| {
        restricted_main_session_asp_command(command, asp_session_policy)
            .map(|invocation| (command.as_str(), invocation))
    })
}

fn first_testing_command<'a>(
    commands: &'a [String],
    asp_session_policy: &AspSessionPolicy,
) -> Option<&'a str> {
    commands
        .iter()
        .find(|command| asp_session_policy.testing_command_matches(&shell_like_tokens(command)))
        .map(String::as_str)
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

fn missing_resident_asp_explore_decision(
    platform: &str,
    event: &str,
    payload: &serde_json::Value,
    command: Option<&str>,
    root_session_id: Option<String>,
    asp_session_policy: &AspSessionPolicy,
) -> HookDecision {
    let resident_child_name = asp_session_policy.resident_child_name();
    let mut fields = agent_session_route_fields("start-resident-child", resident_child_name);
    append_resident_agent_fields(&mut fields, asp_session_policy);
    fields.insert(
        "agentSessionBootstrap".to_string(),
        serde_json::Value::String("session-start-reminder".to_string()),
    );
    fields.insert(
        "agentSessionBootstrapGuideCommand".to_string(),
        serde_json::Value::String("asp agent session register --guide".to_string()),
    );
    if let Some(root_session_id) = root_session_id.as_ref() {
        fields.insert(
            "rootSessionId".to_string(),
            serde_json::Value::String(root_session_id.clone()),
        );
    }
    let command_line = command
        .map(|command| format!("\nOriginal command: {command}"))
        .unwrap_or_default();
    let create_action = resident_child_create_action(platform, asp_session_policy);
    let message = render_agent_session_template(
        asp_session_policy
            .messages
            .missing_resident_explore
            .as_deref(),
        &[
            template_value("residentChildName", resident_child_name),
            template_value(
                "residentCodexAgentName",
                asp_session_policy.resident_codex_agent_name(),
            ),
            template_value("createAction", &create_action),
            template_value("rootSessionId", root_session_id.as_deref().unwrap_or("")),
            template_value("originalCommandLine", &command_line),
            template_value("command", command.unwrap_or("")),
        ],
    );
    HookDecision {
        schema_id: HOOK_DECISION_SCHEMA_ID,
        schema_version: HOOK_DECISION_SCHEMA_VERSION,
        protocol_id: HOOK_PROTOCOL_ID,
        protocol_version: HOOK_PROTOCOL_VERSION,
        platform: platform.to_string(),
        event: event.to_string(),
        decision: DecisionKind::Deny,
        reason_kind: ReasonKind::RawBroadSearch,
        language_ids: Vec::new(),
        subject: DecisionSubject {
            tool_name: string_field(payload, &["tool_name", "toolName"]),
            command: command.map(str::to_string),
            paths: Vec::new(),
        },
        routes: Vec::new(),
        message,
        fields,
    }
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
    let message = if let Some(session) = explore_session {
        format!(
            "ASP denied main-session ASP exploration (`{command_label}`). Reuse or resume the registered resident {resident_child_name} child session `{}`; do not spawn another {resident_child_name} session, and do not close it after the result. Before treating a wait timeout as failure, run `asp agent session status --name {resident_child_name} --json` and use registry plus artifact activity evidence. If host status is unavailable, resume or send follow-up to the same session id before considering replacement. Only create a replacement when the host reports the child session is deleted or unrecoverable.\nCommand: {command}",
            session.session_id
        )
    } else {
        format!(
            "ASP denied main-session ASP exploration (`{command_label}`). No registered and profile-valid {resident_child_name} child is available. If you are already inside the intended child, run `asp agent session register --name {resident_child_name} --role asp-explore`; ASP infers root/parent from the Codex rollout. Otherwise create the resident child by selecting the configured Codex agent `{}` and register the returned child id with `asp agent session register --name {resident_child_name} --child-session-id <child-session-id> --role asp-explore`. If registration fails validation, close/delete that child and create a fresh child from the configured agent. Retry only after registration succeeds.\nCommand: {command}",
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
    append_resident_agent_fields(&mut fields, asp_session_policy);
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
        reason_kind: ReasonKind::RawBroadSearch,
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
    let mut fields = agent_session_route_fields("reuse-resident-child", resident_child_name);
    append_resident_agent_fields(&mut fields, asp_session_policy);
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
        reason_kind: ReasonKind::RawBroadSearch,
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

fn main_session_testing_command_decision(
    platform: &str,
    event: &str,
    payload: &serde_json::Value,
    command: &str,
    testing_session: Option<&AgentSessionRecord>,
    asp_session_policy: &AspSessionPolicy,
) -> HookDecision {
    let resident_child_name = asp_session_policy.testing_resident_child_name();
    let mut fields = agent_session_route_fields(
        if testing_session.is_some() {
            "reuse-resident-child"
        } else {
            "start-resident-child"
        },
        resident_child_name,
    );
    fields.insert(
        "agentSessionLane".to_string(),
        serde_json::Value::String("asp-testing".to_string()),
    );
    fields.insert(
        "residentAgentRole".to_string(),
        serde_json::Value::String(asp_session_policy.testing_resident_agent_role().to_string()),
    );
    fields.insert(
        "blockedCommandClass".to_string(),
        serde_json::Value::String("test-build-command".to_string()),
    );
    let message = if let Some(session) = testing_session {
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
            asp_session_policy.messages.testing_with_child.as_deref(),
            &[
                template_value("residentChildName", resident_child_name),
                template_value("childSessionId", &session.session_id),
                template_value("rootSessionId", &session.root_session_id),
                template_value("command", command),
            ],
        )
    } else {
        render_agent_session_template(
            asp_session_policy.messages.testing_without_child.as_deref(),
            &[
                template_value("residentChildName", resident_child_name),
                template_value(
                    "residentCodexAgentName",
                    asp_session_policy.testing_resident_codex_agent_name(),
                ),
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
        reason_kind: ReasonKind::RawBroadSearch,
        language_ids: Vec::new(),
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

fn append_resident_agent_fields(
    fields: &mut BTreeMap<String, serde_json::Value>,
    asp_session_policy: &AspSessionPolicy,
) {
    fields.insert(
        "residentCodexAgentName".to_string(),
        serde_json::Value::String(asp_session_policy.resident_codex_agent_name().to_string()),
    );
}

fn resident_child_create_action(platform: &str, asp_session_policy: &AspSessionPolicy) -> String {
    match platform {
        "codex" => format!(
            "Codex action: start the configured subagent `{}`",
            asp_session_policy.resident_codex_agent_name()
        ),
        "claude" => "Claude action: start the configured subagent `asp-explorer`".to_string(),
        _ => "Host action: start the configured resident ASP explore subagent".to_string(),
    }
}

fn template_value(key: &'static str, value: impl Into<String>) -> (&'static str, String) {
    (key, value.into())
}

fn render_agent_session_template(
    template: Option<&str>,
    values: &[(&'static str, String)],
) -> String {
    let mut rendered = template
        .unwrap_or("ASP agent session routing template missing. Run `asp sync` and retry.")
        .to_string();
    for (key, value) in values {
        rendered = rendered.replace(&format!("{{{{{key}}}}}"), value);
    }
    rendered.trim().to_string()
}

fn agent_session_route_fields(
    action: &str,
    resident_child_name: &str,
) -> BTreeMap<String, serde_json::Value> {
    let resident_role = if resident_child_name == "asp-testing" {
        "asp-testing"
    } else {
        "asp-explore"
    };
    let mut fields = BTreeMap::from([
        (
            "agentSessionRoute".to_string(),
            serde_json::Value::String(resident_child_name.to_string()),
        ),
        (
            "agentSessionLifecycle".to_string(),
            serde_json::Value::String("resident".to_string()),
        ),
        (
            "agentSessionStatusCheck".to_string(),
            serde_json::Value::String("asp-session-status-command".to_string()),
        ),
        (
            "agentSessionStatusCommand".to_string(),
            serde_json::Value::String(format!(
                "asp agent session status --name {resident_child_name} --json"
            )),
        ),
        (
            "agentSessionTimeoutPolicy".to_string(),
            serde_json::Value::String("timeout-is-not-duplicate-worker-trigger".to_string()),
        ),
        (
            "agentSessionAction".to_string(),
            serde_json::Value::String(action.to_string()),
        ),
        (
            "agentSessionSpawnPolicy".to_string(),
            serde_json::Value::String("registered-profile-valid-child-only".to_string()),
        ),
        (
            "agentSessionValidationPolicy".to_string(),
            serde_json::Value::String("register-hard-validates-profile".to_string()),
        ),
        (
            "agentSessionInvalidChildAction".to_string(),
            serde_json::Value::String("close-delete-and-create-configured-child".to_string()),
        ),
        (
            "agentSessionDuplicatePolicy".to_string(),
            serde_json::Value::String(
                "one-active-resident-child-per-root-session-and-name".to_string(),
            ),
        ),
        (
            "agentSessionLookupCommand".to_string(),
            serde_json::Value::String(format!(
                "asp agent session reuse --name {resident_child_name} --json"
            )),
        ),
        (
            "agentSessionRegisterCommandTemplate".to_string(),
            serde_json::Value::String(format!(
                "asp agent session register --name {resident_child_name} --child-session-id <child-session-id> --role {resident_role}"
            )),
        ),
    ]);
    append_agent_session_recovery_action_fields(
        &mut fields,
        action,
        resident_child_name,
        resident_role,
    );
    fields
}

fn append_agent_session_recovery_action_fields(
    fields: &mut BTreeMap<String, serde_json::Value>,
    action: &str,
    resident_child_name: &str,
    resident_role: &str,
) {
    let (required_action, next_action, completion_receipt) = match action {
        "start-resident-child" => (
            format!("start-{resident_child_name}-child"),
            "run-asp-agent-session-register-guide".to_string(),
            format!("{resident_child_name}-child-registration"),
        ),
        "reuse-resident-child" => (
            format!("send-to-{resident_child_name}"),
            format!("run-asp-command-in-registered-{resident_child_name}-child"),
            format!("{resident_child_name}-child-command"),
        ),
        _ => (
            format!("query-{resident_child_name}-status"),
            "run-asp-agent-session-status".to_string(),
            format!("{resident_child_name}-status-receipt"),
        ),
    };

    fields.insert(
        "requiredAction".to_string(),
        serde_json::Value::String(required_action),
    );
    fields.insert(
        "nextAction".to_string(),
        serde_json::Value::String(next_action),
    );
    fields.insert(
        "targetAgentName".to_string(),
        serde_json::Value::String(resident_child_name.to_string()),
    );
    fields.insert(
        "targetAgentRole".to_string(),
        serde_json::Value::String(resident_role.to_string()),
    );
    fields.insert(
        "forbiddenUntilResolved".to_string(),
        serde_json::Value::String("raw-source-fallback".to_string()),
    );
    fields.insert(
        "completionReceipt".to_string(),
        serde_json::Value::String(completion_receipt),
    );
}

fn agent_session_allow_decision(
    platform: &str,
    event: &str,
    payload: &serde_json::Value,
    action: &str,
    message: &str,
) -> HookDecision {
    HookDecision {
        schema_id: HOOK_DECISION_SCHEMA_ID,
        schema_version: HOOK_DECISION_SCHEMA_VERSION,
        protocol_id: HOOK_PROTOCOL_ID,
        protocol_version: HOOK_PROTOCOL_VERSION,
        platform: platform.to_string(),
        event: event.to_string(),
        decision: DecisionKind::Allow,
        reason_kind: ReasonKind::None,
        language_ids: Vec::new(),
        subject: DecisionSubject {
            tool_name: string_field(payload, &["tool_name", "toolName"]),
            command: payload_command_strings(payload).into_iter().next(),
            paths: Vec::new(),
        },
        routes: Vec::new(),
        message: message.to_string(),
        fields: BTreeMap::from([(
            "agentSessionAction".to_string(),
            serde_json::Value::String(action.to_string()),
        )]),
    }
}

fn payload_command_strings(payload: &serde_json::Value) -> Vec<String> {
    let mut commands = Vec::new();
    collect_payload_command_strings(payload, &mut commands);
    commands.sort();
    commands.dedup();
    commands
}

fn payload_evidence_ref(payload: &serde_json::Value) -> Option<String> {
    string_field(
        payload,
        &[
            "evidenceRef",
            "evidence_ref",
            "lastEvidenceRef",
            "last_evidence_ref",
            "recoveryRef",
            "recovery_ref",
        ],
    )
}

fn collect_payload_command_strings(value: &serde_json::Value, commands: &mut Vec<String>) {
    match value {
        serde_json::Value::Array(values) => {
            for value in values {
                collect_payload_command_strings(value, commands);
            }
        }
        serde_json::Value::Object(map) => {
            for (key, value) in map {
                match (key.as_str(), value) {
                    ("command" | "cmd" | "script", serde_json::Value::String(command))
                        if !command.trim().is_empty() =>
                    {
                        commands.push(command.clone());
                    }
                    ("command" | "cmd", serde_json::Value::Array(parts)) => {
                        let command = parts
                            .iter()
                            .filter_map(serde_json::Value::as_str)
                            .collect::<Vec<_>>()
                            .join(" ");
                        if !command.trim().is_empty() {
                            commands.push(command);
                        }
                    }
                    _ => collect_payload_command_strings(value, commands),
                }
            }
        }
        _ => {}
    }
}

fn asp_exploration_command(
    command: &str,
    runtime: &HookRuntime,
    asp_session_policy: &AspSessionPolicy,
) -> Option<AspExplorationCommand> {
    let tokens = shell_like_tokens(command);
    let provider_language_ids = runtime
        .providers
        .iter()
        .map(|provider| provider.language_id.as_str())
        .collect::<BTreeSet<_>>();
    for (index, token) in tokens.iter().enumerate() {
        if !is_asp_binary_token(token) {
            continue;
        }
        if asp_session_policy.main_asp_command_allowed(&tokens, index) {
            continue;
        }
        let facade = tokens.get(index + 1)?;
        let stage = tokens.get(index + 2).cloned();
        if facade == "rg" || matches!(facade.as_str(), "query" | "search" | "pipe") {
            return Some(AspExplorationCommand {
                facade: facade.clone(),
                stage,
                language_id: None,
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
                .is_some_and(|stage| matches!(stage, "query" | "search"))
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
    asp_session_policy: &AspSessionPolicy,
) -> Option<AspExplorationCommand> {
    let tokens = shell_like_tokens(command);
    for (index, token) in tokens.iter().enumerate() {
        if !is_asp_binary_token(token)
            || asp_session_policy.main_asp_command_allowed(&tokens, index)
        {
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

fn string_field(value: &serde_json::Value, keys: &[&str]) -> Option<String> {
    match value {
        serde_json::Value::Object(map) => {
            for key in keys {
                if let Some(value) = map.get(*key).and_then(serde_json::Value::as_str) {
                    return Some(value.to_string());
                }
            }
            for value in map.values() {
                if let Some(found) = string_field(value, keys) {
                    return Some(found);
                }
            }
            None
        }
        serde_json::Value::Array(values) => {
            values.iter().find_map(|value| string_field(value, keys))
        }
        _ => None,
    }
}

fn unix_timestamp() -> Result<i64, String> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock is before unix epoch: {error}"))?;
    i64::try_from(duration.as_secs()).map_err(|error| format!("timestamp overflow: {error}"))
}
