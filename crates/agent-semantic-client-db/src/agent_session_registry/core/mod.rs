//! Agent-session registry facade over storage and synchronous API ownership.

mod api;
mod retirement;
mod storage;

pub(super) use crate::agent_session_registry::record_owner as record;
pub(super) use crate::agent_session_registry::types_owner as types;
pub use storage::AgentSessionRegistry;
pub(super) use storage::block_on_agent_session_registry_async;
pub(super) use storage::connect_turso_agent_session_registry;
pub(super) use storage::turso_session_by_id;
