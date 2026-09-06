// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Typed MVCC partition heads, records, aliases, and compare-and-append operations.

use std::{collections::BTreeSet, sync::Arc, time::Duration};

use crate::{
    turso_mvcc_partition_sql::{
        INSERT_HEAD_SQL, INSERT_RECORD_SQL, SELECT_HEAD_SQL, SELECT_RECORD_ID_SQL,
        SELECT_RECORDS_SQL, UPDATE_HEAD_SQL,
    },
    turso_mvcc_store::TursoMvccStore,
};
use serde::{Deserialize, Serialize};

/// Committed head metadata for one exact MVCC partition.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TursoMvccPartitionHead {
    pub partition_key: String,
    pub revision: u64,
    pub head_digest: String,
    pub last_sequence: u64,
    pub projection: Vec<u8>,
    pub committed_at_ms: i64,
}

/// Optional compare-and-append precondition for a partition head.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TursoMvccExpectedHead {
    pub revision: u64,
    pub head_digest: String,
    pub last_sequence: u64,
}

impl From<&TursoMvccPartitionHead> for TursoMvccExpectedHead {
    fn from(head: &TursoMvccPartitionHead) -> Self {
        Self {
            revision: head.revision,
            head_digest: head.head_digest.clone(),
            last_sequence: head.last_sequence,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
struct TursoMvccRecordId(String);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
struct TursoMvccRecordKind(String);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
struct TursoMvccRecordPayload(Vec<u8>);

/// Immutable record appended to one MVCC partition revision.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TursoMvccPartitionRecord {
    record_id: TursoMvccRecordId,
    record_kind: TursoMvccRecordKind,
    payload: TursoMvccRecordPayload,
}

impl TursoMvccPartitionRecord {
    /// Construct a validated non-empty partition record.
    pub fn new(
        record_id: impl Into<String>,
        record_kind: impl Into<String>,
        payload: Vec<u8>,
    ) -> Result<Self, String> {
        let record_id = record_id.into();
        let record_kind = record_kind.into();
        if record_id.is_empty() || record_kind.is_empty() || payload.is_empty() {
            return Err(
                "MVCC partition records require non-empty identity, kind, and payload".to_string(),
            );
        }
        Ok(Self {
            record_id: TursoMvccRecordId(record_id),
            record_kind: TursoMvccRecordKind(record_kind),
            payload: TursoMvccRecordPayload(payload),
        })
    }

    /// Return the stable record identity.
    #[must_use]
    pub fn record_id(&self) -> &str {
        &self.record_id.0
    }

    /// Return the semantic record kind.
    #[must_use]
    pub fn record_kind(&self) -> &str {
        &self.record_kind.0
    }

    /// Borrow the encoded record payload.
    #[must_use]
    pub fn payload(&self) -> &[u8] {
        &self.payload.0
    }
}

/// Atomic compare-and-append request for one partition revision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TursoMvccPartitionCommit {
    pub partition_key: String,
    pub expected: Option<TursoMvccExpectedHead>,
    pub next_revision: u64,
    pub next_head_digest: String,
    pub next_projection: Vec<u8>,
    pub records: Vec<TursoMvccPartitionRecord>,
    pub committed_at_ms: i64,
}

/// Validated secondary lookup key for one partition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TursoMvccPartitionAlias {
    alias_namespace: String,
    alias_key: String,
}

impl TursoMvccPartitionAlias {
    /// Parse and validate an alias namespace and key.
    pub fn parse(
        alias_namespace: impl Into<String>,
        alias_key: impl Into<String>,
    ) -> Result<Self, String> {
        let alias_namespace = alias_namespace.into();
        let alias_key = alias_key.into();
        validate_alias_component(&alias_namespace, "namespace")?;
        validate_alias_component(&alias_key, "key")?;
        Ok(Self {
            alias_namespace,
            alias_key,
        })
    }

    /// Return the validated alias namespace.
    pub fn alias_namespace(&self) -> &str {
        &self.alias_namespace
    }

    /// Return the validated alias key.
    pub fn alias_key(&self) -> &str {
        &self.alias_key
    }
}

