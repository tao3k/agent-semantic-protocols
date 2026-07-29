//! Atomic Turso 0.7 writers for provider Tree-sitter inventory and query caches.

use std::collections::BTreeSet;

use sha2::{Digest, Sha256};

use super::provider_incremental::ProviderIncrementalScoped;
use super::provider_treesitter::{
    ProviderOwnerInventoryEntry, ProviderOwnerInventoryEntryState, ProviderOwnerInventoryState,
    ProviderOwnerInventoryWrite, ProviderOwnerInventoryWriteReceipt,
    ProviderTreeSitterCaptureProjection, ProviderTreeSitterOwnerResult,
    ProviderTreeSitterOwnerResultState, ProviderTreeSitterOwnerWriteReceipt,
    ProviderTreeSitterQueryIdentity,
};
pub(super) async fn upsert_provider_owner_inventory_on_connection(
    connection: &mut turso::Connection,
    request: &ProviderOwnerInventoryWrite,
) -> Result<ProviderOwnerInventoryWriteReceipt, String> {
    let inventory_digest = inventory_digest(request);
    let inventory_generation = inventory_generation(&request.scope, inventory_digest.as_str());
    let current_paths = request
        .entries
        .iter()
        .map(|entry| entry.owner_path.clone())
        .collect::<BTreeSet<_>>();
    let transaction = connection
        .transaction_with_behavior(turso::transaction::TransactionBehavior::Immediate)
        .await
        .map_err(|error| format!("failed to begin provider inventory transaction: {error}"))?;
    let previous_paths = read_inventory_paths(&transaction, &request.scope).await?;
    let write = replace_inventory_transaction(
        &transaction,
        request,
        inventory_digest.as_str(),
        inventory_generation.as_str(),
    )
    .await;
    let receipt = match write {
        Ok(()) => {
            transaction.commit().await.map_err(|error| {
                format!("failed to commit provider inventory transaction: {error}")
            })?;
            let deleted_entry_count = previous_paths
                .difference(&current_paths)
                .count()
                .try_into()
                .unwrap_or(u32::MAX);
            ProviderOwnerInventoryWriteReceipt {
                inventory_digest,
                inventory_generation,
                upserted_entry_count: request.entries.len().try_into().unwrap_or(u32::MAX),
                deleted_entry_count,
            }
        }
        Err(error) => {
            transaction.rollback().await.map_err(|rollback_error| {
                format!("{error}; inventoryRollbackError={rollback_error}")
            })?;
            return Err(error);
        }
    };
    verify_inventory_visibility(&connection, request, &receipt).await?;
    Ok(receipt)
}

pub(super) async fn write_provider_treesitter_owner_result_on_connection(
    connection: &mut turso::Connection,
    query: &ProviderTreeSitterQueryIdentity,
    result: &ProviderTreeSitterOwnerResult,
) -> Result<ProviderTreeSitterOwnerWriteReceipt, String> {
    let capture_names = canonical_capture_names(&query.capture_names);
    let capture_names_json = serde_json::to_string(&capture_names)
        .map_err(|error| format!("failed to encode Tree-sitter capture names: {error}"))?;
    let transaction = connection
        .transaction_with_behavior(turso::transaction::TransactionBehavior::Immediate)
        .await
        .map_err(|error| format!("failed to begin Tree-sitter owner transaction: {error}"))?;
    let write =
        replace_query_owner_transaction(&transaction, query, result, capture_names_json.as_str())
            .await;
    match write {
        Ok(()) => transaction
            .commit()
            .await
            .map_err(|error| format!("failed to commit Tree-sitter owner transaction: {error}"))?,
        Err(error) => {
            transaction.rollback().await.map_err(|rollback_error| {
                format!("{error}; ownerResultRollbackError={rollback_error}")
            })?;
            return Err(error);
        }
    }
    verify_query_owner_visibility(&connection, query, result).await?;
    Ok(ProviderTreeSitterOwnerWriteReceipt {
        query_digest: query.query_digest.clone(),
        owner_path: result.owner_path.clone(),
        owner_content_digest: result.owner_content_digest.clone(),
        query_metadata_writes: 1,
        owner_result_writes: 1,
        capture_projection_writes: result.projections.len().try_into().unwrap_or(u32::MAX),
    })
}

