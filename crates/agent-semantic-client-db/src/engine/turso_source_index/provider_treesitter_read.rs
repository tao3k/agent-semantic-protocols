use std::collections::BTreeMap;

use super::{
    ProviderOwnerInventoryEntryStateV1, ProviderOwnerInventoryEntryV1,
    ProviderOwnerInventoryStateV1, ProviderOwnerInventoryV1, ProviderRemainingOwnerCountKindV1,
    ProviderTreeSitterCaptureProjectionV1, ProviderTreeSitterContinuationV1,
    ProviderTreeSitterOwnerResultStateV1, ProviderTreeSitterOwnerResultV1,
    ProviderTreeSitterQueryCountersV1, ProviderTreeSitterQueryIdentityV1,
    ProviderTreeSitterQueryReadStateV1, ProviderTreeSitterQueryReadV1,
    ProviderTreeSitterQueryReceiptV1,
};
use super::workspace_db_registry::ProviderSearchWorkspaceSessionV1;

pub(super) async fn read_provider_treesitter_query_in_session_v1(
    session: &ProviderSearchWorkspaceSessionV1,
    query: &ProviderTreeSitterQueryIdentityV1,
    incremental_budget: u32,
    continuation: Option<&ProviderTreeSitterContinuationV1>,
) -> Result<ProviderTreeSitterQueryReadV1, String> {
    validate_query(query)?;
    read_provider_treesitter_query_on_connection(
        session.read_connection(),
        query,
        incremental_budget,
        continuation,
    )
    .await
}

async fn read_provider_treesitter_query_on_connection(
    connection: &turso::Connection,
    query: &ProviderTreeSitterQueryIdentityV1,
    incremental_budget: u32,
    continuation: Option<&ProviderTreeSitterContinuationV1>,
) -> Result<ProviderTreeSitterQueryReadV1, String> {
    let Some(inventory) = read_inventory(connection, query).await? else {
        if continuation.is_some() {
            return Err("Tree-sitter continuation cannot resume without provider inventory".into());
        }
        return Ok(ProviderTreeSitterQueryReadV1::missing());
    };
    validate_continuation(query, &inventory, continuation)?;
    let cached_by_owner = read_current_cached_results(connection, query, &inventory).await?;
    let (mut cached_results, mut work) = (Vec::new(), Vec::new());
    for entry in &inventory.entries {
        let is_current_cache = entry.state == ProviderOwnerInventoryEntryStateV1::Indexed
            && entry.owner_content_digest.is_some()
            && cached_by_owner.contains_key(entry.owner_path.as_str());
        if is_current_cache {
            cached_results.push(
                cached_by_owner
                    .get(entry.owner_path.as_str())
                    .expect("checked cache membership")
                    .clone(),
            );
        } else {
            work.push(entry.clone());
        }
    }
    let start = continuation
        .map(|token| {
            work.iter()
                .position(|entry| entry.owner_path == token.next_owner_cursor)
                .ok_or_else(|| {
                    "Tree-sitter continuation cursor is not a current pending owner".to_owned()
                })
        })
        .transpose()?
        .unwrap_or(0);
    let budget = usize::try_from(incremental_budget).unwrap_or(usize::MAX);
    let end = start.saturating_add(budget).min(work.len());
    let scheduled_entries = work[start..end].to_vec();
    let remaining_owner_count = work.len().saturating_sub(end);
    let remaining_count_kind = match inventory.state {
        ProviderOwnerInventoryStateV1::Exact => ProviderRemainingOwnerCountKindV1::Exact,
        ProviderOwnerInventoryStateV1::Known => ProviderRemainingOwnerCountKindV1::KnownLowerBound,
    };
    let next = work.get(end).map(|entry| ProviderTreeSitterContinuationV1 {
        provider_workspace_identity_digest: query.scope.provider_workspace_identity_digest.clone(),
        query_digest: query.query_digest.clone(),
        inventory_digest: inventory.inventory_digest.clone(),
        inventory_generation: inventory.inventory_generation.clone(),
        inventory_state: inventory.state,
        remaining_count_kind,
        next_owner_cursor: entry.owner_path.clone(),
    });
    let complete = inventory.state == ProviderOwnerInventoryStateV1::Exact
        && work.is_empty()
        && continuation.is_none();
    let cached_projection_count = cached_results
        .iter()
        .map(|result| result.projections.len())
        .sum::<usize>()
        .try_into()
        .unwrap_or(u32::MAX);
    let receipt = ProviderTreeSitterQueryReceiptV1 {
        query: query.clone(),
        inventory_state: inventory.state,
        inventory_digest: inventory.inventory_digest.clone(),
        inventory_generation: inventory.inventory_generation.clone(),
        cached_owner_count: cached_results.len().try_into().unwrap_or(u32::MAX),
        cached_projection_count,
        scheduled_owner_count: scheduled_entries.len().try_into().unwrap_or(u32::MAX),
        remaining_owner_count: remaining_owner_count.try_into().unwrap_or(u32::MAX),
        remaining_count_kind,
        incremental_budget,
        continuation: next,
        counters: ProviderTreeSitterQueryCountersV1 {
            query_cache_reads: 1,
            ..ProviderTreeSitterQueryCountersV1::default()
        },
    };
    Ok(ProviderTreeSitterQueryReadV1 {
        state: if complete {
            ProviderTreeSitterQueryReadStateV1::Complete
        } else {
            ProviderTreeSitterQueryReadStateV1::Partial
        },
        absence_authoritative: complete,
        inventory: Some(inventory),
        cached_results,
        scheduled_entries,
        receipt: Some(receipt),
    })
}

