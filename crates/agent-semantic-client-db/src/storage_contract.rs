// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Backend-neutral storage contracts owned by `agent-semantic-client-db`.
//!
//! Concrete Turso profiles and the in-memory reference backend implement the
//! same atomic batch and keyset pagination semantics from this module.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;

/// Schema identifier for atomic session event batches.
pub const SESSION_EVENT_BATCH_SCHEMA_ID: &str = "asp.storage-session-event-batch.v1";
/// Schema identifier for terminal batch write receipts.
pub const SESSION_EVENT_BATCH_RECEIPT_SCHEMA_ID: &str =
    "asp.storage-session-event-batch-write-receipt.v1";
/// Maximum rows accepted in one atomic event batch.
pub const MAX_SESSION_EVENT_BATCH_ROWS: usize = 1_024;
/// Maximum rows returned by one keyset page.
pub const MAX_KEYSET_PAGE_LIMIT: usize = 1_000;

/// Sendable backend-neutral storage operation future.
pub type StorageFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, StorageError>> + Send + 'a>>;

type SessionEventPartitions = BTreeMap<String, BTreeMap<(i64, String), SessionEvent>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// Declared concurrency and checkpoint profile for one storage operation.
pub enum StorageOptimizationProfile {
    CompatibilityImmediate,
    MvccConcurrent,
    MvccConcurrentPassiveCheckpoint,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// Transaction isolation mechanism required by a storage profile.
pub enum StorageTransactionMode {
    Immediate,
    Concurrent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// Authority that durably owns one storage result.
pub enum StorageAuthorityKind {
    Local,
    RemoteSync,
    InMemory,
}

macro_rules! storage_value_type {
    ($name:ident) => {
        #[doc = concat!("Typed storage contract value `", stringify!($name), "`.")]
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }

            #[must_use]
            pub fn into_string(self) -> String {
                self.0
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_string())
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        impl std::ops::Deref for $name {
            type Target = str;

            fn deref(&self) -> &Self::Target {
                self.as_str()
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(self.as_str())
            }
        }
    };
}

storage_value_type!(StorageRepoId);
storage_value_type!(StorageWorkspaceId);
storage_value_type!(StorageScopeId);
storage_value_type!(StorageSessionId);
storage_value_type!(StorageAgentId);
storage_value_type!(StorageSessionEventId);
storage_value_type!(StorageTurnId);
storage_value_type!(StorageSessionEventKind);
storage_value_type!(StorageReceiptSchemaId);
storage_value_type!(StorageSessionEventBatchId);
storage_value_type!(StorageBackendId);
storage_value_type!(StorageSloMatrixReceiptSchemaId);

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Exact repository/workspace/session/agent partition identity.
pub struct StoragePartitionKey {
    pub repo_id: StorageRepoId,
    pub workspace_id: StorageWorkspaceId,
    pub scope_id: StorageScopeId,
    pub session_id: StorageSessionId,
    pub agent_id: StorageAgentId,
}

impl StoragePartitionKey {
    /// Encode the partition as an unambiguous length-delimited key.
    pub fn canonical_key(&self) -> String {
        [
            ("repo", self.repo_id.as_str()),
            ("workspace", self.workspace_id.as_str()),
            ("scope", self.scope_id.as_str()),
            ("session", self.session_id.as_str()),
            ("agent", self.agent_id.as_str()),
        ]
        .into_iter()
        .map(|(name, value)| format!("{name}:{}:{value}", value.len()))
        .collect::<Vec<_>>()
        .join("|")
    }

