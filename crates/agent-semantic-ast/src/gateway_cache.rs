// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! BLAKE3 content-addressed Single-Flight deduplication cache for
//! [`crate::AspForwardingGateway`].
//!
//! When N concurrent agents or tools request `ast-inspect` for the exact same
//! `(language_id, path, blake3_source_hash)`, only 1 native compiler call runs;
//! the other N-1 callers coalesce on its in-flight future and reuse the verified
//! `WorkspaceOpenEdbIngestBatch` in microseconds.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use agent_semantic_search::WorkspaceOpenEdbIngestBatch;
use tokio::sync::{Semaphore, watch};

use crate::gateway_governance::ProviderGovernanceTable;
use crate::open_edb_projection::project_inspection_to_open_edb_batch;
use crate::provider_port::{AstInspectRequest, AstProviderPort};
use crate::types::{AstLanguageId, AstProviderId, AstWorkspacePath};

const MAX_CACHED_INSPECTIONS: usize = 4096;
const DEFAULT_PER_FILE_INSPECT_TIMEOUT: Duration = Duration::from_secs(30);

/// Content-addressed inspection cache key `(language_id, path, blake3_hex)`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct InspectCacheKey {
    pub language_id: String,
    pub path: String,
    pub source_hash: String,
}

type InFlightOutcome = Option<Result<WorkspaceOpenEdbIngestBatch, String>>;

/// Telemetry snapshot of the gateway's BLAKE3 single-flight cache.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InspectCacheStats {
    pub entries: usize,
    pub cache_hits: u64,
    pub single_flight_coalesced: u64,
    pub cache_misses: u64,
}

/// Resolution step returned by [`InspectSingleFlightCache::acquire`].
pub enum InspectCacheSlot {
    /// Immediate cache hit for this exact `(language_id, path, source_hash)`.
    Hit(WorkspaceOpenEdbIngestBatch),
    /// Another task is already inspecting this exact file+hash; await `rx`.
    Coalesced(watch::Receiver<InFlightOutcome>),
    /// Caller owns execution and must resolve via [`InspectSingleFlightCache::complete`].
    Leader(watch::Sender<InFlightOutcome>),
}

#[derive(Default)]
struct CacheInner {
    entries: HashMap<InspectCacheKey, WorkspaceOpenEdbIngestBatch>,
    order: VecDeque<InspectCacheKey>,
    in_flight: HashMap<InspectCacheKey, watch::Receiver<InFlightOutcome>>,
}

/// Thread-safe bounded BLAKE3 cache with single-flight coalescing.
#[derive(Default)]
pub struct InspectSingleFlightCache {
    inner: Mutex<CacheInner>,
    cache_hits: AtomicU64,
    coalesced: AtomicU64,
    misses: AtomicU64,
}

impl InspectSingleFlightCache {
    /// Construct an empty bounded cache.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Look up `key`, coalesce onto an existing in-flight worker, or claim leadership.
    #[must_use]
    pub fn acquire(&self, key: &InspectCacheKey) -> InspectCacheSlot {
        let Ok(mut guard) = self.inner.lock() else {
            let (tx, _) = watch::channel(None);
            return InspectCacheSlot::Leader(tx);
        };
        if let Some(batch) = guard.entries.get(key).cloned() {
            self.cache_hits.fetch_add(1, Ordering::Relaxed);
            return InspectCacheSlot::Hit(batch);
        }
        if let Some(rx) = guard.in_flight.get(key).cloned() {
            self.coalesced.fetch_add(1, Ordering::Relaxed);
            return InspectCacheSlot::Coalesced(rx);
        }
        self.misses.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = watch::channel(None);
        guard.in_flight.insert(key.clone(), rx);
        InspectCacheSlot::Leader(tx)
    }

    /// Complete an in-flight leader inspection, populating the cache on success
    /// and waking all coalesced waiters immediately.
    pub fn complete(
        &self,
        key: InspectCacheKey,
        tx: watch::Sender<InFlightOutcome>,
        outcome: Result<WorkspaceOpenEdbIngestBatch, String>,
    ) {
        if let Ok(mut guard) = self.inner.lock() {
            guard.in_flight.remove(&key);
            if let Ok(ref batch) = outcome {
                if !guard.entries.contains_key(&key) {
                    if guard.order.len() >= MAX_CACHED_INSPECTIONS
                        && let Some(evicted) = guard.order.pop_front()
                    {
                        guard.entries.remove(&evicted);
                    }
                    guard.order.push_back(key.clone());
                }
                guard.entries.insert(key, batch.clone());
            }
        }
        let _ = tx.send(Some(outcome));
    }

