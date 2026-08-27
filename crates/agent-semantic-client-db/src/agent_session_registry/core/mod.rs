//! Agent-session registry facade over storage and synchronous API ownership.

mod api;
mod retirement;
mod storage;
mod storage_bootstrap;

pub use storage::AgentSessionRegistry;
pub(super) use storage::{
    block_on_agent_session_registry_async, connect_turso_agent_session_registry,
    turso_session_by_id,
};
pub(super) use storage_bootstrap::bootstrap_turso_agent_session_schema;

pub(super) use crate::agent_session_registry::{
    dispatch_owner as dispatch, permissions_owner as permissions, publication,
    record_owner as record, types_owner as types,
};
