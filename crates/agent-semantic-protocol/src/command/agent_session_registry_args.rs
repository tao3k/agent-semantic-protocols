use std::{env, path::PathBuf};

pub(super) fn agent_usage() -> &'static str {
    "usage: asp agent <session> ..."
}

pub(super) fn session_usage() -> &'static str {
    "usage: asp agent session <bootstrap|observe-host-capability|observe-host-tree|observe-host-ack|dispatch-claim|dispatch-execute|dispatch-complete|dispatch-mark-orphaned|register|list|show|status|lifecycle audit|smoke|resume|fork|archive|close|gc|reconcile|delete|unarchive|switch-model> [--guide] [--state-root PATH] [--name NAME] [--canonical-target PATH] [--dispatch-identity ID] [--command-digest DIGEST] [--command-json JSON] [--resident-bridge] [--evidence-ref REF] [--agent-type-field present|absent] [--resident-target-status present|absent|unroutable] [--schema-digest DIGEST] [--observation-ttl-seconds N] [--child-session-id ID] [--message-target-id ID] [--root-session-id ID] [--parent-session-id ID] [--roles ROLE[,ROLE...]] [--model MODEL] [--status STATUS] [--expires-at UNIX_TS] [--artifact-stale-after-seconds N] [--active] [--replace] [--force] [--activity|--heartbeat] [--json] [CODEX_SESSION_ARGS...]"
}

#[derive(Clone, Copy)]
pub(super) enum SessionCommand {
    Bootstrap,
    ObserveHostCapability,
    ObserveHostTree,
    ObserveHostAck,
    DispatchClaim,
    DispatchExecute,
    DispatchComplete,
    DispatchMarkOrphaned,
    Register,
    List,
    Show,
    Status,
    LifecycleAudit,
    Smoke,
    Resume,
    Fork,
    Archive,
    Close,
    Gc,
    Reconcile,
    Delete,
    Unarchive,
    SwitchModel,
}

pub(super) struct SessionArgs {
    pub(super) help: bool,
    pub(super) guide: bool,
    pub(super) command: SessionCommand,
    pub(super) state_root: Option<PathBuf>,
    pub(super) name: Option<String>,
    pub(super) child_session_id: Option<String>,
    pub(super) message_target_id: Option<String>,
    pub(super) root_session_id: Option<String>,
    pub(super) parent_session_id: Option<String>,
    pub(super) role: Option<String>,
    pub(super) model: Option<String>,
    pub(super) status: Option<String>,
    pub(super) metadata_json: Option<String>,
    pub(super) expires_at: Option<i64>,
    pub(super) artifact_stale_after_seconds: i64,
    pub(super) all: bool,
    pub(super) active: bool,
    pub(super) replace: bool,
    pub(super) force: bool,
    pub(super) activity: bool,
    pub(super) json: bool,
    pub(super) codex_args: Vec<String>,
    pub(super) agent_type_field: Option<String>,
    pub(super) resident_target_status: Option<String>,
    pub(super) schema_digest: Option<String>,
    pub(super) dispatch_identity: Option<String>,
    pub(super) command_digest: Option<String>,
    pub(super) command_json: Option<String>,
    pub(super) receipt_kind: Option<String>,
    pub(super) resident_bridge: bool,
    pub(super) evidence_ref: Option<String>,
    pub(super) canonical_target: Option<String>,
    pub(super) observation_ttl_seconds: i64,
}

