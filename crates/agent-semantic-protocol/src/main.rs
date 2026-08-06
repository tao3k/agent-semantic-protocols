#![deny(dead_code)]

fn main() -> std::process::ExitCode {
    // Keep `asp hook` as the stable public ABI while bootstrap remains an
    // internal implementation boundary that runs before the full CLI parser.
    if agent_semantic_protocol::hook_bootstrap::is_hook_event_dispatch(std::env::args_os().skip(1))
    {
        agent_semantic_protocol::hook_bootstrap::terminate_hook_process(
            agent_semantic_protocol::hook_bootstrap::run_hook_bootstrap_from_env(),
        );
    }
    if let Err(message) = agent_semantic_protocol::run_binary_from_env() {
        eprintln!("{message}");
        return std::process::ExitCode::from(2);
    }
    std::process::ExitCode::SUCCESS
}
