// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Typed contracts for provider-scoped incremental Tree-sitter persistence.

use super::provider_incremental::ProviderIncrementalScoped;

/// Completeness authority of the provider-scoped owner inventory.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderOwnerInventoryState {
    Exact,
    Known,
}

/// Persisted indexing state of one known provider owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderOwnerInventoryEntryState {
    Indexed,
    Dirty,
    Unindexed,
}

/// Meaning of a reported remaining-owner count.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderRemainingOwnerCountKind {
    Exact,
    KnownLowerBound,
}

/// One owner in a provider-scoped inventory generation.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderOwnerInventoryEntry {
    pub owner_path: String,
    pub owner_content_digest: Option<String>,
    pub state: ProviderOwnerInventoryEntryState,
}

/// Provider-scoped owner inventory published without source blobs.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderOwnerInventory {
    pub scope: ProviderIncrementalScoped,
    pub state: ProviderOwnerInventoryState,
    pub inventory_digest: String,
    pub inventory_generation: String,
    pub entries: Vec<ProviderOwnerInventoryEntry>,
}

/// Request to publish a deterministic provider owner inventory generation.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderOwnerInventoryWrite {
    pub scope: ProviderIncrementalScoped,
    pub state: ProviderOwnerInventoryState,
    pub entries: Vec<ProviderOwnerInventoryEntry>,
}

/// Committed inventory identity and replacement counts.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderOwnerInventoryWriteReceipt {
    pub inventory_digest: String,
    pub inventory_generation: String,
    pub upserted_entry_count: u32,
    pub deleted_entry_count: u32,
}

/// Stable identity of one provider Tree-sitter query.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderTreeSitterQueryIdentity {
    pub scope: ProviderIncrementalScoped,
    pub query_digest: String,
    pub capture_names: Vec<String>,
}

/// Query-owner cache state surfaced by an incremental read.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderTreeSitterOwnerResultState {
    Cached,
    Processed,
}

/// One Tree-sitter capture joined to its smallest containing owner item.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderTreeSitterCaptureProjection {
    pub structural_selector: String,
    pub signature: String,
    pub item_kind: String,
    pub item_name: String,
    pub capture_name: String,
    pub item_source_byte_start: u64,
    pub item_source_byte_end: u64,
    pub source_byte_start: u64,
    pub source_byte_end: u64,
}

/// Complete cached or newly processed result for one query-owner identity.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderTreeSitterOwnerResult {
    pub owner_path: String,
    pub owner_content_digest: String,
    pub query_digest: String,
    pub inventory_generation: String,
    pub state: ProviderTreeSitterOwnerResultState,
    pub complete_owner_refresh_count: u32,
    pub projections: Vec<ProviderTreeSitterCaptureProjection>,
}

/// Visibility receipt for one atomic query-owner result replacement.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderTreeSitterOwnerWriteReceipt {
    pub query_digest: String,
    pub owner_path: String,
    pub owner_content_digest: String,
    pub query_metadata_writes: u32,
    pub owner_result_writes: u32,
    pub capture_projection_writes: u32,
}

/// Continuation identity for deterministic provider owner work.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderTreeSitterContinuation {
    pub provider_workspace_identity_digest: String,
    pub query_digest: String,
    pub inventory_digest: String,
    pub inventory_generation: String,
    pub inventory_state: ProviderOwnerInventoryState,
    pub remaining_count_kind: ProviderRemainingOwnerCountKind,
    pub next_owner_cursor: String,
}

/// Observable side effects of one incremental Tree-sitter acquisition.
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderTreeSitterQueryCounters {
    pub metadata_reads: u32,
    pub source_byte_reads: u32,
    pub source_bytes_read: u64,
    pub provider_parses: u32,
    pub query_cache_reads: u32,
    pub query_cache_writes: u32,
    pub complete_owner_refreshes: u32,
    pub owner_index_writes: u32,
    pub cas_writes: u32,
    pub merkle_leaf_writes: u32,
    pub merkle_path_node_writes: u32,
    pub full_source_walks: u32,
    pub full_cas_materializations: u32,
    pub full_merkle_rebuilds: u32,
    pub unrelated_provider_count: u32,
}

/// Provider-scoped cache and work-queue receipt for one query invocation.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderTreeSitterQueryReceipt {
    pub query: ProviderTreeSitterQueryIdentity,
    pub inventory_state: ProviderOwnerInventoryState,
    pub inventory_digest: String,
    pub inventory_generation: String,
    pub cached_owner_count: u32,
    pub cached_projection_count: u32,
    pub scheduled_owner_count: u32,
    pub remaining_owner_count: u32,
    pub remaining_count_kind: ProviderRemainingOwnerCountKind,
    pub incremental_budget: u32,
    pub continuation: Option<ProviderTreeSitterContinuation>,
    pub counters: ProviderTreeSitterQueryCounters,
}

/// Typed state of one provider-scoped incremental query-cache read.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderTreeSitterQueryReadState {
    MissingInventory,
    Partial,
    Complete,
}

/// Cached projections and deterministic owner work selected without source IO.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderTreeSitterQueryRead {
    pub state: ProviderTreeSitterQueryReadState,
    pub absence_authoritative: bool,
    pub inventory: Option<ProviderOwnerInventory>,
    pub cached_results: Vec<ProviderTreeSitterOwnerResult>,
    pub scheduled_entries: Vec<ProviderOwnerInventoryEntry>,
    pub receipt: Option<ProviderTreeSitterQueryReceipt>,
}

impl ProviderTreeSitterQueryRead {
    pub(super) fn missing() -> Self {
        Self {
            state: ProviderTreeSitterQueryReadState::MissingInventory,
            absence_authoritative: false,
            inventory: None,
            cached_results: Vec::new(),
            scheduled_entries: Vec::new(),
            receipt: None,
        }
    }
}
