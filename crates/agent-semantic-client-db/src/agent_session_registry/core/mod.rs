//! Agent-session registry facade over storage and synchronous API ownership.

mod api;
mod storage;

pub use storage::AgentSessionRegistry;
pub(super) use storage::{
    block_on_agent_session_registry_async, connect_turso_agent_session_registry,
    turso_session_by_name,
};

pub(super) use crate::agent_session_registry::{bootstrap, dispatch, permissions, record, types};