/// Terminal receipt for one successful partition commit.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TursoMvccPartitionCommitReceipt {
    pub head: TursoMvccPartitionHead,
    pub committed_records: usize,
    pub retry_count: usize,
    pub busy_count: usize,
    pub snapshot_conflict_count: usize,
}

/// Result of an atomic partition compare-and-append attempt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TursoMvccPartitionCommitOutcome {
    Committed(TursoMvccPartitionCommitReceipt),
    Conflict(Option<TursoMvccPartitionHead>),
}

/// One partition record with its committed sequence and timestamp.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TursoMvccStoredPartitionRecord {
    partition_key: String,
    sequence: u64,
    record_id: String,
    record_kind: String,
    payload: Vec<u8>,
    committed_at_ms: i64,
}

impl TursoMvccStoredPartitionRecord {
    /// Return the owning partition key.
    pub fn partition_key(&self) -> &str {
        self.partition_key.as_str()
    }

    /// Return the commit sequence within the partition.
    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Return the stable record identity.
    pub fn record_id(&self) -> &str {
        self.record_id.as_str()
    }

    /// Return the semantic record kind.
    pub fn record_kind(&self) -> &str {
        self.record_kind.as_str()
    }

    /// Borrow the encoded record payload.
    pub fn payload(&self) -> &[u8] {
        self.payload.as_slice()
    }

    /// Return the commit timestamp in Unix milliseconds.
    pub fn committed_at_ms(&self) -> i64 {
        self.committed_at_ms
    }
}

impl TursoMvccStore {
    /// Load the current committed head for one exact partition.
    pub async fn load_partition_head(
        &self,
        partition_key: &str,
    ) -> Result<Option<TursoMvccPartitionHead>, String> {
        validate_partition_key(partition_key)?;
        let lane = partition_lane(self, partition_key);
        let connection = lane.lock_owned().await;
        select_head(&connection, partition_key).await
    }

    /// Resolve an alias and load the corresponding current partition head.
    pub async fn load_partition_head_by_alias(
        &self,
        alias_namespace: &str,
        alias_key: &str,
    ) -> Result<Option<TursoMvccPartitionHead>, String> {
        validate_alias_component(alias_namespace, "namespace")?;
        validate_alias_component(alias_key, "key")?;
        let lane = partition_lane(self, alias_key);
        let connection = lane.lock_owned().await;
        select_head_by_alias(&connection, alias_namespace, alias_key).await
    }

    /// Read committed records from an exact sequence boundary.
    pub async fn read_partition_records(
        &self,
        partition_key: &str,
    ) -> Result<Vec<TursoMvccStoredPartitionRecord>, String> {
        validate_partition_key(partition_key)?;
        let lane = partition_lane(self, partition_key);
        let connection = lane.lock_owned().await;
        read_records(&connection, partition_key).await
    }

    /// Atomically compare and append one partition revision.
    pub async fn compare_and_append_partition(
        &self,
        commit: &TursoMvccPartitionCommit,
    ) -> Result<TursoMvccPartitionCommitOutcome, String> {
        self.compare_and_append_partition_with_aliases(commit, &[])
            .await
    }

