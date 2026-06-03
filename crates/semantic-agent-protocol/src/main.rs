use std::process;

fn main() {
    if let Err(message) = semantic_agent_protocol::run_cli_from_env() {
        eprintln!("{message}");
        process::exit(2);
    }
}
