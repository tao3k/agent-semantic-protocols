use std::collections::BTreeMap;

use super::{
    ProviderIncrementalScopeV1, ProviderOwnerDecisionV1, ProviderOwnerMetadataV1,
    ProviderOwnerProbeV1, ProviderSearchWorkspaceSessionV1,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderOwnerBatchProbeRequestV1 {
    pub owner_path: String,
    pub metadata: ProviderOwnerMetadataV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderOwnerBatchProbeResultV1 {
    pub owner_path: String,
    pub probe: ProviderOwnerProbeV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderOwnerBatchProbeReceiptV1 {
    pub results: Vec<ProviderOwnerBatchProbeResultV1>,
    pub read_lock_count: u32,
    pub connection_open_count: u32,
    pub scope_scan_count: u32,
}

struct StoredOwner {
    metadata: ProviderOwnerMetadataV1,
    content_digest: String,
}

/// Classify a provider inventory through a process-resident workspace session.
pub(super) async fn probe_provider_owners_in_session_v1(
    session: &ProviderSearchWorkspaceSessionV1,
    scope: &ProviderIncrementalScopeV1,
    owners: &[ProviderOwnerBatchProbeRequestV1],
) -> Result<ProviderOwnerBatchProbeReceiptV1, String> {
    let generation = match read_active_generation(session.read_connection(), scope).await {
        Ok(generation) => generation,
        Err(error) if schema_is_missing(error.as_str()) => {
            return Ok(ProviderOwnerBatchProbeReceiptV1 {
                results: new_owner_results(owners, None),
                read_lock_count: 0,
                connection_open_count: 0,
                scope_scan_count: 0,
            });
        }
        Err(error) => return Err(error),
    };
    let stored = read_stored_owners(session.read_connection(), scope).await?;
    Ok(ProviderOwnerBatchProbeReceiptV1 {
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
    owners: &[ProviderOwnerBatchProbeRequestV1],
    generation_before: Option<String>,
) -> Vec<ProviderOwnerBatchProbeResultV1> {
    owners
        .iter()
        .map(|owner| ProviderOwnerBatchProbeResultV1 {
            owner_path: owner.owner_path.clone(),
            probe: ProviderOwnerProbeV1 {
                decision: ProviderOwnerDecisionV1::New,
                generation_before: generation_before.clone(),
                content_digest: None,
            },
        })
        .collect()
}

fn classify_owner(
    owner: &ProviderOwnerBatchProbeRequestV1,
    stored: Option<&StoredOwner>,
    generation_before: &Option<String>,
) -> ProviderOwnerBatchProbeResultV1 {
    let probe = match stored {
        None => ProviderOwnerProbeV1 {
            decision: ProviderOwnerDecisionV1::New,
            generation_before: generation_before.clone(),
            content_digest: None,
        },
        Some(stored) => ProviderOwnerProbeV1 {
            decision: if stored.metadata == owner.metadata {
                ProviderOwnerDecisionV1::Unchanged
            } else {
                ProviderOwnerDecisionV1::Changed
            },
            generation_before: generation_before.clone(),
            content_digest: Some(stored.content_digest.clone()),
        },
    };
    ProviderOwnerBatchProbeResultV1 {
        owner_path: owner.owner_path.clone(),
        probe,
    }
}

async fn read_active_generation(
    connection: &turso::Connection,
    scope: &ProviderIncrementalScopeV1,
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
    scope: &ProviderIncrementalScopeV1,
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
                metadata: ProviderOwnerMetadataV1 {
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