fn validate_query(query: &ProviderTreeSitterQueryIdentityV1) -> Result<(), String> {
    if query.query_digest.trim().is_empty() {
        return Err("Tree-sitter query digest must be non-empty".into());
    }
    if query
        .scope
        .provider_workspace_identity_digest
        .trim()
        .is_empty()
        || query.scope.provider_id.trim().is_empty()
    {
        return Err("Tree-sitter provider scope must be complete".into());
    }
    Ok(())
}

fn validate_continuation(
    query: &ProviderTreeSitterQueryIdentityV1,
    inventory: &ProviderOwnerInventoryV1,
    continuation: Option<&ProviderTreeSitterContinuationV1>,
) -> Result<(), String> {
    let Some(token) = continuation else {
        return Ok(());
    };
    let matches = token.provider_workspace_identity_digest
        == query.scope.provider_workspace_identity_digest
        && token.query_digest == query.query_digest
        && token.inventory_digest == inventory.inventory_digest
        && token.inventory_generation == inventory.inventory_generation
        && token.inventory_state == inventory.state
        && token.remaining_count_kind
            == match inventory.state {
                ProviderOwnerInventoryStateV1::Exact => ProviderRemainingOwnerCountKindV1::Exact,
                ProviderOwnerInventoryStateV1::Known => {
                    ProviderRemainingOwnerCountKindV1::KnownLowerBound
                }
            };
    if matches {
        Ok(())
    } else {
        Err("Tree-sitter continuation identity does not match current query inventory".into())
    }
}

