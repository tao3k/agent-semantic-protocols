// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Public interface for the DB-owned agent session registry.

mod core;
pub(crate) mod dispatch;
mod lifecycle;
pub(crate) mod permissions;
mod publication;
pub(crate) mod record;
mod schema;
pub(crate) use record as record_owner;
pub(crate) mod types;
pub(crate) use types as types_owner;

pub use types::AgentSessionModelObservationRef;
pub use types::AgentSessionModelObservationSource;

pub use core::AgentSessionRegistry;
pub use dispatch::derive_agent_session_dispatch_identity;
pub use types::AGENT_SESSION_REGISTRY_DB_NAME;
pub use types::AGENT_SESSION_STATUS_ACTIVE;
pub use types::AGENT_SESSION_STATUS_ARCHIVED;
pub use types::AGENT_SESSION_STATUS_IDLE;
pub use types::AGENT_SESSION_STATUS_INVALID;
pub use types::AgentSessionCommandDigest;
pub use types::AgentSessionDeliveryTargetId;
pub use types::AgentSessionDispatchClaimRequest;
pub use types::AgentSessionDispatchClaimResult;
pub use types::AgentSessionDispatchCompleteRequest;
pub use types::AgentSessionDispatchDerivedIdentity;
pub use types::AgentSessionDispatchIdentity;
pub use types::AgentSessionDispatchIdentityInput;
pub use types::AgentSessionDispatchLeaseRecord;
pub use types::AgentSessionDispatchMarkOrphanedRequest;
pub use types::AgentSessionEvidenceRef;
pub use types::AgentSessionId;
pub use types::AgentSessionLookupRequest;
pub use types::AgentSessionMessageTargetId;
pub use types::AgentSessionMetadataJson;
pub use types::AgentSessionModelEvidenceRef;
pub use types::AgentSessionModelId;
pub use types::AgentSessionModelObservationSourceId;
pub use types::AgentSessionProjectId;
pub use types::AgentSessionRecord;
pub use types::AgentSessionRegisterRequest;
pub use types::AgentSessionResidentName;
pub use types::AgentSessionRole;
pub use types::AgentSessionRootSessionId;
pub use types::AgentSessionStatus;
pub use types::AgentSessionToolEventRequest;
pub use types::agent_session_message_target_is_currently_routable;
pub use types::agent_session_message_target_is_live_bound;
pub use types::agent_session_normalized_metadata_json;
pub use types::agent_session_status_is_routable;
pub use types::agent_session_unix_timestamp;