async fn replace_inventory_transaction(
    connection: &turso::Connection,
    request: &ProviderOwnerInventoryWrite,
    inventory_digest: &str,
    inventory_generation: &str,
) -> Result<(), String> {
    delete_inventory_entries(connection, &request.scope).await?;
    let mut entries = request.entries.iter().collect::<Vec<_>>();
    entries.sort_by(|left, right| left.owner_path.cmp(&right.owner_path));
    for entry in entries {
        insert_inventory_entry(connection, &request.scope, inventory_generation, entry).await?;
    }
    connection
        .execute(
            "INSERT INTO provider_owner_inventory_v1 (
                project_root, workspace_identity,
                provider_workspace_identity_digest, provider_id,
                language_id, provider_workspace_root, inventory_state,
                inventory_digest, inventory_generation, known_owner_count,
                updated_at_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT (
                project_root, workspace_identity,
                provider_workspace_identity_digest, provider_id
             ) DO UPDATE SET
                language_id = excluded.language_id,
                provider_workspace_root = excluded.provider_workspace_root,
                inventory_state = excluded.inventory_state,
                inventory_digest = excluded.inventory_digest,
                inventory_generation = excluded.inventory_generation,
                known_owner_count = excluded.known_owner_count,
                updated_at_ms = excluded.updated_at_ms",
            (
                request.scope.project_root.as_str(),
                request.scope.workspace_identity.as_str(),
                request.scope.provider_workspace_identity_digest.as_str(),
                request.scope.provider_id.as_str(),
                request.scope.language_id.as_str(),
                request.scope.provider_workspace_root.as_str(),
                inventory_state_text(request.state),
                inventory_digest,
                inventory_generation,
                i64::try_from(request.entries.len()).unwrap_or(i64::MAX),
                unix_time_ms(),
            ),
        )
        .await
        .map_err(|error| format!("failed to publish provider owner inventory: {error}"))?;
    Ok(())
}

async fn delete_inventory_entries(
    connection: &turso::Connection,
    scope: &ProviderIncrementalScoped,
) -> Result<(), String> {
    connection
        .execute(
            "DELETE FROM provider_owner_inventory_entry_v1
             WHERE project_root = ?1
               AND workspace_identity = ?2
               AND provider_workspace_identity_digest = ?3
               AND provider_id = ?4",
            scope_params(scope),
        )
        .await
        .map_err(|error| format!("failed to replace provider inventory entries: {error}"))?;
    Ok(())
}

async fn insert_inventory_entry(
    connection: &turso::Connection,
    scope: &ProviderIncrementalScoped,
    inventory_generation: &str,
    entry: &ProviderOwnerInventoryEntry,
) -> Result<(), String> {
    connection
        .execute(
            "INSERT INTO provider_owner_inventory_entry_v1 (
                project_root, workspace_identity,
                provider_workspace_identity_digest, provider_id,
                inventory_generation, owner_path, owner_content_digest,
                owner_state
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            (
                scope.project_root.as_str(),
                scope.workspace_identity.as_str(),
                scope.provider_workspace_identity_digest.as_str(),
                scope.provider_id.as_str(),
                inventory_generation,
                entry.owner_path.as_str(),
                entry.owner_content_digest.as_deref(),
                inventory_entry_state_text(entry.state),
            ),
        )
        .await
        .map_err(|error| format!("failed to write provider inventory entry: {error}"))?;
    Ok(())
}

async fn replace_query_owner_transaction(
    connection: &turso::Connection,
    query: &ProviderTreeSitterQueryIdentity,
    result: &ProviderTreeSitterOwnerResult,
    capture_names_json: &str,
) -> Result<(), String> {
    ensure_query_metadata(connection, query, capture_names_json).await?;
    delete_owner_captures(connection, query, result).await?;
    upsert_owner_result(connection, query, result).await?;
    for projection in &result.projections {
        insert_capture_projection(connection, query, result, projection).await?;
    }
    Ok(())
}

