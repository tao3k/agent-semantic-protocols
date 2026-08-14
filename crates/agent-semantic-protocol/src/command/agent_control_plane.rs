use super::{agent_config_sync, agent_window};

pub(crate) fn run_agent_command(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("config") => agent_config_sync::run_agent_config_command(&args[1..]),
        Some("help" | "--help" | "-h") | None => {
            println!("usage: asp agent config sync");
            Ok(())
        }
        Some(command) => Err(format!(
            "unknown agent command {command}\nusage: asp agent config sync"
        )),
    }
}

pub(crate) async fn run_session_control_plane_command(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("help" | "--help" | "-h") => {
            println!("{}", agent_window::session_control_plane_usage());
            Ok(())
        }
        Some("register-current-child") => agent_window::register_current_child_session().await,
        _ => agent_window::run_session_control_plane(args).await,
    }
}