impl SessionArgs {
    pub(super) fn parse(args: &[String]) -> Result<Self, String> {
        let mut parsed = Self {
            help: false,
            guide: false,
            command: SessionCommand::List,
            state_root: None,
            name: None,
            child_session_id: None,
            message_target_id: None,
            root_session_id: None,
            parent_session_id: None,
            role: None,
            model: None,
            status: None,
            metadata_json: None,
            expires_at: None,
            artifact_stale_after_seconds: 1800,
            all: false,
            active: false,
            replace: false,
            force: false,
            activity: false,
            json: false,
            codex_args: Vec::new(),
            agent_type_field: None,
            resident_target_status: None,
            schema_digest: None,
            dispatch_identity: None,
            command_digest: None,
            command_json: None,
            receipt_kind: None,
            resident_bridge: false,
            evidence_ref: None,
            canonical_target: None,
            observation_ttl_seconds: 300,
        };
        let mut passthrough_codex_args = false;
        let mut index = 0;
        while index < args.len() {
            let arg = &args[index];
            if passthrough_codex_args {
                parsed.codex_args.push(arg.clone());
                index += 1;
                continue;
            }
            match arg.as_str() {
                "-h" | "--help" | "help" => parsed.help = true,
                "--guide" | "guide" => parsed.guide = true,
                "bootstrap" | "loop" if index == 0 => parsed.command = SessionCommand::Bootstrap,
                "observe-host-capability" if index == 0 => {
                    parsed.command = SessionCommand::ObserveHostCapability;
                }
                "observe-host-tree" if index == 0 => {
                    parsed.command = SessionCommand::ObserveHostTree;
                }
                "observe-host-ack" if index == 0 => {
                    parsed.command = SessionCommand::ObserveHostAck;
                }
                "dispatch-claim" if index == 0 => {
                    parsed.command = SessionCommand::DispatchClaim;
                }
                "dispatch-execute" if index == 0 => {
                    parsed.command = SessionCommand::DispatchExecute;
                }
                "dispatch-complete" if index == 0 => {
                    parsed.command = SessionCommand::DispatchComplete;
                }
                "dispatch-mark-orphaned" | "dispatch-orphan" if index == 0 => {
                    parsed.command = SessionCommand::DispatchMarkOrphaned;
                }
                "register" | "add" | "upsert" if index == 0 => {
                    parsed.command = SessionCommand::Register;
                }
                "list" | "ls" if index == 0 => parsed.command = SessionCommand::List,
                "show" | "get" if index == 0 => parsed.command = SessionCommand::Show,
                "status" if index == 0 => parsed.command = SessionCommand::Status,
                "smoke" | "check" if index == 0 => parsed.command = SessionCommand::Smoke,
                "lifecycle-audit" if index == 0 => {
                    parsed.command = SessionCommand::LifecycleAudit;
                }
                "lifecycle" if index == 0 => {
                    index += 1;
                    match args.get(index).map(String::as_str) {
                        Some("audit") => parsed.command = SessionCommand::LifecycleAudit,
                        Some(other) => {
                            return Err(format!(
                                "unknown asp agent session lifecycle subcommand `{other}`"
                            ));
                        }
                        None => {
                            return Err("asp agent session lifecycle requires subcommand `audit`"
                                .to_string());
                        }
                    }
                }
                "resume" if index == 0 => parsed.command = SessionCommand::Resume,
                "fork" if index == 0 => parsed.command = SessionCommand::Fork,
                "archive" if index == 0 => parsed.command = SessionCommand::Archive,
                "close" if index == 0 => parsed.command = SessionCommand::Close,
                "gc" if index == 0 => parsed.command = SessionCommand::Gc,
                "reconcile" if index == 0 => parsed.command = SessionCommand::Reconcile,
                "delete" if index == 0 => parsed.command = SessionCommand::Delete,
                "unarchive" if index == 0 => parsed.command = SessionCommand::Unarchive,
                "switch-model" | "switch" if index == 0 => {
                    parsed.command = SessionCommand::SwitchModel
                }
                "--" if is_codex_wrapper_command(parsed.command) => {
                    passthrough_codex_args = true;
                }
                "--state-root" => {
                    index += 1;
                    parsed.state_root = Some(PathBuf::from(required_flag_value(
                        args,
                        index,
                        "--state-root",
                    )?));
                }
                "--name" => {
                    index += 1;
                    parsed.name = Some(non_empty_flag(args, index, "--name")?.to_string());
                }
                "--child-session-id" | "--child-session" => {
                    index += 1;
                    parsed.child_session_id =
                        Some(non_empty_flag(args, index, arg.as_str())?.to_string());
                }
                "--message-target-id"
                | "--message-agent-target-id"
                | "--agent-message-target-id" => {
                    index += 1;
                    parsed.message_target_id =
                        Some(non_empty_flag(args, index, arg.as_str())?.to_string());
                }
                "--root-session-id" | "--root" => {
                    index += 1;
                    parsed.root_session_id =
                        Some(non_empty_flag(args, index, arg.as_str())?.to_string());
                }
                "--parent-session-id" | "--parent" => {
                    index += 1;
                    parsed.parent_session_id =
                        Some(non_empty_flag(args, index, arg.as_str())?.to_string());
                }
                "--role" => {
                    return Err(
                        "`--role` is no longer supported; use `--roles subagent,search`"
                            .to_string(),
                    );
                }
                "--roles" => {
                    index += 1;
                    parsed.role =
                        Some(parse_roles_flag(non_empty_flag(args, index, "--roles")?)?.join(","));
                }
                "--model" => {
                    index += 1;
                    parsed.model = Some(non_empty_flag(args, index, "--model")?.to_string());
                }
                "--status" => {
                    index += 1;
                    parsed.status = Some(non_empty_flag(args, index, "--status")?.to_string());
                }
                "--metadata-json" => {
                    index += 1;
                    parsed.metadata_json =
                        Some(non_empty_flag(args, index, "--metadata-json")?.to_string());
                }
                "--expires-at" => {
                    index += 1;
                    let value = non_empty_flag(args, index, "--expires-at")?;
                    parsed.expires_at = Some(value.parse::<i64>().map_err(|error| {
                        format!("--expires-at requires a unix timestamp integer: {error}")
                    })?);
                }
                "--artifact-stale-after-seconds" => {
                    index += 1;
                    let value = non_empty_flag(args, index, "--artifact-stale-after-seconds")?;
                    parsed.artifact_stale_after_seconds =
                        value.parse::<i64>().map_err(|error| {
                            format!("--artifact-stale-after-seconds requires an integer: {error}")
                        })?;
                    if parsed.artifact_stale_after_seconds < 0 {
                        return Err(
                            "--artifact-stale-after-seconds must be non-negative".to_string()
                        );
                    }
                }
                "--agent-type-field" => {
                    index += 1;
                    parsed.agent_type_field =
                        Some(non_empty_flag(args, index, "--agent-type-field")?.to_string());
                }
                "--resident-target-status" => {
                    index += 1;
                    parsed.resident_target_status =
                        Some(non_empty_flag(args, index, "--resident-target-status")?.to_string());
                }
                "--schema-digest" => {
                    index += 1;
                    parsed.schema_digest =
                        Some(non_empty_flag(args, index, "--schema-digest")?.to_string());
                }
                "--dispatch-identity" => {
                    index += 1;
                    parsed.dispatch_identity =
                        Some(non_empty_flag(args, index, "--dispatch-identity")?.to_string());
                }
                "--command-digest" => {
                    index += 1;
                    parsed.command_digest =
                        Some(non_empty_flag(args, index, "--command-digest")?.to_string());
                }
                "--command-json" => {
                    index += 1;
                    parsed.command_json =
                        Some(non_empty_flag(args, index, "--command-json")?.to_string());
                }
                "--receipt-kind" => {
                    index += 1;
                    parsed.receipt_kind =
                        Some(non_empty_flag(args, index, "--receipt-kind")?.to_string());
                }
                "--resident-bridge" => parsed.resident_bridge = true,
                "--evidence-ref" => {
                    index += 1;
                    parsed.evidence_ref =
                        Some(non_empty_flag(args, index, "--evidence-ref")?.to_string());
                }
                "--canonical-target" => {
                    index += 1;
                    parsed.canonical_target =
                        Some(non_empty_flag(args, index, "--canonical-target")?.to_string());
                }
                "--observation-ttl-seconds" => {
                    index += 1;
                    let value = non_empty_flag(args, index, "--observation-ttl-seconds")?;
                    parsed.observation_ttl_seconds = value.parse::<i64>().map_err(|error| {
                        format!("--observation-ttl-seconds requires an integer: {error}")
                    })?;
                    if !(1..=3600).contains(&parsed.observation_ttl_seconds) {
                        return Err(
                            "--observation-ttl-seconds must be between 1 and 3600".to_string()
                        );
                    }
                }
                "--all" => parsed.all = true,
                "--active" => parsed.active = true,
                "--replace" => parsed.replace = true,
                "--force"
                    if matches!(parsed.command, SessionCommand::Delete | SessionCommand::Gc) =>
                {
                    parsed.force = true;
                }
                "--activity" | "--heartbeat" => parsed.activity = true,
                "--json" => parsed.json = true,
                _ if is_codex_wrapper_command(parsed.command) => {
                    parsed.codex_args.push(arg.clone());
                }
                _ if arg.starts_with('-') => return Err(format!("unknown session flag `{arg}`")),
                _ => return Err(format!("unknown session subcommand `{arg}`")),
            }
            index += 1;
        }
        Ok(parsed)
    }
}