async fn ensure_query_metadata(
    connection: &turso::Connection,
    query: &ProviderTreeSitterQueryIdentity,
    capture_names_json: &str,
) -> Result<(), String> {
    let existing = read_query_capture_names(connection, query).await?;
    if existing
        .as_deref()
        .is_some_and(|stored| stored != capture_names_json)
    {
        return Err(
            "Tree-sitter query metadata drift for the same provider/query digest".to_string(),
        );
    }
    connection
        .execute(
            "INSERT INTO provider_treesitter_query_v1 (
                project_root, workspace_identity,
                provider_workspace_identity_digest, provider_id,
                query_digest, capture_names_json, updated_at_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT (
                project_root, workspace_identity,
                provider_workspace_identity_digest, provider_id, query_digest
             ) DO UPDATE SET updated_at_ms = excluded.updated_at_ms",
            (
                query.scope.project_root.as_str(),
                query.scope.workspace_identity.as_str(),
                query.scope.provider_workspace_identity_digest.as_str(),
                query.scope.provider_id.as_str(),
                query.query_digest.as_str(),
                capture_names_json,
                unix_time_ms(),
            ),
        )
        .await
        .map_err(|error| format!("failed to write Tree-sitter query metadata: {error}"))?;
    Ok(())
}

async fn delete_owner_captures(
    connection: &turso::Connection,
    query: &ProviderTreeSitterQueryIdentity,
    result: &ProviderTreeSitterOwnerResult,
) -> Result<(), String> {
    connection
        .execute(
            "DELETE FROM provider_treesitter_capture_projection_v1
             WHERE project_root = ?1
               AND workspace_identity = ?2
               AND provider_workspace_identity_digest = ?3
               AND provider_id = ?4
               AND query_digest = ?5
               AND owner_path = ?6
               AND owner_content_digest = ?7",
            (
                query.scope.project_root.as_str(),
                query.scope.workspace_identity.as_str(),
                query.scope.provider_workspace_identity_digest.as_str(),
                query.scope.provider_id.as_str(),
                query.query_digest.as_str(),
                result.owner_path.as_str(),
                result.owner_content_digest.as_str(),
            ),
        )
        .await
        .map_err(|error| format!("failed to replace Tree-sitter owner captures: {error}"))?;
    Ok(())
}

async fn upsert_owner_result(
    connection: &turso::Connection,
    query: &ProviderTreeSitterQueryIdentity,
    result: &ProviderTreeSitterOwnerResult,
) -> Result<(), String> {
    connection
        .execute(
            "INSERT INTO provider_treesitter_query_owner_v1 (
                project_root, workspace_identity,
                provider_workspace_identity_digest, provider_id,
                query_digest, owner_path, owner_content_digest,
                inventory_generation, capture_count,
                complete_owner_refresh_count, processed_at_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT (
                project_root, workspace_identity,
                provider_workspace_identity_digest, provider_id,
                query_digest, owner_path, owner_content_digest
             ) DO UPDATE SET
                inventory_generation = excluded.inventory_generation,
                capture_count = excluded.capture_count,
                complete_owner_refresh_count = excluded.complete_owner_refresh_count,
                processed_at_ms = excluded.processed_at_ms",
            (
                query.scope.project_root.as_str(),
                query.scope.workspace_identity.as_str(),
                query.scope.provider_workspace_identity_digest.as_str(),
                query.scope.provider_id.as_str(),
                query.query_digest.as_str(),
                result.owner_path.as_str(),
                result.owner_content_digest.as_str(),
                result.inventory_generation.as_str(),
                i64::try_from(result.projections.len()).unwrap_or(i64::MAX),
                i64::from(result.complete_owner_refresh_count),
                unix_time_ms(),
            ),
        )
        .await
        .map_err(|error| format!("failed to write Tree-sitter owner result: {error}"))?;
    Ok(())
}

async fn insert_capture_projection(
    connection: &turso::Connection,
    query: &ProviderTreeSitterQueryIdentity,
    result: &ProviderTreeSitterOwnerResult,
    projection: &ProviderTreeSitterCaptureProjection,
) -> Result<(), String> {
    connection
        .execute(
            "INSERT INTO provider_treesitter_capture_projection_v1 (
                project_root, workspace_identity,
                provider_workspace_identity_digest, provider_id,
                query_digest, owner_path, owner_content_digest,
                structural_selector, signature, item_kind, item_name,
                capture_name, item_source_byte_start, item_source_byte_end,
                source_byte_start, source_byte_end
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
                ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16
             )",
            (
                query.scope.project_root.as_str(),
                query.scope.workspace_identity.as_str(),
                query.scope.provider_workspace_identity_digest.as_str(),
                query.scope.provider_id.as_str(),
                query.query_digest.as_str(),
                result.owner_path.as_str(),
                result.owner_content_digest.as_str(),
                projection.structural_selector.as_str(),
                projection.signature.as_str(),
                projection.item_kind.as_str(),
                projection.item_name.as_str(),
                projection.capture_name.as_str(),
                i64::try_from(projection.item_source_byte_start).unwrap_or(i64::MAX),
                i64::try_from(projection.item_source_byte_end).unwrap_or(i64::MAX),
                i64::try_from(projection.source_byte_start).unwrap_or(i64::MAX),
                i64::try_from(projection.source_byte_end).unwrap_or(i64::MAX),
            ),
        )
        .await
        .map_err(|error| format!("failed to write Tree-sitter capture projection: {error}"))?;
    Ok(())
}

