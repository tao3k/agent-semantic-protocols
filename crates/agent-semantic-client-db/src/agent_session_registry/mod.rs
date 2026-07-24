//! Public interface for the DB-owned agent session registry.

mod bootstrap;
mod core;
mod dispatch;
mod interactive_loop;
mod interactive_loop_actions;
mod interactive_loop_host_tree;
mod interactive_loop_runtime;
mod interactive_loop_transport;
mod interactive_loop_types;
pub use interactive_loop_host_tree::{
    resident_child_host_tree_audit_required_menu, resident_child_host_tree_observation_menu,
};
mod lifecycle;
mod permissions;
mod record;
mod replacement;
mod types;

pub use types::{AgentSessionModelObservationRef, AgentSessionModelObservationSource};

pub use interactive_loop::resident_child_bootstrap_menu;
pub use interactive_loop_runtime::{
    SameChildRuntimeOverrideState, classify_same_child_runtime_override_state,
    resident_child_host_runtime_refresh_eligible, resident_child_runtime_evidence_incomplete_menu,
    resident_child_runtime_repair_menu, typed_runtime_observation_matches_profile,
};
pub use interactive_loop_transport::{
    resident_child_live_transport_gate, resident_child_runtime_verified_menu,
};
pub use interactive_loop_types::{
    AgentSessionHostRequirement, AgentSessionInteractiveChoice, AgentSessionInteractiveMenu,
    AgentSessionInteractiveReceipt, AgentSessionInteractiveSession, AgentSessionLoopState,
    AgentSessionLoopTraceStep, ResidentChildBootstrapMenuInput,
};

pub use core::AgentSessionRegistry;
pub use dispatch::derive_agent_session_dispatch_identity;
pub use types::{
    AGENT_SESSION_REGISTRY_DB_NAME, AGENT_SESSION_STATUS_ACTIVE, AGENT_SESSION_STATUS_ARCHIVED,
    AGENT_SESSION_STATUS_IDLE, AGENT_SESSION_STATUS_INVALID, AgentSessionDispatchClaimRequest,
    AgentSessionDispatchClaimResult, AgentSessionDispatchCompleteRequest,
    AgentSessionDispatchDerivedIdentity, AgentSessionDispatchIdentityInput,
    AgentSessionDispatchLeaseRecord, AgentSessionDispatchMarkOrphanedRequest, AgentSessionId,
    AgentSessionLookupRequest, AgentSessionProjectId, AgentSessionRecord,
    AgentSessionRegisterRequest, AgentSessionResidentName, AgentSessionRootSessionId,
    AgentSessionStatus, AgentSessionToolEventRequest,
    agent_session_message_target_is_currently_routable, agent_session_message_target_is_live_bound,
    agent_session_normalized_metadata_json, agent_session_status_is_routable,
    agent_session_unix_timestamp,
};
