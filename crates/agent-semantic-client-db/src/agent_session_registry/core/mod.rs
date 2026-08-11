//! Agent-session registry facade over storage and synchronous API ownership.

mod api;
mod host_execution;
mod retirement;
mod storage;
mod storage_bootstrap;

pub use storage::AgentSessionRegistry;
pub(super) use storage::{
    block_on_agent_session_registry_async, connect_turso_agent_session_registry,
    turso_session_by_name,
};

pub(super) use crate::agent_session_registry::{
    bootstrap_owner as bootstrap, dispatch_owner as dispatch, permissions_owner as permissions,
    record_owner as record, types_owner as types,
};
