use crate::command::{
    RegisteredSession, asp_explore_session_for_current_root, current_registered_session,
    current_root_session_id, has_current_agent_session, record_current_session_tool_event,
};
use agent_semantic_config::{
    HookClientAspSessionPolicyConfig, HookClientConfigFile, load_hook_client_config_file,
};
use agent_semantic_hook::{
    DecisionKind, DecisionSubject, HOOK_DECISION_SCHEMA_ID, HOOK_DECISION_SCHEMA_VERSION,
    HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, HookDecision, HookRuntime, ReasonKind,
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
    resident_codex_agent_name: String,
    main_allowed_asp_command_prefixes: Vec<Vec<String>>,
}

impl AspSessionPolicy {
    fn enabled(&self) -> bool {
        self.enabled
    }

    fn resident_child_name(&self) -> &str {
        &self.resident_child_name
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

struct MainSessionRouteContext {
    has_agent_session: bool,
    current_session: Option<RegisteredSession>,
    active_explore_session: Option<RegisteredSession>,
    root_session_id: Option<String>,
}

pub(super) fn load_asp_session_policy(config_path: &Path) -> Result<AspSessionPolicy, String> {
    let config = if config_path.is_file() {
        load_hook_client_config_file(config_path)?
    } else {
        HookClientConfigFile::default()
    };
    AspSessionPolicy::try_from(config.asp_session_policy)
}

impl TryFrom<HookClientAspSessionPolicyConfig> for AspSessionPolicy {
    type Error = String;

    fn try_from(config: HookClientAspSessionPolicyConfig) -> Result<Self, Self::Error> {
        let main_allowed_asp_command_prefixes = config
            .main_allowed_asp_command_prefixes
            .iter()
            .map(|prefix| command_prefix_tokens(prefix))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            enabled: config.enabled,
            resident_child_name: config.resident_child_name,
            resident_codex_agent_name: config.resident_codex_agent_name,
            main_allowed_asp_command_prefixes,
        })
    }
}

impl MainSessionRouteContext {
    fn current_is_active_asp_explore(&self, now: i64) -> bool {
        self.current_session
            .as_ref()
            .is_some_and(|session| session.role == "asp-explore" && session.is_routable_at(now))
    }

    fn outside_agent_session(&self) -> bool {
        !self.has_agent_session
            && self.current_session.is_none()
            && self.active_explore_session.is_none()
    }

