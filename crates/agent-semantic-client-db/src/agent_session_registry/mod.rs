//! Public interface for the DB-owned agent session registry.

pub(crate) mod bootstrap;
pub(crate) use bootstrap as bootstrap_owner;
mod core;
pub(crate) mod dispatch;
pub(crate) use dispatch as dispatch_owner;
mod lifecycle;
pub(crate) mod permissions;
pub(crate) use permissions as permissions_owner;
pub(crate) mod record;
pub(crate) use record as record_owner;
mod replacement;
pub(crate) mod types;
pub(crate) use types as types_owner;

pub use types::{AgentSessionModelObservationRef, AgentSessionModelObservationSource};

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