pub(super) fn is_codex_wrapper_command(command: SessionCommand) -> bool {
    matches!(
        command,
        SessionCommand::Resume
            | SessionCommand::Fork
            | SessionCommand::Archive
            | SessionCommand::Delete
            | SessionCommand::Unarchive
    )
}

pub(super) fn session_guide(command: SessionCommand) -> Result<String, String> {
    let guide = render_agent_session_guide(load_agent_session_guide());
    guide_text_for(&guide, command)
        .map(str::to_string)
        .ok_or_else(|| "agent session guide is not configured in hooks/config.toml".to_string())
}

fn guide_text_for(
    guide: &agent_semantic_config::HookClientAgentSessionGuideConfig,
    command: SessionCommand,
) -> Option<&str> {
    match command {
        SessionCommand::Bootstrap => Some(
            "asp agent session bootstrap guide\n\
Run the resident ASP child lifecycle loop as a structured menu. The loop prints state and choices only; the agent chooses a menu option, performs the platform-native action, then reruns bootstrap until state=Ready.\n\
asp agent session bootstrap --name <residentChildName-from-hook-decision>",
        ),
        SessionCommand::ObserveHostCapability => Some(
            "asp agent session observe-host-capability guide\n\
Record a short-lived observation of the native collaboration.spawn_agent schema for the active CODEX_THREAD_ID.\n\
This receipt is diagnostic only: it does not register a resident child and does not authorize fallback.\n\
asp agent session observe-host-capability --name asp-explore --agent-type-field present|absent",
        ),
        SessionCommand::ObserveHostTree => Some(
            "asp agent session observe-host-tree guide\n\
Record a short-lived observation of the canonical resident target in the native collaboration.list_agents tree for the active CODEX_THREAD_ID.\n\
This receipt never accepts a child id and does not register a resident child.\n\
For present targets, pass the exact canonical path selected by the hook; the lane name is never used to infer it.\n\
asp agent session observe-host-tree --name <resident-lane> --resident-target-status present --canonical-target /root/<agent>\n\
asp agent session observe-host-tree --name <resident-lane> --resident-target-status absent",
        ),
        SessionCommand::ObserveHostAck => Some(
            "asp agent session observe-host-ack guide\n\
Record a short-lived acknowledgement that a host-native follow-up or dispatch to the canonical resident target succeeded in the active CODEX_THREAD_ID.\n\
This receipt never accepts a child id, never creates a resident, and only refreshes an existing same-root resident binding.\n\
Pass the exact canonical path used by the host-native follow-up; the lane name is never used to infer it.\n\
asp agent session observe-host-ack --name <resident-lane> --canonical-target /root/<agent> [--evidence-ref <dispatch-or-followup-id>]",
        ),
        SessionCommand::DispatchClaim => Some(
            "asp agent session dispatch-claim guide\n\
Atomically claim or poll one exact resident command. Only action=send authorizes a native follow-up; action=wait polls the existing attempt and action=complete forbids replay.\n\
Derive the stable identity from the verified canonical target, receipt kind, and exact argv. Explicit identity/digest values are accepted only when they match the derived values.\n\
asp agent session dispatch-claim --name <resident-lane> --receipt-kind <kind> --command-json '<argv-json>'",
        ),
        SessionCommand::DispatchExecute => Some(
            "asp agent session dispatch-execute guide\n\
Execute one previously claimed exact argv and atomically record its terminal receipt. Root execution is allowed only through --resident-bridge bound to the fresh verified canonical target.\n\
asp agent session dispatch-execute --name <resident-lane> --receipt-kind <kind> --command-json '<argv-json>' --resident-bridge",
        ),
        SessionCommand::DispatchComplete => Some(
            "asp agent session dispatch-complete guide\n\
Record one terminal compact receipt for an existing dispatch identity. Repeated completion is idempotent and permanently disables replay.\n\
asp agent session dispatch-complete --name asp-explore --dispatch-identity <id> --command-digest <digest> --evidence-ref <ref>",
        ),
        SessionCommand::DispatchMarkOrphaned => Some(
            "asp agent session dispatch-mark-orphaned guide\n\
Mark an in-flight resident dispatch as orphaned-awaiting-rebind after the host proves the delivery target disappeared before a terminal receipt was recorded. This does not replay the command; the next verified generation may claim the same dispatch identity once.\n\
asp agent session dispatch-mark-orphaned --name <resident-lane> --dispatch-identity <id> --command-digest <digest>",
        ),
        SessionCommand::Register => guide.register(),
        SessionCommand::List => guide.list(),
        SessionCommand::Show => guide.show(),
        SessionCommand::Status => guide.status(),
        SessionCommand::LifecycleAudit => Some(
            "asp agent session lifecycle audit guide\n\
Read-only lifecycle audit for the current root session.\n\
Combines ASP registry rows with Codex rollout session/activity evidence without creating, closing, or deleting sessions.\n\
asp agent session lifecycle audit --json",
        ),
        SessionCommand::Smoke => Some(
            "asp agent session smoke guide\n\
Run one-step lifecycle smoke checks in temporary ASP/Codex state.\n\
The default scenario verifies invalid resident children enter cleanup/bootstrap without invoking a restricted search command from the shell hook.\n\
asp agent session smoke --json",
        ),
        SessionCommand::Close => Some(
            "asp agent session close guide\n\
Archive one registered session by --name or --child-session-id.\n\
asp agent session close --name <resident-name>",
        ),
        SessionCommand::Gc => Some(
            "asp agent session gc guide\n\
Delete archived, closed, expired, or invalid sessions. Use --force to delete matched sessions regardless of status.\n\
asp agent session gc --name <resident-name> --force",
        ),
        SessionCommand::Reconcile => Some(
            "asp agent session reconcile guide\n\
Refresh expired registry entries and report lifecycle cleanup candidates.\n\
asp agent session reconcile --json",
        ),
        SessionCommand::Resume => Some(
            "asp agent session resume guide\n\
Resume is a saved-session operation, not the resident-child bootstrap workflow.\n\
This does not create a resident ASP child session.\n\
If no configured resident child is registered, use bootstrap flow instead:\n\
asp agent session bootstrap --name asp-explore",
        ),
        SessionCommand::Fork => Some(
            "asp agent session fork guide\n\
Fork is a saved-session operation, not the resident-child bootstrap workflow.\n\
This does not create a resident ASP child session.\n\
If no configured resident child is registered, do not use fork as bootstrap.\n\
Use bootstrap flow instead:\n\
asp agent session bootstrap --name <residentChildName-from-hook-decision>",
        ),
        SessionCommand::Archive => Some(
            "asp agent session archive guide\n\
Wrap Codex saved-session archive.\n\
Use an explicit session id, or resolve a registered child by --name/--child-session-id.\n\
asp agent session archive --name <resident-name>",
        ),
        SessionCommand::Delete => Some(
            "asp agent session delete guide\n\
Wrap Codex saved-session delete.\n\
Use --force for non-interactive UUID deletion.\n\
asp agent session delete --name <resident-name> --force",
        ),
        SessionCommand::Unarchive => Some(
            "asp agent session unarchive guide\n\
Wrap Codex saved-session unarchive.\n\
Use an explicit session id, or resolve a registered child by --name/--child-session-id.\n\
asp agent session unarchive --name <resident-name>",
        ),
        SessionCommand::SwitchModel => Some(
            "asp agent session switch-model guide\n\
Configuration-layer child-session model switch only. This updates the expected model for ASP-owned subagent/child-session config and Codex agent projections; it never switches the main session model.\n\
Use --name to switch one resident subagent child-session config, or omit --name to update all ASP-owned Codex subagent projections.\n\
For a live resumed child mismatch, keep the main session model unchanged; the main agent must send a native message-agent follow-up to that existing child session and ask it to switch/confirm the configured child-session model.\n\
asp agent session switch-model --name asp-explore --model <model-id>",
        ),
    }
    .filter(|value| !value.trim().is_empty())
}

