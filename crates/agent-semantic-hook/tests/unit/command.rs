#[path = "../../src/command/shell.rs"]
mod shell_impl;

pub(crate) use shell_impl::{looks_like_command_transcript, semantic_shell_tokens};