pub(super) fn validate_inventory_write(
    request: &ProviderOwnerInventoryWrite,
) -> Result<(), String> {
    validate_scope(&request.scope)?;
    let mut owner_paths = BTreeSet::new();
    for entry in &request.entries {
        validate_owner_path(entry.owner_path.as_str())?;
        if !owner_paths.insert(entry.owner_path.as_str()) {
            return Err(format!(
                "provider owner inventory contains duplicate owner {}",
                entry.owner_path
            ));
        }
        match entry.state {
            ProviderOwnerInventoryEntryState::Indexed if entry.owner_content_digest.is_none() => {
                return Err(format!(
                    "indexed inventory owner {} requires a content digest",
                    entry.owner_path
                ));
            }
            ProviderOwnerInventoryEntryState::Unindexed if entry.owner_content_digest.is_some() => {
                return Err(format!(
                    "unindexed inventory owner {} must not claim a content digest",
                    entry.owner_path
                ));
            }
            _ => {}
        }
        if let Some(digest) = entry.owner_content_digest.as_deref() {
            validate_digest("ownerContentDigest", digest)?;
        }
    }
    Ok(())
}

pub(super) fn validate_query_owner_write(
    query: &ProviderTreeSitterQueryIdentity,
    result: &ProviderTreeSitterOwnerResult,
) -> Result<(), String> {
    validate_scope(&query.scope)?;
    validate_digest("queryDigest", query.query_digest.as_str())?;
    validate_digest("ownerContentDigest", result.owner_content_digest.as_str())?;
    validate_digest("inventoryGeneration", result.inventory_generation.as_str())?;
    validate_owner_path(result.owner_path.as_str())?;
    if result.query_digest != query.query_digest {
        return Err("Tree-sitter owner result query digest drift".to_string());
    }
    if result.state != ProviderTreeSitterOwnerResultState::Processed {
        return Err("Tree-sitter writer accepts only processed owner results".to_string());
    }
    if result.complete_owner_refresh_count > 1 {
        return Err("Tree-sitter owner result refresh count exceeds one".to_string());
    }
    let capture_names = canonical_capture_names(&query.capture_names);
    if capture_names.is_empty() || capture_names.iter().any(String::is_empty) {
        return Err("Tree-sitter query requires non-empty capture names".to_string());
    }
    if capture_names.len() != query.capture_names.len() {
        return Err("Tree-sitter query capture names must be unique".to_string());
    }
    let selector_prefix = format!("{}://{}#item/", query.scope.language_id, result.owner_path);
    let mut capture_keys = BTreeSet::new();
    for projection in &result.projections {
        validate_capture_projection(
            projection,
            selector_prefix.as_str(),
            capture_names.as_slice(),
        )?;
        let key = (
            projection.capture_name.as_str(),
            projection.source_byte_start,
            projection.source_byte_end,
            projection.structural_selector.as_str(),
        );
        if !capture_keys.insert(key) {
            return Err(
                "Tree-sitter owner result contains duplicate capture projection".to_string(),
            );
        }
    }
    Ok(())
}

fn validate_capture_projection(
    projection: &ProviderTreeSitterCaptureProjection,
    selector_prefix: &str,
    capture_names: &[String],
) -> Result<(), String> {
    if projection.signature.is_empty()
        || projection.item_kind.is_empty()
        || projection.item_name.is_empty()
        || projection.capture_name.is_empty()
        || !projection.structural_selector.starts_with(selector_prefix)
    {
        return Err("Tree-sitter capture projection identity drift".to_string());
    }
    if capture_names
        .binary_search(&projection.capture_name)
        .is_err()
    {
        return Err("Tree-sitter capture is not declared by query metadata".to_string());
    }
    if projection.item_source_byte_start > projection.source_byte_start
        || projection.source_byte_start >= projection.source_byte_end
        || projection.source_byte_end > projection.item_source_byte_end
    {
        return Err("Tree-sitter capture span is outside its containing item".to_string());
    }
    Ok(())
}

