// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::{
    ProviderIncrementalScoped, ProviderOwnerDecision, ProviderOwnerMetadata, ProviderOwnerProbe,
    ProviderSearchWorkspaceSession,
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderOwnerBatchProbeRequest {
    pub owner_path: String,
    pub metadata: ProviderOwnerMetadata,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderOwnerBatchProbeResult {
    pub owner_path: String,
    pub probe: ProviderOwnerProbe,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderOwnerBatchProbeReceipt {
    pub results: Vec<ProviderOwnerBatchProbeResult>,
    pub read_lock_count: u32,
    pub connection_open_count: u32,
    pub scope_scan_count: u32,
}

struct StoredOwner {
    metadata: ProviderOwnerMetadata,
    content_digest: String,
}

/// Classify a provider inventory through a process-resident workspace session.
pub(super) async fn probe_provider_owners_in_session(
    session: &ProviderSearchWorkspaceSession,
    scope: &ProviderIncrementalScoped,
    owners: &[ProviderOwnerBatchProbeRequest],
) -> Result<ProviderOwnerBatchProbeReceipt, String> {
    let read_lease = session.read_connection();
    let generation = match read_active_generation(&read_lease, scope).await {
        Ok(generation) => generation,
        Err(error) if schema_is_missing(error.as_str()) => {
            return Ok(ProviderOwnerBatchProbeReceipt {
                results: new_owner_results(owners, None),
                read_lock_count: 0,
                connection_open_count: 0,
                scope_scan_count: 0,
            });
        }
        Err(error) => return Err(error),
    };
    let stored = read_stored_owners(&read_lease, scope).await?;
    Ok(ProviderOwnerBatchProbeReceipt {
        results: owners
            .iter()
            .map(|owner| classify_owner(owner, stored.get(owner.owner_path.as_str()), &generation))
            .collect(),
        read_lock_count: 0,
        connection_open_count: 0,
        scope_scan_count: 1,
    })
}

fn new_owner_results(
    owners: &[ProviderOwnerBatchProbeRequest],
    generation_before: Option<String>,
) -> Vec<ProviderOwnerBatchProbeResult> {
    owners
        .iter()
        .map(|owner| ProviderOwnerBatchProbeResult {
            owner_path: owner.owner_path.clone(),
            probe: ProviderOwnerProbe {
                decision: ProviderOwnerDecision::New,
                generation_before: generation_before.clone(),
                content_digest: None,
            },
        })
        .collect()
}

fn classify_owner(
    owner: &ProviderOwnerBatchProbeRequest,
    stored: Option<&StoredOwner>,
    generation_before: &Option<String>,
) -> ProviderOwnerBatchProbeResult {
    let probe = match stored {
        None => ProviderOwnerProbe {
            decision: ProviderOwnerDecision::New,
            generation_before: generation_before.clone(),
            content_digest: None,
        },
        Some(stored) => ProviderOwnerProbe {
            decision: if stored.metadata == owner.metadata {
                ProviderOwnerDecision::Unchanged
            } else {
                ProviderOwnerDecision::Changed
            },
            generation_before: generation_before.clone(),
            content_digest: Some(stored.content_digest.clone()),
        },
    };
    ProviderOwnerBatchProbeResult {
        owner_path: owner.owner_path.clone(),
        probe,
    }
}

async fn read_active_generation(
    connection: &turso::Connection,
    scope: &ProviderIncrementalScoped,
) -> Result<Option<String>, String> {
    let mut rows = connection
        .query(
            "SELECT generation_id
             FROM provider_active_generation_v1
             WHERE project_root = ?1
               AND workspace_identity = ?2
               AND provider_workspace_identity_digest = ?3
               AND provider_id = ?4
             LIMIT 1",
            (
                scope.project_root.as_str(),
                scope.workspace_identity.as_str(),
                scope.provider_workspace_identity_digest.as_str(),
                scope.provider_id.as_str(),
            ),
        )
        .await
        .map_err(|error| format!("failed to read provider active generation: {error}"))?;
    rows.next()
        .await
        .map_err(|error| format!("failed to read provider active generation row: {error}"))?
        .map(|row| {
            row.get::<String>(0)
                .map_err(|error| format!("failed to decode provider active generation: {error}"))
        })
        .transpose()
}

async fn read_stored_owners(
    connection: &turso::Connection,
    scope: &ProviderIncrementalScoped,
) -> Result<BTreeMap<String, StoredOwner>, String> {
    let mut rows = connection
        .query(
            "SELECT owner_path, file_identity, size_bytes, modified_unix_nanos,
                    change_time_unix_nanos, content_digest
             FROM provider_owner_fingerprint_v1
             WHERE project_root = ?1
               AND workspace_identity = ?2
               AND provider_workspace_identity_digest = ?3
               AND provider_id = ?4
             ORDER BY owner_path",
            (
                scope.project_root.as_str(),
                scope.workspace_identity.as_str(),
                scope.provider_workspace_identity_digest.as_str(),
                scope.provider_id.as_str(),
            ),
        )
        .await
        .map_err(|error| format!("failed to read provider owner fingerprints: {error}"))?;
    let mut stored = BTreeMap::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read provider owner fingerprint row: {error}"))?
    {
        let owner_path = row
            .get::<String>(0)
            .map_err(|error| format!("failed to decode ownerPath: {error}"))?;
        let size_bytes = row
            .get::<i64>(2)
            .map_err(|error| format!("failed to decode sizeBytes: {error}"))?
            .try_into()
            .map_err(|_| "stored provider sizeBytes must be non-negative".to_string())?;
        stored.insert(
            owner_path,
            StoredOwner {
                metadata: ProviderOwnerMetadata {
                    file_identity: row
                        .get::<String>(1)
                        .map_err(|error| format!("failed to decode fileIdentity: {error}"))?,
                    size_bytes,
                    modified_unix_nanos: row
                        .get::<i64>(3)
                        .map_err(|error| format!("failed to decode modifiedUnixNanos: {error}"))?,
                    change_time_unix_nanos: row.get::<i64>(4).map_err(|error| {
                        format!("failed to decode changeTimeUnixNanos: {error}")
                    })?,
                },
                content_digest: row
                    .get::<String>(5)
                    .map_err(|error| format!("failed to decode contentDigest: {error}"))?,
            },
        );
    }
    Ok(stored)
}

fn schema_is_missing(error: &str) -> bool {
    error.contains("no such table") || error.contains("does not exist")
}
