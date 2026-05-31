use std::process;

fn main() {
    if let Err(message) = semantic_agent_hook::run_cli_from_env() {
        eprintln!("{message}");
        process::exit(2);
    }
}