    /// Atomically compare and append while publishing validated aliases.
    pub async fn compare_and_append_partition_with_aliases(
        &self,
        commit: &TursoMvccPartitionCommit,
        aliases: &[TursoMvccPartitionAlias],
    ) -> Result<TursoMvccPartitionCommitOutcome, String> {
        validate_commit(commit)?;
        validate_aliases(aliases)?;
        if commit.expected.is_some() && !aliases.is_empty() {
            return Err(
                "MVCC partition aliases may only be created with revision-zero initialization"
                    .to_string(),
            );
        }
        let lane = partition_lane(self, &commit.partition_key);
        let connection = lane.lock_owned().await;
        let mut retry = PartitionAppendRetry::default();
        for attempt in 0..self.inner.retry_attempts {
            match compare_and_append_once(&connection, commit, aliases).await {
                Ok(AttemptOutcome::Committed(head)) => {
                    retry.terminal = Some(Ok(TursoMvccPartitionCommitOutcome::Committed(
                        TursoMvccPartitionCommitReceipt {
                            head,
                            committed_records: commit.records.len(),
                            retry_count: attempt,
                            busy_count: retry.busy_count,
                            snapshot_conflict_count: retry.snapshot_conflict_count,
                        },
                    )));
                }
                Ok(AttemptOutcome::Conflict(head)) => {
                    retry.terminal = Some(Ok(TursoMvccPartitionCommitOutcome::Conflict(head)));
                }
                Err(AttemptError::Retryable { message, snapshot }) => {
                    retry.busy_count += usize::from(!snapshot);
                    retry.snapshot_conflict_count += usize::from(snapshot);
                    retry.last_retryable_error = Some(message);
                    tokio::time::sleep(retry_delay(attempt)).await;
                }
                Err(AttemptError::Fatal(message)) => retry.terminal = Some(Err(message)),
            }
            if retry.terminal.is_some() {
                break;
            }
        }
        retry.terminal.unwrap_or_else(|| {
            Err(format!(
                "{} after {} Turso MVCC partition transaction attempts",
                retry
                    .last_retryable_error
                    .unwrap_or_else(|| "Turso MVCC partition conflict persisted".to_string()),
                self.inner.retry_attempts
            ))
        })
    }

    /// Resolve one validated alias to its canonical partition key.
    pub async fn resolve_partition_alias(
        &self,
        alias_namespace: &str,
        alias_key: &str,
    ) -> Result<Option<String>, String> {
        validate_alias_component(alias_namespace, "namespace")?;
        validate_alias_component(alias_key, "key")?;
        let lane = partition_lane(self, alias_key);
        let connection = lane.lock_owned().await;
        select_partition_alias(&connection, alias_namespace, alias_key).await
    }
}

enum AttemptOutcome {
    Committed(TursoMvccPartitionHead),
    Conflict(Option<TursoMvccPartitionHead>),
}

enum AttemptError {
    Retryable { message: String, snapshot: bool },
    Fatal(String),
}

#[derive(Default)]
struct PartitionAppendRetry {
    busy_count: usize,
    snapshot_conflict_count: usize,
    last_retryable_error: Option<String>,
    terminal: Option<Result<TursoMvccPartitionCommitOutcome, String>>,
}

async fn compare_and_append_once(
    connection: &turso::Connection,
    commit: &TursoMvccPartitionCommit,
    aliases: &[TursoMvccPartitionAlias],
) -> Result<AttemptOutcome, AttemptError> {
    connection
        .execute("BEGIN CONCURRENT", ())
        .await
        .map_err(|error| classify_error("begin MVCC partition transaction", error))?;
    let result = compare_and_append_body(connection, commit, aliases).await;
    if result.is_err() {
        let _ = connection.execute("ROLLBACK", ()).await;
    }
    result
}