fn validate_scope(scope: &ProviderIncrementalScoped) -> Result<(), String> {
    for (field, value) in [
        ("projectRoot", scope.project_root.as_str()),
        ("workspaceIdentity", scope.workspace_identity.as_str()),
        ("languageId", scope.language_id.as_str()),
        ("providerId", scope.provider_id.as_str()),
        (
            "providerWorkspaceRoot",
            scope.provider_workspace_root.as_str(),
        ),
    ] {
        if value.is_empty() {
            return Err(format!("provider Tree-sitter scope requires {field}"));
        }
    }
    validate_digest(
        "providerWorkspaceIdentityDigest",
        scope.provider_workspace_identity_digest.as_str(),
    )
}

fn validate_owner_path(owner_path: &str) -> Result<(), String> {
    if owner_path.is_empty()
        || owner_path.starts_with('/')
        || owner_path.contains('\\')
        || owner_path
            .split('/')
            .any(|part| part.is_empty() || part == "..")
    {
        return Err(format!("invalid provider owner path {owner_path:?}"));
    }
    Ok(())
}

fn validate_digest(field: &str, digest: &str) -> Result<(), String> {
    if digest.len() == 64
        && digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(format!("{field} must be a lowercase 256-bit hex digest"))
    }
}

fn inventory_digest(request: &ProviderOwnerInventoryWrite) -> String {
    let mut entries = request.entries.iter().collect::<Vec<_>>();
    entries.sort_by(|left, right| left.owner_path.cmp(&right.owner_path));
    let mut hasher = Sha256::new();
    hasher.update(b"asp.provider-owner-inventory.v1\0");
    hash_text(&mut hasher, inventory_state_text(request.state));
    for entry in entries {
        hash_text(&mut hasher, entry.owner_path.as_str());
        hash_text(
            &mut hasher,
            entry.owner_content_digest.as_deref().unwrap_or(""),
        );
        hash_text(&mut hasher, inventory_entry_state_text(entry.state));
    }
    format!("{:x}", hasher.finalize())
}

fn inventory_generation(scope: &ProviderIncrementalScoped, inventory_digest: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"asp.provider-owner-inventory-generation.v1\0");
    for value in [
        scope.project_root.as_str(),
        scope.workspace_identity.as_str(),
        scope.provider_workspace_identity_digest.as_str(),
        scope.provider_id.as_str(),
        inventory_digest,
    ] {
        hash_text(&mut hasher, value);
    }
    format!("{:x}", hasher.finalize())
}

fn hash_text(hasher: &mut Sha256, value: &str) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value.as_bytes());
}

fn inventory_state_text(state: ProviderOwnerInventoryState) -> &'static str {
    match state {
        ProviderOwnerInventoryState::Exact => "exact",
        ProviderOwnerInventoryState::Known => "known",
    }
}

fn inventory_entry_state_text(state: ProviderOwnerInventoryEntryState) -> &'static str {
    match state {
        ProviderOwnerInventoryEntryState::Indexed => "indexed",
        ProviderOwnerInventoryEntryState::Dirty => "dirty",
        ProviderOwnerInventoryEntryState::Unindexed => "unindexed",
    }
}

fn canonical_capture_names(capture_names: &[String]) -> Vec<String> {
    let mut names = capture_names.to_vec();
    names.sort();
    names.dedup();
    names
}

async fn read_inventory_paths(
    connection: &turso::Connection,
    scope: &ProviderIncrementalScoped,
) -> Result<BTreeSet<String>, String> {
    let mut rows = connection
        .query(
            "SELECT owner_path
             FROM provider_owner_inventory_entry_v1
             WHERE project_root = ?1
               AND workspace_identity = ?2
               AND provider_workspace_identity_digest = ?3
               AND provider_id = ?4",
            scope_params(scope),
        )
        .await
        .map_err(|error| format!("failed to read previous provider inventory: {error}"))?;
    let mut paths = BTreeSet::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to iterate previous provider inventory: {error}"))?
    {
        let path = row
            .get::<String>(0)
            .map_err(|error| format!("failed to decode previous inventory owner: {error}"))?;
        paths.insert(path);
    }
    Ok(paths)
}

