#![deny(dead_code)]

use std::process;

fn main() {
    // Keep `asp hook` as the stable public ABI while bootstrap remains an
    // internal implementation boundary that runs before the full CLI parser.
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("hook")) {
        std::process::exit(agent_semantic_protocol::hook_bootstrap::run_hook_bootstrap_from_env());
    }
    if let Err(message) = agent_semantic_protocol::run_binary_from_env() {
        eprintln!("{message}");
        process::exit(2);
    }
}
