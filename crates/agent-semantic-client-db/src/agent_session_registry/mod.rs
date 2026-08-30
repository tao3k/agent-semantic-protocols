//! Public interface for the DB-owned agent session registry.

mod core;
mod collaboration;
pub(crate) mod dispatch;
pub(crate) use dispatch as dispatch_owner;
mod lifecycle;
pub(crate) mod permissions;
pub(crate) use permissions as permissions_owner;
mod publication;
pub(crate) mod record;
pub(crate) use record as record_owner;
pub(crate) mod types;
pub(crate) use types as types_owner;

pub use types::{AgentSessionModelObservationRef, AgentSessionModelObservationSource};

pub use core::AgentSessionRegistry;
pub use collaboration::{
    CollaborationAgentObservation, CollaborationSnapshotPersistenceReceipt,
    run_collaboration_snapshot_inbox,
};
pub use dispatch::derive_agent_session_dispatch_identity;
pub use types::{
    AGENT_SESSION_REGISTRY_DB_NAME, AGENT_SESSION_STATUS_ACTIVE, AGENT_SESSION_STATUS_ARCHIVED,
    AGENT_SESSION_STATUS_IDLE, AGENT_SESSION_STATUS_INVALID, AgentSessionCommandDigest,
    AgentSessionDeliveryTargetId, AgentSessionDispatchClaimRequest,
    AgentSessionDispatchClaimResult, AgentSessionDispatchCompleteRequest,
    AgentSessionDispatchDerivedIdentity, AgentSessionDispatchIdentity,
    AgentSessionDispatchIdentityInput, AgentSessionDispatchLeaseRecord,
    AgentSessionDispatchMarkOrphanedRequest, AgentSessionEvidenceRef, AgentSessionId,
    AgentSessionLookupRequest, AgentSessionMessageTargetId, AgentSessionMetadataJson,
    AgentSessionModelEvidenceRef, AgentSessionModelId, AgentSessionModelObservationSourceId,
    AgentSessionProjectId, AgentSessionRecord, AgentSessionRegisterRequest,
    AgentSessionResidentName, AgentSessionRole, AgentSessionRootSessionId, AgentSessionStatus,
    AgentSessionToolEventRequest, agent_session_message_target_is_currently_routable,
    agent_session_message_target_is_live_bound, agent_session_normalized_metadata_json,
    agent_session_status_is_routable, agent_session_unix_timestamp,
};
