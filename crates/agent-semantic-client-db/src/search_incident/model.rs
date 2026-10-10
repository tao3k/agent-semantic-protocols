// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum IncidentSurface {
    Search,
    Query,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RequestedProjection {
    None,
    Seeds,
    Source,
    CallableSkeleton,
    Packet,
}

impl RequestedProjection {
    pub fn requires_code(self) -> bool {
        matches!(self, Self::Source | Self::CallableSkeleton)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum IncidentState {
    Open,
    Repairing,
    VerificationPending,
    FailedVerification,
    Resolved,
    Superseded,
    Compacted,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IncidentIdentity {
    pub incident_id: String,
    pub workspace_identity: String,
    pub language_id: String,
    pub surface: IncidentSurface,
    pub canonical_request_digest: String,
    pub requested_projection: RequestedProjection,
    pub stage: String,
    pub reason_kind: String,
    pub runtime_artifact_digest: Option<String>,
    pub provider_contract_digest: Option<String>,
    pub generation_digest: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceObservation {
    pub resident_memory_bytes: Option<u64>,
    pub peak_resident_memory_bytes: Option<u64>,
    pub disk_read_bytes: Option<u64>,
    pub disk_write_bytes: Option<u64>,
    pub event_loop_lag_micros: Option<u64>,
    pub active_task_count: Option<u64>,
    pub active_child_count: Option<u64>,
    pub writer_queue_depth: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IncidentObservation {
    pub identity: IncidentIdentity,
    pub succeeded: bool,
    pub blocked: bool,
    pub budget_exceeded: bool,
    pub code_projection_present: bool,
    pub budget_micros: Option<u64>,
    pub elapsed_micros: Option<u64>,
    pub observed_at_unix_micros: u64,
    pub resources: ResourceObservation,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayReceipt {
    pub receipt_digest: String,
    pub canonical_request_digest: String,
    pub workspace_identity: String,
    pub runtime_artifact_digest: String,
    pub generation_digest: String,
    pub code_projection_present: bool,
    pub within_budget: bool,
    pub verified_at_unix_micros: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IncidentRecord {
    pub identity: IncidentIdentity,
    pub state: IncidentState,
    pub occurrence_count: u64,
    pub transition_sequence: u64,
    pub first_seen_unix_micros: u64,
    pub last_seen_unix_micros: u64,
    pub code_projection_present: bool,
    pub budget_micros: Option<u64>,
    pub elapsed_micros: Option<u64>,
    pub repair_artifact_digest: Option<String>,
    pub verification_receipt: Option<ReplayReceipt>,
    pub resources: ResourceObservation,
}

impl IncidentRecord {
    pub fn is_active(&self) -> bool {
        matches!(
            self.state,
            IncidentState::Open
                | IncidentState::Repairing
                | IncidentState::VerificationPending
                | IncidentState::FailedVerification
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransitionError {
    IdentityMismatch,
    InvalidState,
    ReplayMismatch,
    VerificationRequired,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IncidentTelemetryEvent {
    pub observation: IncidentObservation,
    pub state: IncidentState,
    pub transition: String,
    pub transition_sequence: u64,
}

impl IncidentTelemetryEvent {
    pub fn from_record(
        observation: IncidentObservation,
        record: &IncidentRecord,
        transition: impl Into<String>,
    ) -> Result<Self, TransitionError> {
        if observation.identity != record.identity || record.transition_sequence == 0 {
            return Err(TransitionError::IdentityMismatch);
        }
        Ok(Self {
            observation,
            state: record.state,
            transition: transition.into(),
            transition_sequence: record.transition_sequence,
        })
    }

    pub fn is_valid(&self) -> bool {
        self.transition_sequence > 0
            && !self.transition.is_empty()
            && !self.observation.identity.incident_id.is_empty()
            && !self
                .observation
                .identity
                .canonical_request_digest
                .is_empty()
            && !self.observation.identity.workspace_identity.is_empty()
    }
}
