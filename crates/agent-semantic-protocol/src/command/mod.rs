//! Command tree for the `asp` binary.

mod ast_patch;
mod dispatch;
mod graph;
mod hook;
mod hook_runtime;
mod protocol_binary;
mod provider;
mod source_access;

pub(crate) use dispatch::run_protocol_command;