async fn read_inventory(
    connection: &turso::Connection,
    query: &ProviderTreeSitterQueryIdentityV1,
) -> Result<Option<ProviderOwnerInventoryV1>, String> {
    let scope = &query.scope;
    let mut rows = match connection
        .query(
            "SELECT inventory_state, inventory_digest, inventory_generation
             FROM provider_owner_inventory_v1
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
    {
        Ok(rows) => rows,
        Err(error) if is_missing_schema(&error.to_string()) => return Ok(None),
        Err(error) => return Err(format!("failed to read provider owner inventory: {error}")),
    };
    let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read provider inventory row: {error}"))?
    else {
        return Ok(None);
    };
    let state = decode_inventory_state(row_text(&row, 0, "inventoryState")?)?;
    let inventory_digest = row_text(&row, 1, "inventoryDigest")?;
    let inventory_generation = row_text(&row, 2, "inventoryGeneration")?;
    let mut entry_rows = connection
        .query(
            "SELECT owner_path, owner_content_digest, owner_state
             FROM provider_owner_inventory_entry_v1
             WHERE project_root = ?1
               AND workspace_identity = ?2
               AND provider_workspace_identity_digest = ?3
               AND provider_id = ?4
               AND inventory_generation = ?5
             ORDER BY owner_path",
            (
                scope.project_root.as_str(),
                scope.workspace_identity.as_str(),
                scope.provider_workspace_identity_digest.as_str(),
                scope.provider_id.as_str(),
                inventory_generation.as_str(),
            ),
        )
        .await
        .map_err(|error| format!("failed to read provider inventory entries: {error}"))?;
    let mut entries = Vec::new();
    while let Some(entry) = entry_rows
        .next()
        .await
        .map_err(|error| format!("failed to read provider inventory entry: {error}"))?
    {
        entries.push(ProviderOwnerInventoryEntryV1 {
            owner_path: row_text(&entry, 0, "ownerPath")?,
            owner_content_digest: entry
                .get::<Option<String>>(1)
                .map_err(|error| format!("failed to decode ownerContentDigest: {error}"))?,
            state: decode_entry_state(row_text(&entry, 2, "ownerState")?)?,
        });
    }
    Ok(Some(ProviderOwnerInventoryV1 {
        scope: scope.clone(),
        state,
        inventory_digest,
        inventory_generation,
        entries,
    }))
}

async fn read_current_cached_results(
    connection: &turso::Connection,
    query: &ProviderTreeSitterQueryIdentityV1,
    inventory: &ProviderOwnerInventoryV1,
) -> Result<BTreeMap<String, ProviderTreeSitterOwnerResultV1>, String> {
    let scope = &query.scope;
    let mut rows = connection
        .query(
            "SELECT owner.owner_path, owner.owner_content_digest,
                    owner.inventory_generation, owner.complete_owner_refresh_count,
                    capture.structural_selector, capture.signature,
                    capture.item_kind, capture.item_name, capture.capture_name,
                    capture.item_source_byte_start, capture.item_source_byte_end,
                    capture.source_byte_start, capture.source_byte_end
             FROM provider_treesitter_query_owner_v1 AS owner
             JOIN provider_owner_inventory_entry_v1 AS inventory
               ON inventory.project_root = owner.project_root
              AND inventory.workspace_identity = owner.workspace_identity
              AND inventory.provider_workspace_identity_digest =
                  owner.provider_workspace_identity_digest
              AND inventory.provider_id = owner.provider_id
              AND inventory.owner_path = owner.owner_path
              AND inventory.owner_content_digest = owner.owner_content_digest
              AND inventory.inventory_generation = ?6
              AND inventory.owner_state = 'indexed'
             LEFT JOIN provider_treesitter_capture_projection_v1 AS capture
               ON capture.project_root = owner.project_root
              AND capture.workspace_identity = owner.workspace_identity
              AND capture.provider_workspace_identity_digest =
                  owner.provider_workspace_identity_digest
              AND capture.provider_id = owner.provider_id
              AND capture.query_digest = owner.query_digest
              AND capture.owner_path = owner.owner_path
              AND capture.owner_content_digest = owner.owner_content_digest
             WHERE owner.project_root = ?1
               AND owner.workspace_identity = ?2
               AND owner.provider_workspace_identity_digest = ?3
               AND owner.provider_id = ?4
               AND owner.query_digest = ?5
             ORDER BY owner.owner_path, capture.source_byte_start,
                      capture.source_byte_end, capture.capture_name",
            (
                scope.project_root.as_str(),
                scope.workspace_identity.as_str(),
                scope.provider_workspace_identity_digest.as_str(),
                scope.provider_id.as_str(),
                query.query_digest.as_str(),
                inventory.inventory_generation.as_str(),
            ),
        )
        .await
        .map_err(|error| format!("failed to read Tree-sitter query cache: {error}"))?;
    let mut results = BTreeMap::<String, ProviderTreeSitterOwnerResultV1>::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Tree-sitter query cache row: {error}"))?
    {
        let owner_path = row_text(&row, 0, "ownerPath")?;
        let result =
            results
                .entry(owner_path.clone())
                .or_insert_with(|| ProviderTreeSitterOwnerResultV1 {
                    owner_path,
                    owner_content_digest: row.get::<String>(1).unwrap_or_default(),
                    query_digest: query.query_digest.clone(),
                    inventory_generation: row.get::<String>(2).unwrap_or_default(),
                    state: ProviderTreeSitterOwnerResultStateV1::Cached,
                    complete_owner_refresh_count: row.get::<i64>(3).unwrap_or_default() as u32,
                    projections: Vec::new(),
                });
        if let Some(structural_selector) = row
            .get::<Option<String>>(4)
            .map_err(|error| format!("failed to decode structuralSelector: {error}"))?
        {
            result
                .projections
                .push(ProviderTreeSitterCaptureProjectionV1 {
                    structural_selector,
                    signature: row_text(&row, 5, "signature")?,
                    item_kind: row_text(&row, 6, "itemKind")?,
                    item_name: row_text(&row, 7, "itemName")?,
                    capture_name: row_text(&row, 8, "captureName")?,
                    item_source_byte_start: row_u64(&row, 9, "itemSourceByteStart")?,
                    item_source_byte_end: row_u64(&row, 10, "itemSourceByteEnd")?,
                    source_byte_start: row_u64(&row, 11, "sourceByteStart")?,
                    source_byte_end: row_u64(&row, 12, "sourceByteEnd")?,
                });
        }
    }
    Ok(results)
}

fn decode_inventory_state(value: String) -> Result<ProviderOwnerInventoryStateV1, String> {
    match value.as_str() {
        "exact" => Ok(ProviderOwnerInventoryStateV1::Exact),
        "known" => Ok(ProviderOwnerInventoryStateV1::Known),
        _ => Err(format!("unknown provider inventory state `{value}`")),
    }
}

fn decode_entry_state(value: String) -> Result<ProviderOwnerInventoryEntryStateV1, String> {
    match value.as_str() {
        "indexed" => Ok(ProviderOwnerInventoryEntryStateV1::Indexed),
        "dirty" => Ok(ProviderOwnerInventoryEntryStateV1::Dirty),
        "unindexed" => Ok(ProviderOwnerInventoryEntryStateV1::Unindexed),
        _ => Err(format!("unknown provider inventory entry state `{value}`")),
    }
}

fn row_text(row: &turso::Row, index: usize, field: &str) -> Result<String, String> {
    row.get::<String>(index)
        .map_err(|error| format!("failed to decode {field}: {error}"))
}

fn row_u64(row: &turso::Row, index: usize, field: &str) -> Result<u64, String> {
    let value = row
        .get::<i64>(index)
        .map_err(|error| format!("failed to decode {field}: {error}"))?;
    value
        .try_into()
        .map_err(|_| format!("Tree-sitter cache field `{field}` must be non-negative"))
}

fn is_missing_schema(error: &str) -> bool {
    error.contains("no such table") || error.contains("does not exist")
}