    fn validate(&self) -> Result<(), StorageError> {
        for (name, value) in [
            ("repoId", self.repo_id.as_str()),
            ("workspaceId", self.workspace_id.as_str()),
            ("scopeId", self.scope_id.as_str()),
            ("sessionId", self.session_id.as_str()),
            ("agentId", self.agent_id.as_str()),
        ] {
            if value.is_empty() {
                return Err(StorageError::invalid_request(format!(
                    "storage partition field {name} must not be empty"
                )));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Immutable agent session event stored in one partition.
pub struct SessionEvent {
    pub event_id: StorageSessionEventId,
    pub turn_id: StorageTurnId,
    pub event_kind: StorageSessionEventKind,
    pub payload: Vec<u8>,
    pub created_at_ms: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Bounded conflict retry policy for explicit storage writes.
pub struct StorageRetryPolicy {
    pub(crate) max_attempts: u32,
    pub(crate) base_delay_ms: u64,
    pub(crate) max_delay_ms: u64,
    pub(crate) jitter_ms: u64,
}

impl Default for StorageRetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 8,
            base_delay_ms: 2,
            max_delay_ms: 100,
            jitter_ms: 3,
        }
    }
}

impl StorageRetryPolicy {
    /// Validate retry bounds before any operation starts.
    pub fn validate(&self) -> Result<(), StorageError> {
        if self.max_attempts == 0 {
            return Err(StorageError::invalid_request(
                "storage retry maxAttempts must be greater than zero",
            ));
        }
        if self.base_delay_ms > self.max_delay_ms {
            return Err(StorageError::invalid_request(
                "storage retry baseDelayMs must not exceed maxDelayMs",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Atomic, typed session event batch and its storage profile.
pub struct SessionEventBatch {
    pub schema_id: String,
    pub batch_id: String,
    pub partition: StoragePartitionKey,
    pub optimization_profile: StorageOptimizationProfile,
    pub transaction_mode: StorageTransactionMode,
    pub retry_policy: StorageRetryPolicy,
    pub events: Vec<SessionEvent>,
}

impl SessionEventBatch {
    /// Validate schema, partition, identities, size, and profile compatibility.
    pub fn validate(&self) -> Result<(), StorageError> {
        if self.schema_id != SESSION_EVENT_BATCH_SCHEMA_ID {
            return Err(StorageError::invalid_request(format!(
                "unsupported session event batch schemaId: {}",
                self.schema_id
            )));
        }
        if self.batch_id.is_empty() {
            return Err(StorageError::invalid_request(
                "session event batchId must not be empty",
            ));
        }
        self.partition.validate()?;
        self.retry_policy.validate()?;
        if self.events.is_empty() || self.events.len() > MAX_SESSION_EVENT_BATCH_ROWS {
            return Err(StorageError::invalid_request(format!(
                "session event batch rows must be within 1..={MAX_SESSION_EVENT_BATCH_ROWS}"
            )));
        }
        let mut identities = BTreeSet::new();
        for event in &self.events {
            if event.event_id.is_empty() || event.turn_id.is_empty() || event.event_kind.is_empty()
            {
                return Err(StorageError::invalid_request(
                    "session event identity, turn, and kind must not be empty",
                ));
            }
            if !identities.insert(event.event_id.as_str()) {
                return Err(StorageError::duplicate_identity(format!(
                    "duplicate event identity in batch: {}",
                    event.event_id
                )));
            }
        }
        match (self.optimization_profile, self.transaction_mode) {
            (
                StorageOptimizationProfile::CompatibilityImmediate,
                StorageTransactionMode::Immediate,
            )
            | (
                StorageOptimizationProfile::MvccConcurrent
                | StorageOptimizationProfile::MvccConcurrentPassiveCheckpoint,
                StorageTransactionMode::Concurrent,
            ) => Ok(()),
            _ => Err(StorageError::unsupported_profile(
                "storage optimization profile and transaction mode are incompatible",
            )),
        }
    }

    /// Compute the deterministic digest of the validated execution request.
    pub fn execution_digest(&self) -> Result<String, StorageError> {
        let bytes = serde_json::to_vec(self).map_err(|error| {
            StorageError::backend(format!("serialize storage batch digest input: {error}"))
        })?;
        let digest = Sha256::digest(bytes);
        let mut encoded = String::with_capacity(digest.len() * 2);
        for byte in digest {
            let _ = write!(encoded, "{byte:02x}");
        }
        Ok(format!("sha256:{encoded}"))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// Terminal state of one requested storage transaction.
pub enum StorageTransactionState {
    Committed,
    Aborted,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Terminal evidence for one atomic session event batch write.
pub struct SessionEventBatchWriteReceipt {
    pub schema_id: StorageReceiptSchemaId,
    pub batch_id: StorageSessionEventBatchId,
    pub partition: StoragePartitionKey,
    pub authority: StorageAuthorityKind,
    pub backend: StorageBackendId,
    pub backend_version: String,
    pub optimization_profile: StorageOptimizationProfile,
    pub transaction_mode: StorageTransactionMode,
    pub transaction_state: StorageTransactionState,
    pub attempted_rows: usize,
    pub committed_rows: usize,
    pub retry_count: u32,
    pub busy_count: u32,
    pub snapshot_conflict_count: u32,
    pub retry_delay_ms: u64,
    pub conflict_count: u32,
    pub execution_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Stable composite continuation cursor for session event pagination.
pub struct SessionEventCursor {
    pub created_at_ms: i64,
    pub event_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Bounded keyset page request for one exact storage partition.
pub struct SessionEventPageRequest {
    pub partition: StoragePartitionKey,
    pub after: Option<SessionEventCursor>,
    pub limit: usize,
}

impl SessionEventPageRequest {
    /// Validate partition identity and page bounds.
    pub fn validate(&self) -> Result<(), StorageError> {
        self.partition.validate()?;
        if self.limit == 0 || self.limit > MAX_KEYSET_PAGE_LIMIT {
            return Err(StorageError::invalid_request(format!(
                "session event page limit must be within 1..={MAX_KEYSET_PAGE_LIMIT}"
            )));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Ordered session events and an optional continuation cursor.
pub struct SessionEventPage {
    pub items: Vec<SessionEvent>,
    pub next: Option<SessionEventCursor>,
}

/// Stable failure category shared by every agent storage backend.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StorageErrorCode {
    InvalidRequest,
    DuplicateIdentity,
    Busy,
    Locked,
    SnapshotConflict,
    SchemaConflict,
    UnsupportedProfile,
    Io,
    Backend,
}

/// Typed backend-neutral storage failure with explicit retry eligibility.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageError {
    pub code: StorageErrorCode,
    pub retryable: bool,
    pub message: String,
}

impl StorageError {
    /// Construct a non-retryable request validation failure.
    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::new(StorageErrorCode::InvalidRequest, false, message)
    }

    /// Construct a non-retryable duplicate identity failure.
    pub fn duplicate_identity(message: impl Into<String>) -> Self {
        Self::new(StorageErrorCode::DuplicateIdentity, false, message)
    }

    /// Construct a non-retryable unsupported profile failure.
    pub fn unsupported_profile(message: impl Into<String>) -> Self {
        Self::new(StorageErrorCode::UnsupportedProfile, false, message)
    }

    /// Construct a non-retryable backend failure.
    pub fn backend(message: impl Into<String>) -> Self {
        Self::new(StorageErrorCode::Backend, false, message)
    }

    /// Construct an explicitly classified storage failure.
    pub fn new(code: StorageErrorCode, retryable: bool, message: impl Into<String>) -> Self {
        Self {
            code,
            retryable,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{:?}: {}", self.code, self.message)
    }
}

impl std::error::Error for StorageError {}

/// Backend-neutral atomic event storage contract for agent sessions.
pub trait AgentStorage: Send + Sync {
    /// Atomically append a validated session event batch.
    fn append_session_events_atomically<'a>(
        &'a self,
        batch: &'a SessionEventBatch,
    ) -> StorageFuture<'a, SessionEventBatchWriteReceipt>;

    /// List one bounded, deterministic page of session events.
    fn list_session_events<'a>(
        &'a self,
        request: &'a SessionEventPageRequest,
    ) -> StorageFuture<'a, SessionEventPage>;
}

#[derive(Default)]
/// Deterministic in-memory implementation used by contract tests and ephemeral owners.
pub struct InMemoryAgentStorage {
    partitions: Mutex<SessionEventPartitions>,
}

impl AgentStorage for InMemoryAgentStorage {
    fn append_session_events_atomically<'a>(
        &'a self,
        batch: &'a SessionEventBatch,
    ) -> StorageFuture<'a, SessionEventBatchWriteReceipt> {
        Box::pin(async move {
            batch.validate()?;
            let partition_key = batch.partition.canonical_key();
            let execution_digest = batch.execution_digest()?;
            let mut partitions = self
                .partitions
                .lock()
                .map_err(|_| StorageError::backend("in-memory storage authority lock poisoned"))?;
            let existing = partitions.entry(partition_key).or_default();
            let existing_ids = existing
                .values()
                .map(|event| event.event_id.as_str())
                .collect::<BTreeSet<_>>();
            if let Some(duplicate) = batch
                .events
                .iter()
                .find(|event| existing_ids.contains(event.event_id.as_str()))
            {
                return Err(StorageError::duplicate_identity(format!(
                    "session event identity already exists: {}",
                    duplicate.event_id
                )));
            }
            for event in &batch.events {
                existing.insert(
                    (event.created_at_ms, event.event_id.as_str().to_owned()),
                    event.clone(),
                );
            }
            Ok(SessionEventBatchWriteReceipt {
                schema_id: SESSION_EVENT_BATCH_RECEIPT_SCHEMA_ID.into(),
                batch_id: batch.batch_id.clone().into(),
                partition: batch.partition.clone(),
                authority: StorageAuthorityKind::InMemory,
                backend: "in-memory".into(),
                backend_version: env!("CARGO_PKG_VERSION").to_string(),
                optimization_profile: batch.optimization_profile,
                transaction_mode: batch.transaction_mode,
                transaction_state: StorageTransactionState::Committed,
                attempted_rows: batch.events.len(),
                committed_rows: batch.events.len(),
                retry_count: 0,
                busy_count: 0,
                snapshot_conflict_count: 0,
                retry_delay_ms: 0,
                conflict_count: 0,
                execution_digest,
            })
        })
    }

    fn list_session_events<'a>(
        &'a self,
        request: &'a SessionEventPageRequest,
    ) -> StorageFuture<'a, SessionEventPage> {
        Box::pin(async move {
            request.validate()?;
            let partitions = self
                .partitions
                .lock()
                .map_err(|_| StorageError::backend("in-memory storage authority lock poisoned"))?;
            let Some(events) = partitions.get(&request.partition.canonical_key()) else {
                return Ok(SessionEventPage {
                    items: Vec::new(),
                    next: None,
                });
            };
            let after_key = request
                .after
                .as_ref()
                .map(|cursor| (cursor.created_at_ms, cursor.event_id.as_str()));
            let mut selected = events
                .iter()
                .filter(|((created_at_ms, event_id), _)| {
                    after_key
                        .as_ref()
                        .is_none_or(|after| (*created_at_ms, event_id.as_str()) > *after)
                })
                .take(request.limit + 1)
                .map(|(_, event)| event.clone())
                .collect::<Vec<_>>();
            let has_more = selected.len() > request.limit;
            if has_more {
                selected.truncate(request.limit);
            }
            let next = has_more.then(|| {
                let last = selected.last().expect("non-empty limited keyset page");
                SessionEventCursor {
                    created_at_ms: last.created_at_ms,
                    event_id: last.event_id.as_str().to_owned(),
                }
            });
            Ok(SessionEventPage {
                items: selected,
                next,
            })
        })
    }
}