fn agent_session_guide_has_any_text(
    guide: &agent_semantic_config::HookClientAgentSessionGuideConfig,
) -> bool {
    guide_text_for(guide, SessionCommand::Register).is_some()
        || guide_text_for(guide, SessionCommand::List).is_some()
        || guide_text_for(guide, SessionCommand::Show).is_some()
        || guide_text_for(guide, SessionCommand::Status).is_some()
}

fn load_agent_session_guide() -> agent_semantic_config::HookClientAgentSessionGuideConfig {
    let cwd = env::current_dir().ok();
    let mut paths = Vec::new();
    if let Some(cwd) = cwd.as_deref() {
        paths.push(
            cwd.join(".codex")
                .join("agent-semantic-protocol")
                .join("hooks")
                .join("config.toml"),
        );
    }
    if let Some(state_home) = env::var_os("ASP_STATE_HOME") {
        paths.push(PathBuf::from(state_home).join("hooks").join("config.toml"));
    }
    if let Some(home) = env::var_os("HOME") {
        paths.push(
            PathBuf::from(home)
                .join(".agent-semantic-protocols")
                .join("hooks")
                .join("config.toml"),
        );
    }

    for path in paths {
        if let Some(guide) = load_agent_session_guide_from_path(&path) {
            return guide;
        }
    }

    load_agent_session_guide_from_str(&agent_semantic_config::default_hook_client_config_template())
        .unwrap_or_else(default_agent_session_guide)
}

