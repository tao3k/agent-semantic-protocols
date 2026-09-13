// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Typed artifact retention planning and leases.

use serde::Deserialize;
use serde::Serialize;

pub const RETENTION_PLAN_SCHEMA_ID: &str = "agent.semantic-protocols.state-home-retention-plan";
pub const RETENTION_PLAN_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum CleanupSelection {
    All,
    WorkspaceDigest { workspace_digest: String },
    ObjectId { object_id: String },
}

impl CleanupSelection {
    pub fn validate(&self) -> Result<(), String> {
        let value = match self {
            Self::All => return Ok(()),
            Self::WorkspaceDigest { workspace_digest } => {
                crate::blake3_content_digest::Blake3ContentDigest::parse(workspace_digest)
                    .map_err(|error| {
                        format!("reasonKind=state-home-cleanup-workspace-digest-invalid {error}")
                    })?;
                return Ok(());
            }
            Self::ObjectId { object_id } => object_id,
        };
        if value.trim().is_empty() {
            return Err("reasonKind=state-home-cleanup-selection-empty".to_string());
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RetentionObjectKind {
    Project,
    Workspace,
    Artifact,
    ProviderBuild,
    SourceSnapshot,
    Quarantine,
    Receipt,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetainedObject {
    pub object_id: String,
    pub kind: RetentionObjectKind,
    pub last_observed_at_ms: u64,
    pub byte_count: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetentionLease {
    pub lease_id: String,
    pub object_id: String,
    pub owner: String,
    pub expires_at_ms: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum CleanupDisposition {
    Keep { reason: String },
    Delete { reason: String },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupPlanEntry {
    pub object: RetainedObject,
    pub disposition: CleanupDisposition,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupPlan {
    pub schema_id: String,
    pub schema_version: u32,
    pub evaluated_at_ms: u64,
    pub retain_for_ms: u64,
    pub selection: CleanupSelection,
    pub retained_count: usize,
    pub deleted_count: usize,
    pub retained_bytes: u64,
    pub deleted_bytes: u64,
    pub entries: Vec<CleanupPlanEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetentionPlanner {
    evaluated_at_ms: u64,
    retain_for_ms: u64,
}

impl RetentionPlanner {
    pub fn new(evaluated_at_ms: u64, retain_for_ms: u64) -> Self {
        Self {
            evaluated_at_ms,
            retain_for_ms,
        }
    }

    pub fn plan(
        &self,
        objects: Vec<RetainedObject>,
        leases: &[RetentionLease],
    ) -> Result<CleanupPlan, String> {
        self.plan_selected(objects, leases, CleanupSelection::All)
    }

    pub fn plan_selected(
        &self,
        mut objects: Vec<RetainedObject>,
        leases: &[RetentionLease],
        selection: CleanupSelection,
    ) -> Result<CleanupPlan, String> {
        selection.validate()?;
        if !matches!(selection, CleanupSelection::All) && objects.is_empty() {
            return Err("reasonKind=state-home-cleanup-selection-no-match".to_string());
        }
        validate_unique_objects(&objects)?;
        validate_leases(leases)?;
        objects.sort_by(|left, right| {
            left.kind
                .cmp(&right.kind)
                .then_with(|| left.object_id.cmp(&right.object_id))
        });

        let entries = objects
            .into_iter()
            .map(|object| {
                let active_lease = leases.iter().find(|lease| {
                    lease.object_id == object.object_id
                        && lease
                            .expires_at_ms
                            .is_none_or(|expires| expires > self.evaluated_at_ms)
                });
                let disposition = if let Some(lease) = active_lease {
                    CleanupDisposition::Keep {
                        reason: format!("active-lease:{}:{}", lease.owner, lease.lease_id),
                    }
                } else if self
                    .evaluated_at_ms
                    .saturating_sub(object.last_observed_at_ms)
                    < self.retain_for_ms
                {
                    CleanupDisposition::Keep {
                        reason: "inside-retention-window".to_string(),
                    }
                } else {
                    CleanupDisposition::Delete {
                        reason: "unleased-and-expired".to_string(),
                    }
                };
                CleanupPlanEntry {
                    object,
                    disposition,
                }
            })
            .collect::<Vec<_>>();

        let retained_count = entries
            .iter()
            .filter(|entry| matches!(entry.disposition, CleanupDisposition::Keep { .. }))
            .count();
        let deleted_count = entries.len() - retained_count;
        let retained_bytes = entries
            .iter()
            .filter(|entry| matches!(entry.disposition, CleanupDisposition::Keep { .. }))
            .map(|entry| entry.object.byte_count)
            .sum();
        let deleted_bytes = entries
            .iter()
            .filter(|entry| matches!(entry.disposition, CleanupDisposition::Delete { .. }))
            .map(|entry| entry.object.byte_count)
            .sum();

        Ok(CleanupPlan {
            schema_id: RETENTION_PLAN_SCHEMA_ID.to_string(),
            schema_version: RETENTION_PLAN_SCHEMA_VERSION,
            evaluated_at_ms: self.evaluated_at_ms,
            retain_for_ms: self.retain_for_ms,
            selection,
            retained_count,
            deleted_count,
            retained_bytes,
            deleted_bytes,
            entries,
        })
    }
}

fn validate_unique_objects(objects: &[RetainedObject]) -> Result<(), String> {
    let mut ids = std::collections::BTreeSet::new();
    for object in objects {
        if object.object_id.is_empty() {
            return Err("retained object id must not be empty".to_string());
        }
        if !ids.insert(object.object_id.as_str()) {
            return Err(format!(
                "duplicate retained object id: {}",
                object.object_id
            ));
        }
    }
    Ok(())
}

fn validate_leases(leases: &[RetentionLease]) -> Result<(), String> {
    for lease in leases {
        if lease.lease_id.is_empty() || lease.object_id.is_empty() || lease.owner.is_empty() {
            return Err("retention lease identity fields must not be empty".to_string());
        }
    }
    Ok(())
}
