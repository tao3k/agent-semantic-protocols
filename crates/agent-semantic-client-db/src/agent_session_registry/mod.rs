//! Public interface for the DB-owned agent session registry.

mod bootstrap;
mod core;
mod interactive_loop;
pub use interactive_loop::{
    resident_child_host_tree_audit_required_menu, resident_child_host_tree_observation_menu,
    resident_child_runtime_evidence_incomplete_menu, typed_runtime_observation_matches_profile,
};
mod lifecycle;
mod record;
mod types;

pub use types::{AgentSessionModelObservationRef, AgentSessionModelObservationSource};

pub use interactive_loop::resident_child_runtime_verified_menu;
pub use interactive_loop::{
    AgentSessionHostRequirement, AgentSessionInteractiveChoice, AgentSessionInteractiveMenu,
    AgentSessionInteractiveReceipt, AgentSessionInteractiveSession, AgentSessionLoopState,
    AgentSessionLoopTraceStep, ResidentChildBootstrapMenuInput, SameChildRuntimeOverrideState,
    classify_same_child_runtime_override_state, resident_child_bootstrap_menu,
    resident_child_host_runtime_refresh_eligible, resident_child_runtime_repair_menu,
};

pub use core::AgentSessionRegistry;
pub use types::{
    AGENT_SESSION_REGISTRY_DB_NAME, AGENT_SESSION_STATUS_ACTIVE, AGENT_SESSION_STATUS_ARCHIVED,
    AGENT_SESSION_STATUS_IDLE, AGENT_SESSION_STATUS_INVALID, AgentSessionLookupRequest,
    AgentSessionRecord, AgentSessionRegisterRequest, AgentSessionToolEventRequest,
    agent_session_message_target_is_live_bound, agent_session_normalized_metadata_json,
    agent_session_status_is_routable, agent_session_unix_timestamp,
};
