//! Database-side keyset pagination for the single-process MVCC event store.

use crate::turso_mvcc_store::{TursoMvccEvent, TursoMvccStore, event_shard};

const FIRST_PAGE_SQL: [&str; 4] = [
    "SELECT partition_key, event_id, payload, created_at_ms FROM asp_mvcc_event_0 WHERE partition_key = ?1 ORDER BY created_at_ms, event_id LIMIT ?2",
    "SELECT partition_key, event_id, payload, created_at_ms FROM asp_mvcc_event_1 WHERE partition_key = ?1 ORDER BY created_at_ms, event_id LIMIT ?2",
    "SELECT partition_key, event_id, payload, created_at_ms FROM asp_mvcc_event_2 WHERE partition_key = ?1 ORDER BY created_at_ms, event_id LIMIT ?2",
    "SELECT partition_key, event_id, payload, created_at_ms FROM asp_mvcc_event_3 WHERE partition_key = ?1 ORDER BY created_at_ms, event_id LIMIT ?2",
];

const AFTER_PAGE_SQL: [&str; 4] = [
    "SELECT partition_key, event_id, payload, created_at_ms FROM asp_mvcc_event_0 WHERE partition_key = ?1 AND (created_at_ms > ?2 OR (created_at_ms = ?2 AND event_id > ?3)) ORDER BY created_at_ms, event_id LIMIT ?4",
    "SELECT partition_key, event_id, payload, created_at_ms FROM asp_mvcc_event_1 WHERE partition_key = ?1 AND (created_at_ms > ?2 OR (created_at_ms = ?2 AND event_id > ?3)) ORDER BY created_at_ms, event_id LIMIT ?4",
    "SELECT partition_key, event_id, payload, created_at_ms FROM asp_mvcc_event_2 WHERE partition_key = ?1 AND (created_at_ms > ?2 OR (created_at_ms = ?2 AND event_id > ?3)) ORDER BY created_at_ms, event_id LIMIT ?4",
    "SELECT partition_key, event_id, payload, created_at_ms FROM asp_mvcc_event_3 WHERE partition_key = ?1 AND (created_at_ms > ?2 OR (created_at_ms = ?2 AND event_id > ?3)) ORDER BY created_at_ms, event_id LIMIT ?4",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TursoMvccPartitionKey(String);

impl TursoMvccPartitionKey {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for TursoMvccPartitionKey {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for TursoMvccPartitionKey {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TursoMvccEventId(String);

impl TursoMvccEventId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for TursoMvccEventId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for TursoMvccEventId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TursoMvccPageCursor {
    pub created_at_ms: i64,
    pub event_id: TursoMvccEventId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TursoMvccPageLimit(usize);

impl TursoMvccPageLimit {
    pub const fn as_usize(self) -> usize {
        self.0
    }
}

impl From<usize> for TursoMvccPageLimit {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TursoMvccKeysetError(String);

impl std::fmt::Display for TursoMvccKeysetError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for TursoMvccKeysetError {}

impl From<String> for TursoMvccKeysetError {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for TursoMvccKeysetError {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl TursoMvccStore {
    /// Reads at most `limit + 1` rows using the composite keyset index.
    ///
    /// The extra row is retained so the backend-neutral adapter can expose a
    /// stable continuation cursor without issuing a count query.
    pub async fn read_partition_page(
        &self,
        partition_key: TursoMvccPartitionKey,
        after: Option<TursoMvccPageCursor>,
        limit: TursoMvccPageLimit,
    ) -> Result<Vec<TursoMvccEvent>, TursoMvccKeysetError> {
        let limit = limit.as_usize();
        if limit == 0 || limit > 1_000 {
            return Err("MVCC event page limit must be in 1..=1000".into());
        }
        let fetch_limit = limit + 1;
        let shard = event_shard(partition_key.as_str());
        let lane = self.inner.lanes[shard % self.inner.lanes.len()]
            .lock()
            .await;
        let mut statement = lane
            .prepare_cached(if after.is_some() {
                AFTER_PAGE_SQL[shard]
            } else {
                FIRST_PAGE_SQL[shard]
            })
            .await
            .map_err(|error| error.to_string())?;
        let mut rows = if let Some(cursor) = after {
            statement
                .query((
                    partition_key.as_str(),
                    cursor.created_at_ms,
                    cursor.event_id.as_str(),
                    fetch_limit as i64,
                ))
                .await
        } else {
            statement
                .query((partition_key.as_str(), fetch_limit as i64))
                .await
        }
        .map_err(|error| error.to_string())?;

        let mut events = Vec::with_capacity(fetch_limit);
        while let Some(row) = rows.next().await.map_err(|error| error.to_string())? {
            events.push(TursoMvccEvent {
                partition_key: row.get(0).map_err(|error| error.to_string())?,
                event_id: row.get(1).map_err(|error| error.to_string())?,
                payload: row.get(2).map_err(|error| error.to_string())?,
                created_at_ms: row.get(3).map_err(|error| error.to_string())?,
            });
        }
        Ok(events)
    }
}