async fn compare_and_append_body(
    connection: &turso::Connection,
    commit: &TursoMvccPartitionCommit,
    aliases: &[TursoMvccPartitionAlias],
) -> Result<AttemptOutcome, AttemptError> {
    let observed = select_head(connection, &commit.partition_key)
        .await
        .map_err(AttemptError::Fatal)?;
    if !matches_expected(observed.as_ref(), commit.expected.as_ref()) {
        connection
            .execute("ROLLBACK", ())
            .await
            .map_err(|error| classify_error("rollback MVCC head conflict", error))?;
        return Ok(AttemptOutcome::Conflict(observed));
    }

    for alias in aliases {
        connection
            .execute(
                crate::turso_mvcc_partition_sql::INSERT_PARTITION_ALIAS_SQL,
                (
                    alias.alias_namespace(),
                    alias.alias_key(),
                    commit.partition_key.as_str(),
                    commit.committed_at_ms,
                ),
            )
            .await
            .map_err(|error| classify_error("insert MVCC partition alias", error))?;
        let observed_partition =
            select_partition_alias(connection, alias.alias_namespace(), alias.alias_key())
                .await
                .map_err(AttemptError::Fatal)?;
        if observed_partition.as_deref() != Some(commit.partition_key.as_str()) {
            return Err(AttemptError::Fatal(format!(
                "MVCC partition alias `{}/{}` already resolves another partition",
                alias.alias_namespace(),
                alias.alias_key()
            )));
        }
    }

    let base_sequence = commit
        .expected
        .as_ref()
        .map_or(0, |expected| expected.last_sequence);
    for (offset, record) in commit.records.iter().enumerate() {
        if record_exists(connection, &commit.partition_key, record.record_id()).await? {
            return Err(AttemptError::Fatal(format!(
                "MVCC record `{}` already exists in partition `{}`",
                record.record_id(),
                commit.partition_key
            )));
        }
        connection
            .execute(
                INSERT_RECORD_SQL,
                (
                    commit.partition_key.as_str(),
                    (base_sequence + offset as u64 + 1) as i64,
                    record.record_id(),
                    record.record_kind(),
                    record.payload().to_vec(),
                    commit.committed_at_ms,
                ),
            )
            .await
            .map_err(|error| classify_error("insert MVCC partition record", error))?;
    }

    let next_head = TursoMvccPartitionHead {
        partition_key: commit.partition_key.clone(),
        revision: commit.next_revision,
        head_digest: commit.next_head_digest.clone(),
        last_sequence: base_sequence + commit.records.len() as u64,
        projection: commit.next_projection.clone(),
        committed_at_ms: commit.committed_at_ms,
    };
    write_head(connection, commit, &next_head).await?;
    connection
        .execute("COMMIT", ())
        .await
        .map_err(|error| classify_error("commit MVCC partition transaction", error))?;
    Ok(AttemptOutcome::Committed(next_head))
}

async fn write_head(
    connection: &turso::Connection,
    commit: &TursoMvccPartitionCommit,
    next_head: &TursoMvccPartitionHead,
) -> Result<(), AttemptError> {
    match &commit.expected {
        None => {
            connection
                .execute(
                    INSERT_HEAD_SQL,
                    (
                        commit.partition_key.as_str(),
                        next_head.revision as i64,
                        next_head.head_digest.as_str(),
                        next_head.last_sequence as i64,
                        next_head.projection.clone(),
                        next_head.committed_at_ms,
                    ),
                )
                .await
                .map_err(|error| classify_error("insert MVCC partition head", error))?;
        }
        Some(expected) => {
            let changed = connection
                .execute(
                    UPDATE_HEAD_SQL,
                    (
                        commit.partition_key.as_str(),
                        next_head.revision as i64,
                        next_head.head_digest.as_str(),
                        next_head.last_sequence as i64,
                        next_head.projection.clone(),
                        next_head.committed_at_ms,
                        expected.revision as i64,
                        expected.head_digest.as_str(),
                        expected.last_sequence as i64,
                    ),
                )
                .await
                .map_err(|error| classify_error("update MVCC partition head", error))?;
            if changed != 1 {
                return Err(AttemptError::Retryable {
                    message: "MVCC partition head changed before update".to_string(),
                    snapshot: true,
                });
            }
        }
    }
    Ok(())
}

async fn select_partition_alias(
    connection: &turso::Connection,
    alias_namespace: &str,
    alias_key: &str,
) -> Result<Option<String>, String> {
    let mut statement = connection
        .prepare_cached(crate::turso_mvcc_partition_sql::SELECT_PARTITION_ALIAS_SQL)
        .await
        .map_err(|error| format!("failed to prepare MVCC partition alias read: {error}"))?;
    let mut rows = statement
        .query((alias_namespace, alias_key))
        .await
        .map_err(|error| format!("failed to query MVCC partition alias: {error}"))?;
    let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to step MVCC partition alias: {error}"))?
    else {
        return Ok(None);
    };
    row.get(0)
        .map(Some)
        .map_err(|error| format!("failed to decode MVCC partition alias: {error}"))
}