    /// Remove all cached entries for `language_id` (e.g. after dynamic provider replacement).
    pub fn invalidate_language(&self, language_id: &str) {
        if let Ok(mut guard) = self.inner.lock() {
            guard.entries.retain(|k, _| k.language_id != language_id);
            guard.order.retain(|k| k.language_id != language_id);
        }
    }

    /// Snapshot current hit/coalesce/miss telemetry counters.
    #[must_use]
    pub fn stats(&self) -> InspectCacheStats {
        let entries = self.inner.lock().map(|g| g.entries.len()).unwrap_or(0);
        InspectCacheStats {
            entries,
            cache_hits: self.cache_hits.load(Ordering::Relaxed),
            single_flight_coalesced: self.coalesced.load(Ordering::Relaxed),
            cache_misses: self.misses.load(Ordering::Relaxed),
        }
    }
}

/// Parameters executing one bounded, single-flight-cached provider inspection.
pub(crate) struct CachedInspectInvocation {
    pub path: AstWorkspacePath,
    pub language_id: AstLanguageId,
    pub provider_id: AstProviderId,
    pub inspect_req: AstInspectRequest,
    pub port: Arc<dyn AstProviderPort>,
    pub global_sem: Arc<Semaphore>,
    pub lang_sem: Arc<Semaphore>,
    pub governance: Arc<ProviderGovernanceTable>,
    pub cache: Arc<InspectSingleFlightCache>,
}

/// Execute one file inspection with BLAKE3 cache lookup, single-flight coalescing,
/// per-language circuit breaker enforcement, semaphore backpressure, and timeout.
pub(crate) async fn execute_cached_single_flight_inspect(
    inv: CachedInspectInvocation,
) -> (u64, Result<WorkspaceOpenEdbIngestBatch, String>) {
    let key = InspectCacheKey {
        language_id: inv.language_id.as_str().to_owned(),
        path: inv.path.as_str().to_owned(),
        source_hash: inv.inspect_req.source_hash.as_str().to_owned(),
    };
    let leader_tx = match inv.cache.acquire(&key) {
        InspectCacheSlot::Hit(batch) => return (0, Ok(batch)),
        InspectCacheSlot::Coalesced(mut rx) => {
            let started = Instant::now();
            loop {
                if let Some(result) = rx.borrow().clone() {
                    let micros = u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
                    return (micros, result);
                }
                if rx.changed().await.is_err() {
                    let micros = u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
                    return (
                        micros,
                        Err("single-flight leader dropped before completing".to_owned()),
                    );
                }
            }
        }
        InspectCacheSlot::Leader(tx) => tx,
    };

    let outcome_with_micros = execute_leader_inspect(&inv).await;
    inv.cache
        .complete(key, leader_tx, outcome_with_micros.1.clone());
    outcome_with_micros
}

async fn execute_leader_inspect(
    inv: &CachedInspectInvocation,
) -> (u64, Result<WorkspaceOpenEdbIngestBatch, String>) {
    if let Err(cb_err) = inv.governance.check_circuit_admits(&inv.language_id) {
        return (0, Err(cb_err));
    }
    let Ok(_global_permit) = Arc::clone(&inv.global_sem).acquire_owned().await else {
        return (0, Err("global inspect semaphore closed".to_owned()));
    };
    let Ok(_lang_permit) = Arc::clone(&inv.lang_sem).acquire_owned().await else {
        return (0, Err("provider inspect semaphore closed".to_owned()));
    };

    let port = Arc::clone(&inv.port);
    let req = inv.inspect_req.clone();
    let started = Instant::now();
    let timed = tokio::time::timeout(
        DEFAULT_PER_FILE_INSPECT_TIMEOUT,
        tokio::task::spawn_blocking(move || port.inspect(&req)),
    )
    .await;
    let call_micros = u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
    let inspected = match timed {
        Ok(Ok(Ok(resp))) => Ok(resp),
        Ok(Ok(Err(err))) => Err(err.to_string()),
        Ok(Err(join_err)) => Err(format!("inspect worker join error: {join_err}")),
        Err(_) => Err(format!(
            "provider `{}` timed out after {}s on {}",
            inv.language_id.as_str(),
            DEFAULT_PER_FILE_INSPECT_TIMEOUT.as_secs(),
            inv.path.as_str()
        )),
    };
    match inspected {
        Ok(resp) => {
            inv.governance.record_success(&inv.language_id, call_micros);
            (
                call_micros,
                Ok(project_inspection_to_open_edb_batch(
                    &inv.path,
                    &inv.language_id,
                    &inv.provider_id,
                    &resp.nodes,
                    &resp.edges,
                )),
            )
        }
        Err(error) => {
            inv.governance.record_failure(&inv.language_id, call_micros);
            (call_micros, Err(error))
        }
    }
}
