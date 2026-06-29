#![deny(dead_code)]

use std::process;

fn main() {
    if let Err(message) = agent_semantic_protocol::run_binary_from_env() {
        eprintln!("{message}");
        process::exit(2);
    }
}
