use super::agent_config_sync;

pub(crate) fn run_config_command(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("agents") => agent_config_sync::run_agent_config_command(&args[1..]),
        Some("help" | "--help" | "-h") | None => {
            println!("usage: asp config agents sync");
            Ok(())
        }
        Some(command) => Err(format!(
            "unknown config command {command}\nusage: asp config agents sync"
        )),
    }
}
