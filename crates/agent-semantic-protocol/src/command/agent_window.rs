use crate::multi_agent_session::{ChoicePlaneRequest, open_choice_plane};

#[derive(Debug)]
struct AgentWindowRequest {
    json: bool,
}

impl AgentWindowRequest {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut choice_plane = false;
        let mut json = false;
        let mut index = 0;
        while index < args.len() {
            match args[index].as_str() {
                "--agents" => {
                    index += 1;
                    let surface = args
                        .get(index)
                        .ok_or_else(|| "--agents requires choice-plane".to_owned())?;
                    if surface != "choice-plane" {
                        return Err(format!(
                            "unsupported Agent session surface {surface}; expected choice-plane"
                        ));
                    }
                    choice_plane = true;
                }
                value if value.starts_with("--agents=") => {
                    let surface = &value["--agents=".len()..];
                    if surface != "choice-plane" {
                        return Err(format!(
                            "unsupported Agent session surface {surface}; expected choice-plane"
                        ));
                    }
                    choice_plane = true;
                }
                "--json" => json = true,
                value if value.starts_with('-') => {
                    return Err(format!("unknown Agent window option {value}"));
                }
                value => {
                    return Err(format!(
                        "asp session does not accept an agent or task argument {value}; the current denied Hook event selects the registered subagent"
                    ));
                }
            }
            index += 1;
        }
        if !choice_plane {
            return Err("asp session requires --agents choice-plane".to_owned());
        }
        Ok(Self { json })
    }
}

pub(super) fn session_control_plane_usage() -> String {
    "usage: asp session --agents choice-plane [--json]".to_owned()
}

pub(super) fn run_session_control_plane(args: &[String]) -> Result<(), String> {
    let request = AgentWindowRequest::parse(args)?;
    let platform = current_session_platform()?;
    let project_root = std::env::current_dir()
        .map_err(|error| format!("failed to resolve Agent window workspace: {error}"))?;
    let rendered = open_choice_plane(ChoicePlaneRequest {
        project_root: &project_root,
        platform,
        json: request.json,
    })?;
    println!("{rendered}");
    Ok(())
}

fn current_session_platform() -> Result<&'static str, String> {
    session_platform_from_ids(
        std::env::var("CODEX_SESSION_ID").ok().as_deref(),
        std::env::var("CODEX_THREAD_ID").ok().as_deref(),
        std::env::var("CLAUDE_CODE_SESSION_ID").ok().as_deref(),
        std::env::var("CLAUDE_CODE_REMOTE_SESSION_ID")
            .ok()
            .as_deref(),
    )
}

fn session_platform_from_ids(
    codex_session_id: Option<&str>,
    codex_thread_id: Option<&str>,
    claude_code_session_id: Option<&str>,
    claude_code_remote_session_id: Option<&str>,
) -> Result<&'static str, String> {
    let present = |value: Option<&str>| value.is_some_and(|value| !value.trim().is_empty());
    let codex = present(codex_session_id) || present(codex_thread_id);
    let claude = present(claude_code_session_id) || present(claude_code_remote_session_id);
    match (codex, claude) {
        (true, false) => Ok("codex"),
        (false, true) => Ok("claude"),
        (false, false) => Err(
            "agent-session-host-unavailable: a Codex or Claude Code session identity is required"
                .to_owned(),
        ),
        (true, true) => Err(
            "agent-session-host-ambiguous: both Codex and Claude session identities are present"
                .to_owned(),
        ),
    }
}
