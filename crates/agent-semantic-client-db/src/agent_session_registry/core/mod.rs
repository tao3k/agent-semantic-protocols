//! Agent-session registry facade over storage and synchronous API ownership.

mod api;
mod retirement;
mod storage;

pub(super) use crate::agent_session_registry::{record_owner as record, types_owner as types};
pub use storage::AgentSessionRegistry;
pub(super) use storage::{
    block_on_agent_session_registry_async, connect_turso_agent_session_registry,
    turso_session_by_id,
};
