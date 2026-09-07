// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Provider-scoped incremental reasoning-search state.
//!
//! This state is deliberately independent from the immutable full source-index
//! generations. Reasoning search may refresh one owner without materializing a
//! complete workspace snapshot.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::engine::turso_statement::run_turso_operation;

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Identity boundary for one provider's incremental reasoning-search state.
pub struct ProviderIncrementalScoped {
    pub project_root: String,
    pub workspace_identity: String,
    pub provider_workspace_identity_digest: String,
    pub language_id: String,
    pub provider_id: String,
    pub provider_workspace_root: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Metadata that can be inspected without reading owner source bytes.
pub struct ProviderOwnerMetadata {
    pub file_identity: String,
    pub size_bytes: u64,
    pub modified_unix_nanos: i64,
    pub change_time_unix_nanos: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Complete metadata and content identity persisted for one owner.
pub struct ProviderOwnerFingerprint {
    pub metadata: ProviderOwnerMetadata,
    pub content_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Parser-owned selector projection persisted for one owner item.
pub struct ProviderSelectorProjection {
    pub structural_selector: String,
    pub capture_name: String,
    pub signature: String,
    pub item_kind: String,
    pub item_name: String,
    pub source_byte_start: u64,
    pub source_byte_end: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// Incremental acquisition decision for one requested owner.
pub enum ProviderOwnerDecision {
    Unchanged,
    Changed,
    New,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Metadata probe result returned before any source read or provider parse.
pub struct ProviderOwnerProbe {
    pub decision: ProviderOwnerDecision,
    pub generation_before: Option<String>,
    pub content_digest: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Atomic replacement request for one provider owner.
pub struct ProviderIncrementalOwnerWrite {
    pub scope: ProviderIncrementalScoped,
    pub owner_path: String,
    pub fingerprint: ProviderOwnerFingerprint,
    pub source_bytes: Vec<u8>,
    pub projection_completeness: String,
    pub projections: Vec<ProviderSelectorProjection>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Durable complete-owner state used to restore one Runtime Server overlay.
pub struct ProviderIncrementalOwnerSnapshot {
    pub fingerprint: ProviderOwnerFingerprint,
    pub source_bytes: Vec<u8>,
    pub projections: Vec<ProviderSelectorProjection>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Committed write counts and generation transition for one owner replacement.
pub struct ProviderIncrementalWriteReceipt {
    pub generation_before: Option<String>,
    pub generation_after: String,
    pub owner_index_writes: u32,
    pub merkle_leaf_writes: u32,
    pub merkle_path_node_writes: u32,
}

pub(super) async fn read_provider_owner_projections(
    connection: &turso::Connection,
    scope: &ProviderIncrementalScoped,
    owner_path: &str,
) -> Result<Vec<ProviderSelectorProjection>, String> {
    let mut rows = run_turso_operation(
        || async {
            connection
                .query(
                    "SELECT structural_selector, capture_name, signature,
                            item_kind, item_name, source_byte_start, source_byte_end
                     FROM provider_selector_projection_v1
                     WHERE project_root = ?1
                       AND workspace_identity = ?2
                       AND provider_workspace_identity_digest = ?3
                       AND provider_id = ?4
                       AND owner_path = ?5
                     ORDER BY source_byte_start, source_byte_end, structural_selector",
                    (
                        scope.project_root.as_str(),
                        scope.workspace_identity.as_str(),
                        scope.provider_workspace_identity_digest.as_str(),
                        scope.provider_id.as_str(),
                        owner_path,
                    ),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to read provider selector projections",
    )
    .await?;
    let mut projections = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read provider selector projection row: {error}"))?
    {
        let source_byte_start = row
            .get::<i64>(5)
            .map_err(|error| format!("failed to decode provider projection start: {error}"))?;
        let source_byte_end = row
            .get::<i64>(6)
            .map_err(|error| format!("failed to decode provider projection end: {error}"))?;
        projections.push(ProviderSelectorProjection {
            structural_selector: row.get::<String>(0).map_err(|error| {
                format!("failed to decode provider projection selector: {error}")
            })?,
            capture_name: row.get::<String>(1).map_err(|error| {
                format!("failed to decode provider projection capture: {error}")
            })?,
            signature: row.get::<String>(2).map_err(|error| {
                format!("failed to decode provider projection signature: {error}")
            })?,
            item_kind: row
                .get::<String>(3)
                .map_err(|error| format!("failed to decode provider projection kind: {error}"))?,
            item_name: row
                .get::<String>(4)
                .map_err(|error| format!("failed to decode provider projection name: {error}"))?,
            source_byte_start: u64::try_from(source_byte_start).map_err(|_| {
                format!("provider projection start is out of range: {source_byte_start}")
            })?,
            source_byte_end: u64::try_from(source_byte_end).map_err(|_| {
                format!("provider projection end is out of range: {source_byte_end}")
            })?,
        });
    }
    Ok(projections)
}

pub(super) async fn read_provider_owner_snapshot(
    connection: &turso::Connection,
    scope: &ProviderIncrementalScoped,
    owner_path: &str,
) -> Result<Option<ProviderIncrementalOwnerSnapshot>, String> {
    let mut rows = run_turso_operation(
        || async {
            connection
                .query(
                    "SELECT file_identity, size_bytes, modified_unix_nanos,
                            change_time_unix_nanos, content_digest, source_bytes
                     FROM provider_owner_fingerprint_v1
                     WHERE project_root = ?1
                       AND workspace_identity = ?2
                       AND provider_workspace_identity_digest = ?3
                       AND provider_id = ?4
                       AND owner_path = ?5
                     LIMIT 1",
                    (
                        scope.project_root.as_str(),
                        scope.workspace_identity.as_str(),
                        scope.provider_workspace_identity_digest.as_str(),
                        scope.provider_id.as_str(),
                        owner_path,
                    ),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to read provider owner snapshot",
    )
    .await?;
    let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read provider owner snapshot row: {error}"))?
    else {
        return Ok(None);
    };
    let size_bytes = row
        .get::<i64>(1)
        .map_err(|error| format!("failed to decode provider owner size: {error}"))?;
    let source_bytes = row
        .get::<Vec<u8>>(5)
        .map_err(|error| format!("failed to decode provider owner bytes: {error}"))?;
    if source_bytes.is_empty() && size_bytes != 0 {
        return Ok(None);
    }
    Ok(Some(ProviderIncrementalOwnerSnapshot {
        fingerprint: ProviderOwnerFingerprint {
            metadata: ProviderOwnerMetadata {
                file_identity: row.get::<String>(0).map_err(|error| {
                    format!("failed to decode provider owner file identity: {error}")
                })?,
                size_bytes: u64::try_from(size_bytes)
                    .map_err(|_| format!("provider owner size is out of range: {size_bytes}"))?,
                modified_unix_nanos: row.get::<i64>(2).map_err(|error| {
                    format!("failed to decode provider owner modified time: {error}")
                })?,
                change_time_unix_nanos: row.get::<i64>(3).map_err(|error| {
                    format!("failed to decode provider owner change time: {error}")
                })?,
            },
            content_digest: row.get::<String>(4).map_err(|error| {
                format!("failed to decode provider owner content digest: {error}")
            })?,
        },
        source_bytes,
        projections: read_provider_owner_projections(connection, scope, owner_path).await?,
    }))
}

pub(super) async fn write_provider_incremental_owner_on_connection(
    connection: &mut turso::Connection,
    request: &ProviderIncrementalOwnerWrite,
) -> Result<ProviderIncrementalWriteReceipt, String> {
    if request.projection_completeness != "complete-owner" {
        return Err(format!(
            "provider incremental write requires projectionCompleteness=complete-owner, got {}",
            request.projection_completeness
        ));
    }
    let transaction = connection
        .transaction_with_behavior(turso::transaction::TransactionBehavior::Immediate)
        .await
        .map_err(|error| format!("failed to begin provider incremental transaction: {error}"))?;
    let result = write_provider_incremental_owner_transaction(&transaction, request).await;
    match result {
        Ok(mut receipt) => {
            transaction.commit().await.map_err(|error| {
                format!("failed to commit provider incremental transaction: {error}")
            })?;
            receipt.generation_after = active_provider_generation(&connection, &request.scope)
                .await?
                .ok_or_else(|| "committed provider generation is not visible".to_string())?;
            Ok(receipt)
        }
        Err(error) => match transaction.rollback().await {
            Ok(()) => Err(error),
            Err(rollback_error) => Err(format!("{error}; rollbackError={rollback_error}")),
        },
    }
}

async fn write_provider_incremental_owner_transaction(
    transaction: &turso::transaction::Transaction<'_>,
    request: &ProviderIncrementalOwnerWrite,
) -> Result<ProviderIncrementalWriteReceipt, String> {
    let connection = &**transaction;
    let scope = &request.scope;
    if request.source_bytes.len() as u64 != request.fingerprint.metadata.size_bytes {
        return Err("provider incremental owner bytes do not match fingerprint size".to_owned());
    }
    let source_digest =
        agent_semantic_content_identity::ArtifactHash::blake3(request.source_bytes.as_slice())
            .value;
    if source_digest != request.fingerprint.content_digest {
        return Err("provider incremental owner bytes do not match fingerprint digest".to_owned());
    }
    let generation_before = active_provider_generation(connection, scope).await?;
    run_turso_operation(
        || async {
            connection
                .execute(
                    "DELETE FROM provider_selector_projection_v1
                     WHERE project_root = ?1
                       AND workspace_identity = ?2
                       AND provider_workspace_identity_digest = ?3
                       AND provider_id = ?4
                       AND owner_path = ?5",
                    (
                        scope.project_root.as_str(),
                        scope.workspace_identity.as_str(),
                        scope.provider_workspace_identity_digest.as_str(),
                        scope.provider_id.as_str(),
                        request.owner_path.as_str(),
                    ),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to replace provider selector projections",
    )
    .await?;
    for projection in &request.projections {
        run_turso_operation(
            || async {
                connection
                    .execute(
                        "INSERT INTO provider_selector_projection_v1 (
                            project_root, workspace_identity,
                            provider_workspace_identity_digest, provider_id,
                            owner_path, source_content_digest, structural_selector,
                            capture_name, signature, item_kind, item_name,
                            source_byte_start, source_byte_end
                         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                        (
                            scope.project_root.as_str(),
                            scope.workspace_identity.as_str(),
                            scope.provider_workspace_identity_digest.as_str(),
                            scope.provider_id.as_str(),
                            request.owner_path.as_str(),
                            request.fingerprint.content_digest.as_str(),
                            projection.structural_selector.as_str(),
                            projection.capture_name.as_str(),
                            projection.signature.as_str(),
                            projection.item_kind.as_str(),
                            projection.item_name.as_str(),
                            i64::try_from(projection.source_byte_start).unwrap_or(i64::MAX),
                            i64::try_from(projection.source_byte_end).unwrap_or(i64::MAX),
                        ),
                    )
                    .await
                    .map_err(|error| error.to_string())
            },
            "failed to write provider selector projection",
        )
        .await?;
    }
    let metadata = &request.fingerprint.metadata;
    let leaf_digest = provider_owner_leaf_digest(
        request.owner_path.as_str(),
        request.fingerprint.content_digest.as_str(),
    );
    run_turso_operation(
        || async {
            connection
                .execute(
                    "INSERT INTO provider_owner_fingerprint_v1 (
                        project_root, workspace_identity,
                        provider_workspace_identity_digest, provider_id,
                        language_id, owner_path, file_identity, size_bytes,
                        modified_unix_nanos, change_time_unix_nanos,
                        content_digest, merkle_leaf_digest, source_bytes
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
                     ON CONFLICT (
                        project_root, workspace_identity,
                        provider_workspace_identity_digest, provider_id, owner_path
                     ) DO UPDATE SET
                        language_id = excluded.language_id,
                        file_identity = excluded.file_identity,
                        size_bytes = excluded.size_bytes,
                        modified_unix_nanos = excluded.modified_unix_nanos,
                        change_time_unix_nanos = excluded.change_time_unix_nanos,
                        content_digest = excluded.content_digest,
                        merkle_leaf_digest = excluded.merkle_leaf_digest,
                        source_bytes = excluded.source_bytes",
                    (
                        scope.project_root.as_str(),
                        scope.workspace_identity.as_str(),
                        scope.provider_workspace_identity_digest.as_str(),
                        scope.provider_id.as_str(),
                        scope.language_id.as_str(),
                        request.owner_path.as_str(),
                        metadata.file_identity.as_str(),
                        i64::try_from(metadata.size_bytes).unwrap_or(i64::MAX),
                        metadata.modified_unix_nanos,
                        metadata.change_time_unix_nanos,
                        request.fingerprint.content_digest.as_str(),
                        leaf_digest.as_str(),
                        request.source_bytes.as_slice(),
                    ),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to write provider owner fingerprint",
    )
    .await?;

    let leaf_key = format!("leaf:{}", request.owner_path);
    let parent_directory = owner_parent_directory(request.owner_path.as_str());
    let leaf_parent_key = format!("dir:{parent_directory}");
    upsert_merkle_node(
        connection,
        scope,
        leaf_key.as_str(),
        leaf_parent_key.as_str(),
        "leaf",
        leaf_digest.as_str(),
    )
    .await?;
    let mut merkle_path_node_writes = 0_u32;
    for directory in owner_directory_ancestors(request.owner_path.as_str()) {
        let node_key = format!("dir:{directory}");
        let parent_key = directory_parent_key(directory.as_str());
        let digest = digest_merkle_children(connection, scope, node_key.as_str()).await?;
        upsert_merkle_node(
            connection,
            scope,
            node_key.as_str(),
            parent_key.as_str(),
            "directory",
            digest.as_str(),
        )
        .await?;
        merkle_path_node_writes = merkle_path_node_writes.saturating_add(1);
    }
    let generation_after = merkle_node_digest(connection, scope, "dir:")
        .await?
        .ok_or_else(|| "provider incremental root node was not written".to_string())?;
    let owner_count = provider_owner_count(connection, scope).await?;
    run_turso_operation(
        || async {
            connection
                .execute(
                    "INSERT INTO provider_active_generation_v1 (
                        project_root, workspace_identity,
                        provider_workspace_identity_digest, provider_id,
                        language_id, provider_workspace_root, generation_id,
                        owner_count, updated_at_ms
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                     ON CONFLICT (
                        project_root, workspace_identity,
                        provider_workspace_identity_digest, provider_id
                     ) DO UPDATE SET
                        language_id = excluded.language_id,
                        provider_workspace_root = excluded.provider_workspace_root,
                        generation_id = excluded.generation_id,
                        owner_count = excluded.owner_count,
                        updated_at_ms = excluded.updated_at_ms",
                    (
                        scope.project_root.as_str(),
                        scope.workspace_identity.as_str(),
                        scope.provider_workspace_identity_digest.as_str(),
                        scope.provider_id.as_str(),
                        scope.language_id.as_str(),
                        scope.provider_workspace_root.as_str(),
                        generation_after.as_str(),
                        i64::from(owner_count),
                        unix_time_ms(),
                    ),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to publish provider active generation",
    )
    .await?;
    Ok(ProviderIncrementalWriteReceipt {
        generation_before,
        generation_after,
        owner_index_writes: 1,
        merkle_leaf_writes: 1,
        merkle_path_node_writes,
    })
}

async fn active_provider_generation(
    connection: &turso::Connection,
    scope: &ProviderIncrementalScoped,
) -> Result<Option<String>, String> {
    let mut rows = run_turso_operation(
        || async {
            connection
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
                .map_err(|error| error.to_string())
        },
        "failed to read provider active generation",
    )
    .await?;
    let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read provider active generation row: {error}"))?
    else {
        return Ok(None);
    };
    row.get::<String>(0)
        .map(Some)
        .map_err(|error| format!("failed to decode provider active generation: {error}"))
}

async fn provider_owner_count(
    connection: &turso::Connection,
    scope: &ProviderIncrementalScoped,
) -> Result<u32, String> {
    let mut rows = run_turso_operation(
        || async {
            connection
                .query(
                    "SELECT COUNT(*)
                     FROM provider_owner_fingerprint_v1
                     WHERE project_root = ?1
                       AND workspace_identity = ?2
                       AND provider_workspace_identity_digest = ?3
                       AND provider_id = ?4",
                    (
                        scope.project_root.as_str(),
                        scope.workspace_identity.as_str(),
                        scope.provider_workspace_identity_digest.as_str(),
                        scope.provider_id.as_str(),
                    ),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to count provider owners",
    )
    .await?;
    let row = rows
        .next()
        .await
        .map_err(|error| format!("failed to read provider owner count: {error}"))?
        .ok_or_else(|| "provider owner count query returned no row".to_string())?;
    let count = row
        .get::<i64>(0)
        .map_err(|error| format!("failed to decode provider owner count: {error}"))?;
    u32::try_from(count).map_err(|_| format!("provider owner count is out of range: {count}"))
}

async fn upsert_merkle_node(
    connection: &turso::Connection,
    scope: &ProviderIncrementalScoped,
    node_key: &str,
    parent_node_key: &str,
    node_kind: &str,
    digest: &str,
) -> Result<(), String> {
    run_turso_operation(
        || async {
            connection
                .execute(
                    "INSERT INTO provider_merkle_node_v1 (
                        project_root, workspace_identity,
                        provider_workspace_identity_digest, provider_id,
                        node_key, parent_node_key, node_kind, digest
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                     ON CONFLICT (
                        project_root, workspace_identity,
                        provider_workspace_identity_digest, provider_id, node_key
                     ) DO UPDATE SET
                        parent_node_key = excluded.parent_node_key,
                        node_kind = excluded.node_kind,
                        digest = excluded.digest",
                    (
                        scope.project_root.as_str(),
                        scope.workspace_identity.as_str(),
                        scope.provider_workspace_identity_digest.as_str(),
                        scope.provider_id.as_str(),
                        node_key,
                        parent_node_key,
                        node_kind,
                        digest,
                    ),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to write provider Merkle node",
    )
    .await?;
    Ok(())
}

async fn digest_merkle_children(
    connection: &turso::Connection,
    scope: &ProviderIncrementalScoped,
    parent_node_key: &str,
) -> Result<String, String> {
    let mut rows = run_turso_operation(
        || async {
            connection
                .query(
                    "SELECT node_key, digest
                     FROM provider_merkle_node_v1
                     WHERE project_root = ?1
                       AND workspace_identity = ?2
                       AND provider_workspace_identity_digest = ?3
                       AND provider_id = ?4
                       AND parent_node_key = ?5
                     ORDER BY node_key",
                    (
                        scope.project_root.as_str(),
                        scope.workspace_identity.as_str(),
                        scope.provider_workspace_identity_digest.as_str(),
                        scope.provider_id.as_str(),
                        parent_node_key,
                    ),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to read provider Merkle children",
    )
    .await?;
    let mut hasher = Sha256::new();
    hasher.update(b"asp.provider-merkle-directory.v1\0");
    hasher.update(parent_node_key.as_bytes());
    while let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read provider Merkle child: {error}"))?
    {
        let node_key = row
            .get::<String>(0)
            .map_err(|error| format!("failed to decode provider Merkle child key: {error}"))?;
        let digest = row
            .get::<String>(1)
            .map_err(|error| format!("failed to decode provider Merkle child digest: {error}"))?;
        update_hash_text(&mut hasher, node_key.as_str());
        update_hash_text(&mut hasher, digest.as_str());
    }
    Ok(format!("{:x}", hasher.finalize()))
}

async fn merkle_node_digest(
    connection: &turso::Connection,
    scope: &ProviderIncrementalScoped,
    node_key: &str,
) -> Result<Option<String>, String> {
    let mut rows = run_turso_operation(
        || async {
            connection
                .query(
                    "SELECT digest
                     FROM provider_merkle_node_v1
                     WHERE project_root = ?1
                       AND workspace_identity = ?2
                       AND provider_workspace_identity_digest = ?3
                       AND provider_id = ?4
                       AND node_key = ?5
                     LIMIT 1",
                    (
                        scope.project_root.as_str(),
                        scope.workspace_identity.as_str(),
                        scope.provider_workspace_identity_digest.as_str(),
                        scope.provider_id.as_str(),
                        node_key,
                    ),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to read provider Merkle node",
    )
    .await?;
    let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read provider Merkle node row: {error}"))?
    else {
        return Ok(None);
    };
    row.get::<String>(0)
        .map(Some)
        .map_err(|error| format!("failed to decode provider Merkle node digest: {error}"))
}

fn provider_owner_leaf_digest(owner_path: &str, content_digest: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"asp.provider-merkle-leaf.v1\0");
    update_hash_text(&mut hasher, owner_path);
    update_hash_text(&mut hasher, content_digest);
    format!("{:x}", hasher.finalize())
}

fn owner_parent_directory(owner_path: &str) -> String {
    owner_path
        .rsplit_once('/')
        .map_or_else(String::new, |(parent, _)| parent.to_string())
}

fn owner_directory_ancestors(owner_path: &str) -> Vec<String> {
    let mut directories = Vec::new();
    let mut current = owner_parent_directory(owner_path);
    loop {
        directories.push(current.clone());
        if current.is_empty() {
            break;
        }
        current = current
            .rsplit_once('/')
            .map_or_else(String::new, |(parent, _)| parent.to_string());
    }
    directories
}

fn directory_parent_key(directory: &str) -> String {
    if directory.is_empty() {
        String::new()
    } else {
        let parent = directory
            .rsplit_once('/')
            .map_or_else(String::new, |(parent, _)| parent.to_string());
        format!("dir:{parent}")
    }
}

fn update_hash_text(hasher: &mut Sha256, value: &str) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value.as_bytes());
}

fn unix_time_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}

#[cfg(test)]
#[path = "../../../tests/unit/provider_incremental_search.rs"]
mod tests;
