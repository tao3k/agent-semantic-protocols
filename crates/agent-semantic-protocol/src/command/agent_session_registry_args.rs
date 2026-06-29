use std::{env, path::PathBuf};

pub(super) fn agent_usage() -> &'static str {
    "usage: asp agent <session> ..."
}

pub(super) fn session_usage() -> &'static str {
    "usage: asp agent session <register|list|show|reuse> [--guide] [--state-root PATH] [--name NAME] [--child-session-id ID] [--root-session-id ID] [--parent-session-id ID] [--role ROLE] [--model MODEL] [--status STATUS] [--expires-at UNIX_TS] [--active] [--replace] [--json]"
}

#[derive(Clone, Copy)]
pub(super) enum SessionCommand {
    Register,
    List,
    Show,
    Reuse,
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
    pub(super) all: bool,
    pub(super) active: bool,
    pub(super) replace: bool,
    pub(super) json: bool,
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
            all: false,
            active: false,
            replace: false,
            json: false,
        };
        let mut index = 0;
        while index < args.len() {
            let arg = &args[index];
            match arg.as_str() {
                "-h" | "--help" | "help" => parsed.help = true,
                "--guide" | "guide" => parsed.guide = true,
                "register" | "add" | "upsert" if index == 0 => {
                    parsed.command = SessionCommand::Register;
                }
                "list" | "ls" if index == 0 => parsed.command = SessionCommand::List,
                "show" | "get" if index == 0 => parsed.command = SessionCommand::Show,
                "reuse" if index == 0 => parsed.command = SessionCommand::Reuse,
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
                "--all" => parsed.all = true,
                "--active" => parsed.active = true,
                "--replace" => parsed.replace = true,
                "--json" => parsed.json = true,
                _ if arg.starts_with('-') => return Err(format!("unknown session flag `{arg}`")),
                _ => return Err(format!("unknown session subcommand `{arg}`")),
            }
            index += 1;
        }
        Ok(parsed)
    }
}

pub(super) fn session_guide(command: SessionCommand) -> Result<String, String> {
    let guide = load_agent_session_guide();
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
    }
    .filter(|value| !value.trim().is_empty())
}

fn agent_session_guide_has_any_text(
    guide: &agent_semantic_config::HookClientAgentSessionGuideConfig,
) -> bool {
    guide_text_for(guide, SessionCommand::Register).is_some()
        || guide_text_for(guide, SessionCommand::List).is_some()
        || guide_text_for(guide, SessionCommand::Show).is_some()
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
Register child agent sessions so the root agent can recall active exploration state.\n\
Use CODEX_THREAD_ID as the root session when available.\n\
asp agent session register --name asp-explore --child-session-id <child-session-id> --role asp-explore"
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
