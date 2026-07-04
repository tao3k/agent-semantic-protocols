//! Public interface for the DB-owned agent session registry.

mod bootstrap;
mod core;
mod lifecycle;
mod types;

pub use core::AgentSessionRegistry;
pub use types::{
    AGENT_SESSION_REGISTRY_DB_NAME, AGENT_SESSION_STATUS_ARCHIVED, AGENT_SESSION_STATUS_IDLE,
    AGENT_SESSION_STATUS_INVALID, AgentSessionLookupRequest, AgentSessionRecord,
    AgentSessionRegisterRequest, AgentSessionToolEventRequest,
    agent_session_normalized_metadata_json, agent_session_status_is_routable,
    agent_session_unix_timestamp,
};
