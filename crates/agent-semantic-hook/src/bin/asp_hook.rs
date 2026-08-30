#![deny(dead_code)]

//! Thin executable boundary for the canonical Runtime Hook evaluator.

fn main() -> std::process::ExitCode {
    agent_semantic_hook::run_hook_binary_from_env()
}