fn default_agent_session_guide() -> agent_semantic_config::HookClientAgentSessionGuideConfig {
    agent_semantic_config::HookClientAgentSessionGuideConfig::new(
        Some(
            "asp agent session register guide\n\
Guide template failed to load. Run `asp sync` or install hooks, then enter `asp agent session bootstrap --name asp-explore`."
                .to_string(),
        ),
        Some(
            "asp agent session list guide\n\
List registered child sessions for the current root session."
                .to_string(),
        ),
        Some(
            "asp agent session show guide\n\
Show one registered child session by --name or --child-session-id."
                .to_string(),
        ),
        Some(
            "asp agent session reuse guide\n\
Guide template failed to load. The legacy reuse guide is removed; enter `asp agent session bootstrap --name asp-explore`."
                .to_string(),
        ),
        Some(
            "asp agent session status guide\n\
Guide template failed to load. Enter `asp agent session bootstrap --name asp-explore` when the resident child is missing or non-routable."
                .to_string(),
        ),
    )
}

fn render_agent_session_guide(
    mut guide: agent_semantic_config::HookClientAgentSessionGuideConfig,
) -> agent_semantic_config::HookClientAgentSessionGuideConfig {
    let host = agent_host_guide();
    if let Some(value) = guide.register_mut() {
        *value = canonical_agent_session_register_guide(&host);
    }
    if let Some(value) = guide.list_mut() {
        *value = render_agent_session_guide_text(value, &host);
    }
    if let Some(value) = guide.show_mut() {
        *value = render_agent_session_guide_text(value, &host);
    }
    if let Some(value) = guide.status_mut() {
        *value = render_agent_session_guide_text(value, &host);
    }
    guide
}