async fn read_query_capture_names(
    connection: &turso::Connection,
    query: &ProviderTreeSitterQueryIdentity,
) -> Result<Option<String>, String> {
    let mut rows = connection
        .query(
            "SELECT capture_names_json
             FROM provider_treesitter_query_v1
             WHERE project_root = ?1
               AND workspace_identity = ?2
               AND provider_workspace_identity_digest = ?3
               AND provider_id = ?4
               AND query_digest = ?5
             LIMIT 1",
            (
                query.scope.project_root.as_str(),
                query.scope.workspace_identity.as_str(),
                query.scope.provider_workspace_identity_digest.as_str(),
                query.scope.provider_id.as_str(),
                query.query_digest.as_str(),
            ),
        )
        .await
        .map_err(|error| format!("failed to inspect Tree-sitter query metadata: {error}"))?;
    let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Tree-sitter query metadata: {error}"))?
    else {
        return Ok(None);
    };
    row.get::<String>(0)
        .map(Some)
        .map_err(|error| format!("failed to decode Tree-sitter query metadata: {error}"))
}

async fn verify_inventory_visibility(
    connection: &turso::Connection,
    request: &ProviderOwnerInventoryWrite,
    receipt: &ProviderOwnerInventoryWriteReceipt,
) -> Result<(), String> {
    let mut rows = connection
        .query(
            "SELECT inventory_digest, inventory_generation, known_owner_count
             FROM provider_owner_inventory_v1
             WHERE project_root = ?1
               AND workspace_identity = ?2
               AND provider_workspace_identity_digest = ?3
               AND provider_id = ?4
             LIMIT 1",
            scope_params(&request.scope),
        )
        .await
        .map_err(|error| format!("failed to verify provider inventory visibility: {error}"))?;
    let row = rows
        .next()
        .await
        .map_err(|error| format!("failed to read committed provider inventory: {error}"))?
        .ok_or_else(|| "committed provider inventory is not visible".to_string())?;
    let digest = row
        .get::<String>(0)
        .map_err(|error| format!("failed to decode committed inventory digest: {error}"))?;
    let generation = row
        .get::<String>(1)
        .map_err(|error| format!("failed to decode committed inventory generation: {error}"))?;
    let owner_count = row
        .get::<i64>(2)
        .map_err(|error| format!("failed to decode committed inventory count: {error}"))?;
    if digest != receipt.inventory_digest
        || generation != receipt.inventory_generation
        || owner_count != i64::try_from(request.entries.len()).unwrap_or(i64::MAX)
    {
        return Err("committed provider inventory visibility drift".to_string());
    }
    Ok(())
}

async fn verify_query_owner_visibility(
    connection: &turso::Connection,
    query: &ProviderTreeSitterQueryIdentity,
    result: &ProviderTreeSitterOwnerResult,
) -> Result<(), String> {
    let mut rows = connection
        .query(
            "SELECT capture_count
             FROM provider_treesitter_query_owner_v1
             WHERE project_root = ?1
               AND workspace_identity = ?2
               AND provider_workspace_identity_digest = ?3
               AND provider_id = ?4
               AND query_digest = ?5
               AND owner_path = ?6
               AND owner_content_digest = ?7
             LIMIT 1",
            (
                query.scope.project_root.as_str(),
                query.scope.workspace_identity.as_str(),
                query.scope.provider_workspace_identity_digest.as_str(),
                query.scope.provider_id.as_str(),
                query.query_digest.as_str(),
                result.owner_path.as_str(),
                result.owner_content_digest.as_str(),
            ),
        )
        .await
        .map_err(|error| format!("failed to verify Tree-sitter owner visibility: {error}"))?;
    let row = rows
        .next()
        .await
        .map_err(|error| format!("failed to read committed Tree-sitter owner result: {error}"))?
        .ok_or_else(|| "committed Tree-sitter owner result is not visible".to_string())?;
    let capture_count = row
        .get::<i64>(0)
        .map_err(|error| format!("failed to decode committed capture count: {error}"))?;
    if capture_count != i64::try_from(result.projections.len()).unwrap_or(i64::MAX) {
        return Err("committed Tree-sitter capture count drift".to_string());
    }
    Ok(())
}

fn scope_params(scope: &ProviderIncrementalScoped) -> (&str, &str, &str, &str) {
    (
        scope.project_root.as_str(),
        scope.workspace_identity.as_str(),
        scope.provider_workspace_identity_digest.as_str(),
        scope.provider_id.as_str(),
    )
}

fn unix_time_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}

#[cfg(test)]
#[path = "../../../tests/unit/provider_treesitter_write.rs"]
mod tests;
