// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use serde::{Deserialize, Serialize};

pub const RESIDENT_WORKSPACE_RETIREMENT_RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.resident-workspace-retirement-receipt";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResidentWorkspaceRetirementReason {
    WorkspaceMissing,
    IdleTimeout,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResidentWorkspaceRetirementReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub workspace_identity: String,
    pub reason: ResidentWorkspaceRetirementReason,
    pub idle_timeout_seconds: u64,
    pub live_lease_count: usize,
    pub in_flight_request_count: usize,
    pub checkpoint_completed: bool,
    pub writer_lane_drained: bool,
    pub endpoint_retired: bool,
}

impl ResidentWorkspaceRetirementReceipt {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != RESIDENT_WORKSPACE_RETIREMENT_RECEIPT_SCHEMA_ID
            || self.schema_version != "1"
        {
            return Err(
                "resident workspace retirement receipt schema identity mismatch".to_owned(),
            );
        }
        if self.workspace_identity.trim().is_empty() || self.idle_timeout_seconds < 3_600 {
            return Err(
                "resident workspace retirement receipt identity or timeout is invalid".to_owned(),
            );
        }
        if self.live_lease_count != 0 || self.in_flight_request_count != 0 {
            return Err("resident workspace retirement occurred with live activity".to_owned());
        }
        if !self.checkpoint_completed || !self.writer_lane_drained || !self.endpoint_retired {
            return Err("resident workspace retirement boundary is incomplete".to_owned());
        }
        Ok(())
    }
}