fn canonical_agent_session_register_guide(host: &AgentHostGuide) -> String {
    format!(
        "asp agent session register guide\n\
Register is a low-level state write owned by the resident-child interactive loop.\n\
Detected host: {host_label}\n\
Session env: {session_env}\n\
Canonical loop entry:\n\
   asp agent session bootstrap --name asp-explore\n\
After a hook deny, run only the loop entry. Choose exactly one number, perform the platform-native action for that choice, then re-enter the same loop until state=Ready.\n\
The pane owns audit, recovery, cleanup, creation, model alignment, and durable registration. Only run register when a pane choice explicitly asks for it and provides both childSessionId and agentMessageTargetId.\n\
Configured resident child action: {create_action}\n\
Config source: {config_source}\n\
Host projection: {host_projection}\n\
Do not run low-level session commands, thread reads, rollout scans, or generic subagent creation as independent fallback paths.",
        host_label = host.host_label,
        session_env = host.session_env,
        create_action = host.create_action,
        config_source = host.config_source,
        host_projection = host.host_projection
    )
}

fn render_agent_session_guide_text(text: &str, host: &AgentHostGuide) -> String {
    text.replace("{{hostLabel}}", host.host_label)
        .replace("{{sessionEnv}}", host.session_env)
        .replace("{{createAction}}", host.create_action)
        .replace("{{configSource}}", host.config_source)
        .replace("{{hostProjection}}", host.host_projection)
}