async fn select_head_by_alias(
    connection: &turso::Connection,
    alias_namespace: &str,
    alias_key: &str,
) -> Result<Option<TursoMvccPartitionHead>, String> {
    let mut statement = connection
        .prepare_cached(crate::turso_mvcc_partition_sql::SELECT_PARTITION_HEAD_BY_ALIAS_SQL)
        .await
        .map_err(|error| format!("failed to prepare MVCC aliased head read: {error}"))?;
    let mut rows = statement
        .query((alias_namespace, alias_key))
        .await
        .map_err(|error| format!("failed to query MVCC aliased head: {error}"))?;
    let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to step MVCC aliased head: {error}"))?
    else {
        return Ok(None);
    };
    let partition_key: String = row
        .get(0)
        .map_err(|error| format!("failed to decode MVCC aliased partition key: {error}"))?;
    let revision: i64 = row
        .get(1)
        .map_err(|error| format!("failed to decode MVCC aliased head revision: {error}"))?;
    let last_sequence: i64 = row
        .get(3)
        .map_err(|error| format!("failed to decode MVCC aliased head sequence: {error}"))?;
    Ok(Some(TursoMvccPartitionHead {
        partition_key,
        revision: revision.max(0) as u64,
        head_digest: row
            .get(2)
            .map_err(|error| format!("failed to decode MVCC aliased head digest: {error}"))?,
        last_sequence: last_sequence.max(0) as u64,
        projection: row
            .get(4)
            .map_err(|error| format!("failed to decode MVCC aliased head projection: {error}"))?,
        committed_at_ms: row
            .get(5)
            .map_err(|error| format!("failed to decode MVCC aliased head timestamp: {error}"))?,
    }))
}

async fn select_head(
    connection: &turso::Connection,
    partition_key: &str,
) -> Result<Option<TursoMvccPartitionHead>, String> {
    let mut statement = connection
        .prepare_cached(SELECT_HEAD_SQL)
        .await
        .map_err(|error| format!("failed to prepare MVCC partition head read: {error}"))?;
    let mut rows = statement
        .query((partition_key,))
        .await
        .map_err(|error| format!("failed to query MVCC partition head: {error}"))?;
    let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to step MVCC partition head: {error}"))?
    else {
        return Ok(None);
    };
    let revision: i64 = row
        .get(0)
        .map_err(|error| format!("failed to decode MVCC head revision: {error}"))?;
    let last_sequence: i64 = row
        .get(2)
        .map_err(|error| format!("failed to decode MVCC head sequence: {error}"))?;
    Ok(Some(TursoMvccPartitionHead {
        partition_key: partition_key.to_string(),
        revision: revision.max(0) as u64,
        head_digest: row
            .get(1)
            .map_err(|error| format!("failed to decode MVCC head digest: {error}"))?,
        last_sequence: last_sequence.max(0) as u64,
        projection: row
            .get(3)
            .map_err(|error| format!("failed to decode MVCC head projection: {error}"))?,
        committed_at_ms: row
            .get(4)
            .map_err(|error| format!("failed to decode MVCC head timestamp: {error}"))?,
    }))
}

async fn read_records(
    connection: &turso::Connection,
    partition_key: &str,
) -> Result<Vec<TursoMvccStoredPartitionRecord>, String> {
    let mut statement = connection
        .prepare_cached(SELECT_RECORDS_SQL)
        .await
        .map_err(|error| format!("failed to prepare MVCC partition record read: {error}"))?;
    let mut rows = statement
        .query((partition_key,))
        .await
        .map_err(|error| format!("failed to query MVCC partition records: {error}"))?;
    let mut records = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to step MVCC partition records: {error}"))?
    {
        let sequence: i64 = row
            .get(0)
            .map_err(|error| format!("failed to decode MVCC record sequence: {error}"))?;
        records.push(TursoMvccStoredPartitionRecord {
            partition_key: partition_key.to_string(),
            sequence: sequence.max(0) as u64,
            record_id: row
                .get(1)
                .map_err(|error| format!("failed to decode MVCC record id: {error}"))?,
            record_kind: row
                .get(2)
                .map_err(|error| format!("failed to decode MVCC record kind: {error}"))?,
            payload: row
                .get(3)
                .map_err(|error| format!("failed to decode MVCC record payload: {error}"))?,
            committed_at_ms: row
                .get(4)
                .map_err(|error| format!("failed to decode MVCC record timestamp: {error}"))?,
        });
    }
    Ok(records)
}

