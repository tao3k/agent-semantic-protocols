//! Command tree for the `asp` binary.

mod ast_patch;
mod dispatch;
mod document_provider;
mod graph;
mod healthcheck;
mod hook;
mod hook_enforcement;
mod hook_runtime;
mod protocol_binary;
mod provider;
mod provider_process;
mod provider_roots;
mod search_pipe;
mod source_access;

pub(crate) use dispatch::run_protocol_command;
