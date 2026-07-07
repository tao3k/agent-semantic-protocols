#![deny(dead_code)]

use std::process;

fn main() {
    if let Err(message) = agent_semantic_protocol::run_binary_from_env() {
        let message = if message.contains("file selectors are not executable code selectors")
            && !message.contains("search owner <path> items")
        {
            format!("{message}; recover with search owner <path> items")
        } else {
            message
        };
        eprintln!("{message}");
        process::exit(2);
    }
}
