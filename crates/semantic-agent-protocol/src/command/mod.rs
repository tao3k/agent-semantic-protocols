//! Command tree for the `semantic-agent-protocol` binary.

mod ast_patch;
mod dispatch;
mod hook;

pub(crate) use dispatch::run_protocol_command;