struct AgentHostGuide {
    host_label: &'static str,
    session_env: &'static str,
    create_action: &'static str,
    config_source: &'static str,
    host_projection: &'static str,
}

fn agent_host_guide() -> AgentHostGuide {
    if env::var_os("CODEX_THREAD_ID").is_some() {
        return AgentHostGuide {
            host_label: "codex",
            session_env: "CODEX_THREAD_ID",
            create_action: "Codex action: start or resume the configured ASP managed subagent `asp_explorer`; use a resident search-lane seed; do not ask the child to fork, create, or register another session; the parent owns registration and must capture the native agentMessageTargetId returned by the host",
            config_source: "~/.agent-semantic-protocols/agents/asp-explorer_codex.toml",
            host_projection: "~/.codex/agents/asp-explorer.toml",
        };
    }
    for env_name in [
        "CLAUDE_CODE_SESSION_ID",
        "CLAUDECODE_SESSION_ID",
        "CLAUDE_SESSION_ID",
    ] {
        if env::var_os(env_name).is_some() {
            return AgentHostGuide {
                host_label: "claude",
                session_env: env_name,
                create_action: "Claude action: start the configured subagent `asp-explorer`",
                config_source: "~/.agent-semantic-protocols/agents/asp-explorer_claude.md",
                host_projection: "~/.claude/agents/asp-explorer.md",
            };
        }
    }
    for env_name in ["AGENT_SESSION_ID", "SESSION_ID"] {
        if env::var_os(env_name).is_some() {
            return AgentHostGuide {
                host_label: "generic-agent",
                session_env: env_name,
                create_action: "Host action: start the configured resident ASP explore subagent",
                config_source: "~/.agent-semantic-protocols/agents/",
                host_projection: "host agent config directory",
            };
        }
    }
    AgentHostGuide {
        host_label: "none",
        session_env: "not detected",
        create_action: "Host action: start the configured resident ASP explore subagent only after entering a supported agent session",
        config_source: "~/.agent-semantic-protocols/agents/",
        host_projection: "host agent config directory",
    }
}

fn load_agent_session_guide_from_path(
    path: &std::path::Path,
) -> Option<agent_semantic_config::HookClientAgentSessionGuideConfig> {
    let config = agent_semantic_config::load_hook_client_config_file(path).ok()?;
    agent_session_guide_has_any_text(&config.agent_session_guide)
        .then_some(config.agent_session_guide)
}

fn load_agent_session_guide_from_str(
    content: &str,
) -> Option<agent_semantic_config::HookClientAgentSessionGuideConfig> {
    let parsed: agent_semantic_config::HookClientConfigFile = toml::from_str(content).ok()?;
    agent_session_guide_has_any_text(&parsed.agent_session_guide)
        .then_some(parsed.agent_session_guide)
}

fn parse_roles_flag(value: &str) -> Result<Vec<String>, String> {
    let mut roles = Vec::new();
    for role in value
        .split(',')
        .map(str::trim)
        .filter(|role| !role.is_empty())
    {
        match role {
            "subagent" | "search" | "testing" | "build" | "checkpoint" => {
                if !roles.iter().any(|existing| existing == role) {
                    roles.push(role.to_string());
                }
            }
            other => {
                return Err(format!(
                    "unknown session role `{other}`; expected one of subagent, search, testing, build, checkpoint"
                ));
            }
        }
    }
    if roles.is_empty() {
        return Err("--roles requires at least one schema role".to_string());
    }
    Ok(roles)
}

fn required_flag_value<'a>(
    args: &'a [String],
    index: usize,
    flag: &str,
) -> Result<&'a str, String> {
    args.get(index)
        .map(String::as_str)
        .ok_or_else(|| format!("{flag} requires a value"))
}

fn non_empty_flag<'a>(args: &'a [String], index: usize, flag: &str) -> Result<&'a str, String> {
    let value = required_flag_value(args, index, flag)?.trim();
    if value.is_empty() {
        Err(format!("{flag} must not be empty"))
    } else {
        Ok(value)
    }
}
