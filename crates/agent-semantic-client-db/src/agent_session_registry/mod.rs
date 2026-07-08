//! Public interface for the DB-owned agent session registry.

mod bootstrap;
mod core;
mod interactive_loop;
mod lifecycle;
mod types;

pub use interactive_loop::{
    AgentSessionHostRequirement, AgentSessionInteractiveChoice, AgentSessionInteractiveMenu,
    AgentSessionInteractiveReceipt, AgentSessionInteractiveSession, AgentSessionLoopState,
    AgentSessionLoopTraceStep, ResidentChildBootstrapMenuInput, resident_child_bootstrap_menu,
};

pub use core::AgentSessionRegistry;
pub use types::{
    AGENT_SESSION_REGISTRY_DB_NAME, AGENT_SESSION_STATUS_ACTIVE, AGENT_SESSION_STATUS_ARCHIVED,
    AGENT_SESSION_STATUS_IDLE, AGENT_SESSION_STATUS_INVALID, AgentSessionLookupRequest,
    AgentSessionRecord, AgentSessionRegisterRequest, AgentSessionToolEventRequest,
    agent_session_normalized_metadata_json, agent_session_status_is_routable,
    agent_session_unix_timestamp,
};