    fn needs_bootstrap_for(
        &self,
        commands: &[String],
        asp_session_policy: &AspSessionPolicy,
    ) -> bool {
        self.has_agent_session
            && self.active_explore_session.is_none()
            && commands
                .iter()
                .any(|command| command_requires_resident_child(command, asp_session_policy))
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
            Ok(None)
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
    if context.current_is_active_asp_explore(now) || context.outside_agent_session() {
        return Ok(None);
    }
    if context.needs_bootstrap_for(&commands, asp_session_policy) {
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

fn main_session_route_context(
    project_root: &Path,
    asp_session_policy: &AspSessionPolicy,
) -> Result<MainSessionRouteContext, String> {
    let current_session = current_registered_session(project_root)?;
    let now = unix_timestamp()?;
    let active_explore_session = asp_explore_session_for_current_root(
        project_root,
        asp_session_policy.resident_child_name(),
    )?
    .filter(|session| session.role == "asp-explore" && session.is_routable_at(now));
    Ok(MainSessionRouteContext {
        has_agent_session: has_current_agent_session(),
        current_session,
        active_explore_session,
        root_session_id: current_root_session_id(),
    })
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

fn classify_session_start_bootstrap(
    project_root: &Path,
    platform: &str,
    event: &str,
    payload: &serde_json::Value,
    asp_session_policy: &AspSessionPolicy,
) -> Result<Option<HookDecision>, String> {
    if !has_current_agent_session() {
        return Ok(None);
    }
    let now = unix_timestamp()?;
    if current_registered_session(project_root)?
        .as_ref()
        .is_some_and(|session| session.role == "asp-explore" && session.is_routable_at(now))
    {
        return Ok(None);
    }
    let active_explore_session = asp_explore_session_for_current_root(
        project_root,
        asp_session_policy.resident_child_name(),
    )?
    .filter(|session| session.role == "asp-explore" && session.is_routable_at(now));
    if active_explore_session.is_some() {
        return Ok(None);
    }
    Ok(Some(session_start_bootstrap_decision(
        platform,
        event,
        payload,
        current_root_session_id(),
        asp_session_policy,
    )))
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

fn session_start_bootstrap_decision(
    platform: &str,
    event: &str,
    payload: &serde_json::Value,
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
    if let Some(root_session_id) = root_session_id.as_ref() {
        fields.insert(
            "rootSessionId".to_string(),
            serde_json::Value::String(root_session_id.clone()),
        );
    }
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
            command: None,
            paths: Vec::new(),
        },
        routes: Vec::new(),
        message: format!(
            "ASP session-start bootstrap: this root session has no registered active {resident_child_name} child session. First recall the ASP registry with `asp agent session reuse --name {resident_child_name} --json`. If no child session is returned, create the resident child by selecting the configured Codex agent `{}`; do not use an ad-hoc natural-language subagent. Then register the returned child id with `asp agent session register --name {resident_child_name} --child-session-id <child-session-id> --role asp-explore`. Do not create duplicate {resident_child_name} sessions for the same root session; if a child id already exists outside the registry, register that id instead.",
            asp_session_policy.resident_codex_agent_name()
        ),
        fields,
    }
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
    if let Some(root_session_id) = root_session_id.as_ref() {
        fields.insert(
            "rootSessionId".to_string(),
            serde_json::Value::String(root_session_id.clone()),
        );
    }
    let command_line = command
        .map(|command| format!("\nOriginal command: {command}"))
        .unwrap_or_default();
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
        message: format!(
            "ASP session-start bootstrap required. This root session has no registered active {resident_child_name} child session in the ASP session registry. First recall the ASP registry with `asp agent session reuse --name {resident_child_name} --json`. If no child session is returned, create the resident child by selecting the configured Codex agent `{}`; do not use an ad-hoc natural-language subagent. Then register it with `asp agent session register --name {resident_child_name} --child-session-id <child-session-id> --role asp-explore`. After registration, retry the original tool command; do not create duplicate {resident_child_name} sessions for the same root session. If a child id already exists outside the registry, register that id instead.{command_line}",
            asp_session_policy.resident_codex_agent_name()
        ),
        fields,
    }
}

fn main_session_asp_exploration_decision(
    platform: &str,
    event: &str,
    payload: &serde_json::Value,
    command: &str,
    invocation: AspExplorationCommand,
    explore_session: Option<&RegisteredSession>,
    asp_session_policy: &AspSessionPolicy,
) -> HookDecision {
    let resident_child_name = asp_session_policy.resident_child_name();
    let command_label = match invocation.stage.as_deref() {
        Some(stage) => format!("asp {} {stage}", invocation.facade),
        None => format!("asp {}", invocation.facade),
    };
    let message = if let Some(session) = explore_session {
        format!(
            "ASP denied main-session ASP exploration (`{command_label}`). Reuse or resume the registered resident {resident_child_name} child session `{}`; do not spawn another {resident_child_name} session, and do not close it after the result. Before treating a wait timeout as failure, query the host session status for this child session id when the runtime exposes a status API. If the host reports active/running, wait or send follow-up to the same child session. If the host status is unavailable, resume or send follow-up to the same session id before considering replacement. Only create a replacement when the host reports the child session is deleted or unrecoverable.\nCommand: {command}",
            session.session_id
        )
    } else {
        format!(
            "ASP denied main-session ASP exploration (`{command_label}`). First recall the ASP registry with `asp agent session reuse --name {resident_child_name} --json`. If no child session is returned, create the resident child by selecting the configured Codex agent `{}`; do not use an ad-hoc natural-language subagent. Then register it with `asp agent session register --name {resident_child_name} --child-session-id <child-session-id> --role asp-explore`. Reuse that child for ASP search/query for the rest of this root session; do not create duplicate {resident_child_name} sessions and do not close it while the root session is active.\nCommand: {command}",
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
    explore_session: Option<&RegisteredSession>,
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
    let child_note = if let Some(session) = explore_session {
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
        format!(
            " Route ASP search/query/evidence work to resident {resident_child_name} child session `{}`.",
            session.session_id
        )
    } else {
        format!(
            " Create or register the resident {resident_child_name} child before ASP evidence work."
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
        message: format!(
            "ASP denied main-session ASP command (`{command_label}`). Main-session ASP usage is limited to session, recovery, and checkpoint commands: `asp agent session ...`, `asp org recall ...`, and `asp org capture ...`.{child_note}\nCommand: {command}"
        ),
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

fn agent_session_route_fields(
    action: &str,
    resident_child_name: &str,
) -> BTreeMap<String, serde_json::Value> {
    BTreeMap::from([
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
            serde_json::Value::String("query-host-status-then-resume".to_string()),
        ),
        (
            "agentSessionAction".to_string(),
            serde_json::Value::String(action.to_string()),
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
                "asp agent session register --name {resident_child_name} --child-session-id <child-session-id> --role asp-explore"
            )),
        ),
    ])
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

fn is_asp_binary_token(token: &str) -> bool {
    token.rsplit('/').next() == Some("asp")
}

fn command_requires_resident_child(command: &str, asp_session_policy: &AspSessionPolicy) -> bool {
    let tokens = shell_like_tokens(command);
    tokens.iter().enumerate().any(|(index, token)| {
        is_asp_binary_token(token) && !asp_session_policy.main_asp_command_allowed(&tokens, index)
    })
}

fn command_prefix_tokens(prefix: &str) -> Result<Vec<String>, String> {
    let tokens = prefix
        .split_whitespace()
        .map(|token| token.to_ascii_lowercase())
        .collect::<Vec<_>>();
    if tokens.is_empty() {
        Err("aspSessionPolicy.mainAllowedAspCommandPrefixes[] must not be empty".to_string())
    } else {
        Ok(tokens)
    }
}

fn command_prefix_matches(tokens: &[String], asp_index: usize, prefix: &[String]) -> bool {
    let command_start = asp_index + 1;
    if tokens.len() <= command_start {
        return prefix.len() == 1 && prefix[0] == "help";
    }
    tokens.len() >= command_start + prefix.len()
        && tokens
            .iter()
            .skip(command_start)
            .zip(prefix.iter())
            .all(|(token, expected)| token.eq_ignore_ascii_case(expected))
}

fn shell_like_tokens(command: &str) -> Vec<String> {
    command
        .split_whitespace()
        .map(|token| {
            token.trim_matches(|character: char| {
                matches!(
                    character,
                    '\'' | '"' | ',' | ';' | ':' | '(' | ')' | '[' | ']' | '{' | '}'
                )
            })
        })
        .filter(|token| !token.is_empty())
        .map(str::to_string)
        .collect()
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