async fn record_exists(
    connection: &turso::Connection,
    partition_key: &str,
    record_id: &str,
) -> Result<bool, AttemptError> {
    let mut statement = connection
        .prepare_cached(SELECT_RECORD_ID_SQL)
        .await
        .map_err(|error| classify_error("prepare MVCC record identity read", error))?;
    let mut rows = statement
        .query((partition_key, record_id))
        .await
        .map_err(|error| classify_error("query MVCC record identity", error))?;
    rows.next()
        .await
        .map(|row| row.is_some())
        .map_err(|error| classify_error("step MVCC record identity", error))
}

fn matches_expected(
    observed: Option<&TursoMvccPartitionHead>,
    expected: Option<&TursoMvccExpectedHead>,
) -> bool {
    match (observed, expected) {
        (None, None) => true,
        (Some(observed), Some(expected)) => {
            observed.revision == expected.revision
                && observed.head_digest == expected.head_digest
                && observed.last_sequence == expected.last_sequence
        }
        _ => false,
    }
}

fn validate_commit(commit: &TursoMvccPartitionCommit) -> Result<(), String> {
    validate_partition_key(&commit.partition_key)?;
    if commit.next_head_digest.is_empty() || commit.next_projection.is_empty() {
        return Err("MVCC partition head digest and projection must be non-empty".to_string());
    }
    if commit.committed_at_ms < 0 {
        return Err("MVCC partition timestamp must be non-negative".to_string());
    }
    match &commit.expected {
        None if commit.next_revision == 0 && commit.records.is_empty() => {}
        Some(expected)
            if expected.revision.checked_add(1) == Some(commit.next_revision)
                && !commit.records.is_empty() => {}
        None => {
            return Err(
                "MVCC partition initialization must be revision zero with no records".to_string(),
            );
        }
        Some(_) => {
            return Err(
                "MVCC partition mutation must advance one revision and append records".to_string(),
            );
        }
    }
    let mut record_ids = BTreeSet::new();
    for record in &commit.records {
        if !record_ids.insert(record.record_id()) {
            return Err("MVCC partition records require unique non-empty identities".to_string());
        }
    }
    Ok(())
}

fn validate_aliases(aliases: &[TursoMvccPartitionAlias]) -> Result<(), String> {
    let mut identities = BTreeSet::new();
    for alias in aliases {
        validate_alias_component(alias.alias_namespace(), "namespace")?;
        validate_alias_component(alias.alias_key(), "key")?;
        if !identities.insert((alias.alias_namespace(), alias.alias_key())) {
            return Err("MVCC partition aliases require unique identities".to_string());
        }
    }
    Ok(())
}

fn validate_alias_component(value: &str, component: &str) -> Result<(), String> {
    if (1..=256).contains(&value.len())
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'/' | b'-')
        })
    {
        Ok(())
    } else {
        Err(format!("invalid MVCC partition alias {component}"))
    }
}

fn validate_partition_key(partition_key: &str) -> Result<(), String> {
    if partition_key.is_empty() {
        Err("MVCC partition key must be non-empty".to_string())
    } else {
        Ok(())
    }
}

fn partition_lane(
    store: &TursoMvccStore,
    partition_key: &str,
) -> Arc<tokio::sync::Mutex<turso::Connection>> {
    let digest = blake3::hash(partition_key.as_bytes());
    let lane = u64::from_le_bytes(
        digest.as_bytes()[..8]
            .try_into()
            .expect("BLAKE3 digest prefix has eight bytes"),
    ) as usize
        % store.inner.lanes.len();
    Arc::clone(&store.inner.lanes[lane])
}

fn classify_error(context: &str, error: turso::Error) -> AttemptError {
    let snapshot = matches!(error, turso::Error::BusySnapshot(_));
    if snapshot || matches!(error, turso::Error::Busy(_)) {
        AttemptError::Retryable {
            message: format!("failed to {context}: {error}"),
            snapshot,
        }
    } else {
        AttemptError::Fatal(format!("failed to {context}: {error}"))
    }
}

fn retry_delay(attempt: usize) -> Duration {
    Duration::from_millis(1_u64 << attempt.min(6))
}
