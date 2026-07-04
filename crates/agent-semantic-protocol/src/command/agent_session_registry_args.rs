use std::{env, path::PathBuf};

pub(super) fn agent_usage() -> &'static str {
    "usage: asp agent <session> ..."
}

pub(super) fn session_usage() -> &'static str {
    "usage: asp agent session <register|list|show|reuse|status|lifecycle audit|resume|fork|archive|close|gc|reconcile|delete|unarchive|switch-model> [--guide] [--state-root PATH] [--name NAME] [--child-session-id ID] [--root-session-id ID] [--parent-session-id ID] [--role ROLE] [--model MODEL] [--status STATUS] [--expires-at UNIX_TS] [--artifact-stale-after-seconds N] [--active] [--replace] [--force] [--json] [CODEX_SESSION_ARGS...]"
}

#[derive(Clone, Copy)]
pub(super) enum SessionCommand {
    Register,
    List,
    Show,
    Reuse,
    Status,
    LifecycleAudit,
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
    pub(super) json: bool,
    pub(super) codex_args: Vec<String>,
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
            json: false,
            codex_args: Vec::new(),
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
                "register" | "add" | "upsert" if index == 0 => {
                    parsed.command = SessionCommand::Register;
                }
                "list" | "ls" if index == 0 => parsed.command = SessionCommand::List,
                "show" | "get" if index == 0 => parsed.command = SessionCommand::Show,
                "reuse" if index == 0 => parsed.command = SessionCommand::Reuse,
                "status" if index == 0 => parsed.command = SessionCommand::Status,
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
                    index += 1;
                    parsed.role = Some(non_empty_flag(args, index, "--role")?.to_string());
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
                "--all" => parsed.all = true,
                "--active" => parsed.active = true,
                "--replace" => parsed.replace = true,
                "--force"
                    if matches!(parsed.command, SessionCommand::Delete | SessionCommand::Gc) =>
                {
                    parsed.force = true;
                }
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
        SessionCommand::Register => guide.register.as_deref(),
        SessionCommand::List => guide.list.as_deref(),
        SessionCommand::Show => guide.show.as_deref(),
        SessionCommand::Reuse => guide.reuse.as_deref(),
        SessionCommand::Status => guide.status.as_deref(),
        SessionCommand::LifecycleAudit => Some(
            "asp agent session lifecycle audit guide\n\
Read-only lifecycle audit for the current root session.\n\
Combines ASP registry rows with Codex rollout session/activity evidence without creating, closing, or deleting sessions.\n\
asp agent session lifecycle audit --json",
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
Action step flow for saved-session resume:\n\
1. Shell action: resolve an already registered child or pass an explicit saved session id.\n\
   asp agent session status --name <resident-name> --json\n\
2. Shell action: resume that existing saved session.\n\
   asp agent session resume --name <resident-name>\n\
This does not create a resident ASP child session.\n\
If no configured resident child is registered, use bootstrap flow instead:\n\
asp agent session register --guide",
        ),
        SessionCommand::Fork => Some(
            "asp agent session fork guide\n\
Action step flow for saved-session fork:\n\
1. Shell action: resolve an already registered child or pass an explicit saved session id.\n\
   asp agent session status --name <resident-name> --json\n\
2. Shell action: fork that existing saved session.\n\
   asp agent session fork --name <resident-name>\n\
This does not create a resident ASP child session.\n\
If no configured resident child is registered, do not use fork as bootstrap.\n\
Use bootstrap flow instead:\n\
asp agent session register --guide",
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
Update the active platform model mapping after a capacity warning or explicit model switch.\n\
For Codex sessions this writes ~/.agent-semantic-protocols/agents/config.toml and updates ASP-owned Codex agent projections.\n\
asp agent session switch-model --model <model-id> --json",
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
        || guide_text_for(guide, SessionCommand::Reuse).is_some()
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

    load_agent_session_guide_from_str(
        &agent_semantic_config::default_hook_client_config_template_for_source_extensions([".rs"]),
    )
    .unwrap_or_else(default_agent_session_guide)
}

fn default_agent_session_guide() -> agent_semantic_config::HookClientAgentSessionGuideConfig {
    agent_semantic_config::HookClientAgentSessionGuideConfig {
        register: Some(
            "asp agent session register guide\n\
Guide template failed to load. Run `asp sync` or install hooks, then rerun `asp agent session register --guide`."
                .to_string(),
        ),
        list: Some(
            "asp agent session list guide\n\
List registered child sessions for the current root session."
                .to_string(),
        ),
        show: Some(
            "asp agent session show guide\n\
Show one registered child session by --name or --child-session-id."
                .to_string(),
        ),
        reuse: Some(
            "asp agent session reuse guide\n\
Guide template failed to load. Run `asp agent session register --guide` for bootstrap steps."
                .to_string(),
        ),
        status: Some(
            "asp agent session status guide\n\
Guide template failed to load. Run `asp agent session register --guide` when nextAction=start-resident-child-and-register."
                .to_string(),
        ),
    }
}

fn render_agent_session_guide(
    mut guide: agent_semantic_config::HookClientAgentSessionGuideConfig,
) -> agent_semantic_config::HookClientAgentSessionGuideConfig {
    let host = agent_host_guide();
    for text in [
        &mut guide.register,
        &mut guide.list,
        &mut guide.show,
        &mut guide.reuse,
        &mut guide.status,
    ] {
        if let Some(value) = text {
            *value = render_agent_session_guide_text(value, &host);
        }
    }
    guide
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
            create_action: "Codex action: start the configured subagent `asp_explorer`",
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
