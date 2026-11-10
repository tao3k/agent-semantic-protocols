// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Validation receipts for workspace recovery, warm data-plane latency, and shutdown.

use serde::{Deserialize, Serialize};

use super::{RuntimeDataPlaneCounters, WorkspaceGenerationState, WorkspaceRecoverySource};

pub const WORKSPACE_RECOVERY_RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.runtime-server-workspace-recovery-receipt.v1";
pub const WORKSPACE_DATA_PLANE_PERFORMANCE_RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.runtime-server-workspace-data-plane-performance-receipt.v1";
pub const RUNTIME_SERVER_SHUTDOWN_RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.runtime-server-shutdown-receipt.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRecoveryReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub request_id: String,
    pub workspace_identity: String,
    pub source: WorkspaceRecoverySource,
    pub state: WorkspaceGenerationState,
    pub active_epoch: u64,
    pub target_epoch: u64,
    pub generation_digest: String,
    pub source_root_digest: String,
    pub projection_capability:
        crate::active_generation_projection_capability::ActiveGenerationProjectionCapabilityReceipt,
    pub old_generation_readable: bool,
    pub resident_publication_elapsed_micros: u64,
    pub counters: RuntimeDataPlaneCounters,
}

impl WorkspaceRecoveryReceipt {
    pub fn validate(&self) -> Result<(), String> {
        self.projection_capability.validate()?;
        if self.projection_capability.publication_epoch != self.target_epoch {
            return Err(format!(
                "workspace recovery projection capability epoch drift: receiptTargetEpoch={} capabilityPublicationEpoch={}",
                self.target_epoch, self.projection_capability.publication_epoch
            ));
        }
        if self.projection_capability.generation_digest != self.generation_digest {
            return Err(
                "workspace recovery projection capability generation digest drift".to_owned(),
            );
        }
        if self.projection_capability.root_digest != self.source_root_digest {
            return Err("workspace recovery projection capability root digest drift".to_owned());
        }
        if self.schema_id != WORKSPACE_RECOVERY_RECEIPT_SCHEMA_ID || self.schema_version != "1" {
            return Err("workspace recovery receipt schema identity mismatch".to_owned());
        }
        if self.request_id.trim().is_empty() || self.workspace_identity.trim().is_empty() {
            return Err("workspace recovery receipt identity must be non-empty".to_owned());
        }
        if self.target_epoch == 0 || self.target_epoch <= self.active_epoch {
            return Err("workspace recovery target epoch must advance".to_owned());
        }
        if self.generation_digest.trim().is_empty() || self.source_root_digest.trim().is_empty() {
            return Err("workspace recovery receipt generation identity is incomplete".to_owned());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDataPlanePerformanceReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub workspace_count: usize,
    pub session_count: usize,
    pub sample_count: usize,
    pub p50_micros: u64,
    pub p99_micros: u64,
    pub max_micros: u64,
    pub counters: RuntimeDataPlaneCounters,
}

impl WorkspaceDataPlanePerformanceReceipt {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != WORKSPACE_DATA_PLANE_PERFORMANCE_RECEIPT_SCHEMA_ID
            || self.schema_version != "1"
        {
            return Err(format!(
                "workspace data-plane performance schema identity mismatch: expectedSchemaId={} actualSchemaId={} expectedSchemaVersion=1 actualSchemaVersion={}",
                WORKSPACE_DATA_PLANE_PERFORMANCE_RECEIPT_SCHEMA_ID,
                self.schema_id,
                self.schema_version
            ));
        }
        if self.workspace_count < 2 || self.session_count < 2 || self.sample_count == 0 {
            return Err(
                "workspace data-plane performance coverage is not concurrent or multi-workspace"
                    .to_owned(),
            );
        }
        if self.p50_micros > self.p99_micros || self.p99_micros > self.max_micros {
            return Err("workspace data-plane latency percentiles are not monotonic".to_owned());
        }
        if self.p99_micros >= 1_000 {
            return Err(format!(
                "workspace data-plane p99 exceeds the sub-millisecond gate: p99Micros={}",
                self.p99_micros
            ));
        }
        self.counters.validate_zero_io()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeServerShutdownReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub workspace_count: usize,
    pub writer_lane_count: usize,
    pub queued_publications_drained: bool,
    pub forced_abort_count: usize,
}

impl RuntimeServerShutdownReceipt {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != RUNTIME_SERVER_SHUTDOWN_RECEIPT_SCHEMA_ID || self.schema_version != "1"
        {
            return Err("Runtime Server shutdown receipt schema identity mismatch".to_owned());
        }
        if !self.queued_publications_drained || self.forced_abort_count != 0 {
            return Err("Runtime Server shutdown did not drain cleanly".to_owned());
        }
        if self.workspace_count != self.writer_lane_count {
            return Err("Runtime Server shutdown lane count mismatch".to_owned());
        }
        Ok(())
    }
}
